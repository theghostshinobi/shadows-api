//! Lettura a flusso di un file di log, in **una sola passata** (§P10).
//!
//! Nella stessa passata si fanno tre cose che sarebbe comodo, e sbagliato,
//! separare: si leggono le righe, si calcola il digest SHA-256 del contenuto
//! (§6) e si contano righe totali, parsate e scartate con il perché. Separarle
//! significherebbe leggere il file due volte, e su un log da gigabyte la
//! seconda lettura è un costo che nessuno vuole pagare.

use std::fmt;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::io::{self, BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use shadow_core::ruleset::{FORMAT_SAMPLE_LINES, MAX_LINE_BYTES};
use shadow_core::{DigestAlgorithm, InputDigest, InputRole, ObservedRequest, RunCounts, SourceRef};

use crate::collector::{Collector, DiscardReason};

/// Esito dell'ingestione di un file.
///
/// Non è un record del modello dati: descrive **come è andata la lettura**, non
/// una richiesta né un endpoint, che restano definiti solo in `shadow-core`
/// (§P7).
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    /// Conteggi di lettura: righe totali, parsate, scartate e perché (§6).
    ///
    /// I campi `endpoints_found` e `findings_by_classification` restano a zero:
    /// li riempie chi consuma le richieste, non chi legge il file.
    pub counts: RunCounts,

    /// Digest SHA-256 del contenuto del file, calcolato in questa stessa
    /// passata (§6).
    pub digest: InputDigest,
}

/// Perché un'ingestione non è potuta iniziare o proseguire.
///
/// Sono i casi che portano al codice di uscita `1` (§7): il run non produce un
/// verdetto, quindi non produce nemmeno un manifest.
#[derive(Debug)]
pub enum IngestError {
    /// Il file non è leggibile.
    Io {
        /// Percorso che si è tentato di leggere.
        path: PathBuf,
        /// Errore riportato dal sistema operativo.
        source: io::Error,
    },

    /// Il contenuto non corrisponde al formato dichiarato.
    ///
    /// È la difesa di §P2: meglio rifiutarsi che estrarre campi dalla posizione
    /// sbagliata e produrre numeri inventati che sembrano veri.
    FormatNotRecognized {
        /// File esaminato.
        path: PathBuf,
        /// Formato con cui lo si stava leggendo.
        format_name: String,
        /// Forma attesa da una riga, da mostrare all'utente.
        expected_shape: String,
        /// Quante righe non vuote sono state esaminate prima di arrendersi.
        lines_examined: u64,
        /// Numero della prima riga che non ha combaciato.
        first_offending_line: Option<u64>,
        /// Perché quella riga non ha combaciato.
        first_reason: Option<DiscardReason>,
    },
}

impl fmt::Display for IngestError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    ///
    /// Non mostrano **mai** il contenuto della riga incriminata, solo il suo
    /// numero: una riga di log può contenere token ed email, e il `Redactor`
    /// che le mascherebbe è di Fase 4. Fino ad allora un messaggio d'errore non
    /// deve poter diventare la falla (§P5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "cannot read {}: {}", path.display(), source)
            }
            Self::FormatNotRecognized {
                path,
                format_name,
                expected_shape,
                lines_examined,
                first_offending_line,
                first_reason,
            } => {
                write!(
                    f,
                    "{} does not look like the '{}' log format: {} non-empty line(s) examined, none could be parsed",
                    path.display(),
                    format_name,
                    lines_examined
                )?;
                if let (Some(line), Some(reason)) = (first_offending_line, first_reason) {
                    write!(f, " (line {}: {})", line, reason.as_str())?;
                }
                write!(
                    f,
                    "\n  expected each line to look like: {expected_shape}\
                     \n  declare the right format with --log-format, or check that this is the file you meant"
                )
            }
        }
    }
}

/// Legge un file di log a flusso, traducendo ogni riga con `collector` e
/// consegnando le richieste a `on_request` **mentre** vengono prodotte.
///
/// Le richieste non vengono accumulate: chi chiama decide cosa farne, e la
/// memoria di questa funzione resta quella di una riga (§P10).
///
/// # Errori
///
/// Fallisce se il file non è leggibile, o se nelle prime
/// [`FORMAT_SAMPLE_LINES`] righe non vuote **nessuna** combacia con il formato:
/// in quel caso il file è in un altro formato e va detto, non interpretato a
/// caso (§P2).
pub fn ingest_file(
    path: &Path,
    role: InputRole,
    collector: &dyn Collector,
    on_request: &mut dyn FnMut(ObservedRequest),
) -> Result<IngestOutcome, IngestError> {
    let file = File::open(path).map_err(|source| IngestError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let mut reader = HashingLineReader::new(BufReader::new(file));
    let mut counts = RunCounts::default();
    let mut line_buf: Vec<u8> = Vec::with_capacity(4096);

    let mut examined = 0_u64;
    let mut first_offending_line: Option<u64> = None;
    let mut first_reason: Option<DiscardReason> = None;

    loop {
        let truncated = match reader.next_line(&mut line_buf) {
            Ok(Some(truncated)) => truncated,
            Ok(None) => break,
            Err(source) => {
                return Err(IngestError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };

        counts.total_lines += 1;
        let line_number = counts.total_lines;

        let outcome = classify_line(&line_buf, truncated, collector, path, line_number);
        // Il campione di riconoscimento guarda solo le righe non vuote: una riga
        // vuota non dice niente sul formato.
        let counts_toward_sample = !matches!(outcome, Err(DiscardReason::BlankLine));

        match outcome {
            Ok(request) => {
                counts.parsed_lines += 1;
                on_request(request);
            }
            Err(reason) => {
                counts.discarded_lines += 1;
                *counts
                    .discarded_by_reason
                    .entry(reason.as_str().to_string())
                    .or_insert(0) += 1;
                if reason != DiscardReason::BlankLine && first_offending_line.is_none() {
                    first_offending_line = Some(line_number);
                    first_reason = Some(reason);
                }
            }
        }

        if counts_toward_sample {
            examined += 1;
        }
        if counts.parsed_lines == 0 && examined >= FORMAT_SAMPLE_LINES {
            return Err(IngestError::FormatNotRecognized {
                path: path.to_path_buf(),
                format_name: collector.format_name().to_string(),
                expected_shape: collector.expected_shape().to_string(),
                lines_examined: examined,
                first_offending_line,
                first_reason,
            });
        }
    }

    // Un file interamente illeggibile per questo formato è lo stesso caso, solo
    // scoperto alla fine perché il file era più corto del campione.
    if counts.parsed_lines == 0 && examined > 0 {
        return Err(IngestError::FormatNotRecognized {
            path: path.to_path_buf(),
            format_name: collector.format_name().to_string(),
            expected_shape: collector.expected_shape().to_string(),
            lines_examined: examined,
            first_offending_line,
            first_reason,
        });
    }

    Ok(IngestOutcome {
        counts,
        digest: InputDigest {
            path: path.to_path_buf(),
            role,
            algorithm: DigestAlgorithm::Sha256,
            digest: reader.finish(),
        },
    })
}

/// Decide il destino di una singola riga già letta.
fn classify_line(
    raw: &[u8],
    truncated: bool,
    collector: &dyn Collector,
    path: &Path,
    line_number: u64,
) -> Result<ObservedRequest, DiscardReason> {
    if truncated {
        return Err(DiscardReason::LineTooLong);
    }
    let raw = strip_carriage_return(raw);
    // Nessuna conversione "tollerante": sostituire i byte invalidi
    // cambierebbe in silenzio il dato analizzato (§P2).
    let line = std::str::from_utf8(raw).map_err(|_| DiscardReason::InvalidUtf8)?;
    if line.trim().is_empty() {
        return Err(DiscardReason::BlankLine);
    }
    collector.parse_line(
        line,
        SourceRef {
            file: path.to_path_buf(),
            line_number,
        },
    )
}

fn strip_carriage_return(raw: &[u8]) -> &[u8] {
    match raw.last() {
        Some(b'\r') => &raw[..raw.len() - 1],
        _ => raw,
    }
}

/// Lettore che consegna una riga per volta e, **negli stessi byte che
/// attraversa**, alimenta il digest SHA-256 del file (§6, §P10).
struct HashingLineReader<R> {
    inner: R,
    hasher: Sha256,
    consumed: u64,
}

impl<R: BufRead> HashingLineReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            consumed: 0,
        }
    }

    /// Un lettore che **riprende** un digest già cominciato, per la lettura
    /// incrementale del demone.
    fn resuming(inner: R, hasher: Sha256) -> Self {
        Self {
            inner,
            hasher,
            consumed: 0,
        }
    }

    /// Byte consumati da questo lettore.
    fn consumed(&self) -> u64 {
        self.consumed
    }

    /// Il digest **senza** consumare il lettore: serve a chi deve continuare.
    fn digest(&self) -> String {
        hex_digest(self.hasher.clone().finalize())
    }

    fn into_hasher(self) -> Sha256 {
        self.hasher
    }

    /// Legge la prossima riga in `buf`, senza il terminatore.
    ///
    /// Restituisce `None` a fine file, oppure `Some(troncata)` dove `troncata`
    /// dice che la riga superava [`MAX_LINE_BYTES`] ed è stata tenuta solo in
    /// parte. Anche una riga troncata viene **interamente attraversata e
    /// interamente digerita dall'hash**: il digest è del file, non di ciò che si
    /// è deciso di tenere.
    fn next_line(&mut self, buf: &mut Vec<u8>) -> io::Result<Option<bool>> {
        buf.clear();
        let mut consumed_any = false;
        let mut truncated = false;

        loop {
            let available = match self.inner.fill_buf() {
                Ok(bytes) => bytes,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };

            if available.is_empty() {
                return Ok(if consumed_any { Some(truncated) } else { None });
            }

            let newline = available.iter().position(|&b| b == b'\n');
            let (chunk, consume_len, end_of_line) = match newline {
                Some(i) => (&available[..i], i + 1, true),
                None => (available, available.len(), false),
            };

            self.hasher.update(&available[..consume_len]);
            self.consumed += consume_len as u64;
            append_capped(buf, chunk, &mut truncated);

            self.inner.consume(consume_len);
            consumed_any = true;

            if end_of_line {
                return Ok(Some(truncated));
            }
        }
    }

    /// Chiude il digest e lo restituisce in esadecimale minuscolo (§6).
    fn finish(self) -> String {
        hex_digest(self.hasher.finalize())
    }
}

/// Accoda `chunk` a `buf` senza superare [`MAX_LINE_BYTES`], segnalando il
/// troncamento. È il punto in cui §P10 resiste a un file senza ritorni a capo.
fn append_capped(buf: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    let room = MAX_LINE_BYTES.saturating_sub(buf.len());
    if chunk.len() > room {
        *truncated = true;
        buf.extend_from_slice(&chunk[..room]);
    } else {
        buf.extend_from_slice(chunk);
    }
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    let digest = digest.as_ref();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(nibble_to_hex(byte >> 4));
        out.push(nibble_to_hex(byte & 0x0f));
    }
    out
}

fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// Lo stato di una lettura **che riprende**: quanto si è già letto, e il digest
/// di tutto ciò che si è letto finora.
///
/// # Perché il digest è cumulativo e non del solo pezzo nuovo
///
/// Un demone analizza un inventario **accumulato**: al ciclo N i verdetti
/// riguardano tutto il traffico visto dall'inizio, non solo le righe arrivate
/// nell'ultimo minuto. Un manifest che dichiarasse il digest del solo pezzo
/// nuovo direbbe di aver analizzato mille byte mentre i finding ne coprono un
/// milione, e §6 vuole che il manifest dica **cosa è stato analizzato**.
///
/// Quindi il digest è quello dei primi `offset` byte del file: esattamente
/// l'input su cui i verdetti si reggono, e riproducibile da chiunque abbia il
/// file.
#[derive(Debug, Default)]
pub struct TailState {
    /// Byte già consumati.
    offset: u64,
    /// Digest cumulativo di quei byte.
    hasher: Option<Sha256>,
    /// Conteggi cumulativi, per lo stesso motivo del digest.
    counts: RunCounts,
}

impl TailState {
    /// Uno stato che comincia dall'inizio del file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Byte consumati finora.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// I conteggi cumulativi (§6).
    pub fn counts(&self) -> &RunCounts {
        &self.counts
    }
}

/// Cosa è successo in un giro di lettura incrementale.
#[derive(Debug, Clone)]
pub struct TailOutcome {
    /// Richieste consegnate in **questo** giro.
    pub new_requests: u64,
    /// Se il file è stato accorciato o sostituito da quando si è letto
    /// l'ultima volta, e la lettura è ripartita da capo.
    pub restarted: bool,
    /// Digest cumulativo di tutto ciò che si è letto (§6).
    pub digest: InputDigest,
}

/// Legge ciò che è **cresciuto** in un file dall'ultima volta.
///
/// # La rotazione
///
/// Se il file è più corto dell'offset, sotto di noi è successo qualcosa:
/// `logrotate` con `copytruncate`, o un file nuovo con lo stesso nome. Non si
/// prova a indovinare quale: si riparte da capo e **lo si dice**
/// ([`TailOutcome::restarted`]), perché un demone che ricomincia in silenzio
/// farebbe risultare nuovo tutto ciò che aveva già visto.
///
/// # Errori
///
/// Fallisce se il file non è leggibile. **Non** applica la validazione del
/// formato sulle prime righe: quella l'ha già fatta il primo giro, e rifarla a
/// ogni ciclo farebbe cadere un demone perché per un minuto sono arrivate solo
/// righe malformate.
pub fn ingest_tail(
    path: &Path,
    state: &mut TailState,
    collector: &dyn Collector,
    on_request: &mut dyn FnMut(ObservedRequest),
) -> Result<TailOutcome, IngestError> {
    let io_error = |source| IngestError::Io {
        path: path.to_path_buf(),
        source,
    };

    let mut file = File::open(path).map_err(io_error)?;
    let length = file.metadata().map_err(io_error)?.len();

    let restarted = length < state.offset;
    if restarted {
        *state = TailState::new();
    }
    file.seek(SeekFrom::Start(state.offset)).map_err(io_error)?;

    let mut reader = HashingLineReader::resuming(
        BufReader::new(file),
        state.hasher.take().unwrap_or_default(),
    );
    let mut line_buf: Vec<u8> = Vec::with_capacity(4096);
    let mut new_requests = 0_u64;

    loop {
        let truncated = match reader.next_line(&mut line_buf) {
            Ok(Some(truncated)) => truncated,
            Ok(None) => break,
            Err(source) => return Err(io_error(source)),
        };

        state.counts.total_lines += 1;
        let line_number = state.counts.total_lines;
        match classify_line(&line_buf, truncated, collector, path, line_number) {
            Ok(request) => {
                state.counts.parsed_lines += 1;
                new_requests += 1;
                on_request(request);
            }
            Err(reason) => {
                state.counts.discarded_lines += 1;
                *state
                    .counts
                    .discarded_by_reason
                    .entry(reason.as_str().to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    state.offset += reader.consumed();
    let digest = reader.digest();
    state.hasher = Some(reader.into_hasher());

    Ok(TailOutcome {
        new_requests,
        restarted,
        digest: InputDigest {
            path: path.to_path_buf(),
            role: InputRole::Log,
            algorithm: DigestAlgorithm::Sha256,
            digest,
        },
    })
}
