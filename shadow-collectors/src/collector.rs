//! Il contratto `Collector` (§5, §8) e il catalogo dei motivi di scarto.

use shadow_core::{ObservedRequest, SourceRef};

/// `Collector` — il modulo che traduce **una riga** di un formato di log
/// specifico in una [`ObservedRequest`] (§5).
///
/// Il contratto è deliberatamente stretto: un Collector sa fare **una** cosa,
/// tradurre una riga, e non sa nulla di file, di flussi o di conteggi. Di
/// leggere il file si occupa [`ingest_file`](crate::ingest::ingest_file), una
/// volta sola per tutti i formati: così aggiungere un formato non significa
/// riscrivere la lettura a flusso né il calcolo del digest, e non c'è modo che
/// un secondo formato reintroduca per sbaglio una seconda passata sul file
/// (§P10).
pub trait Collector {
    /// Nome canonico del formato, cioè il valore che `log_format` (§7) assume
    /// per questo Collector.
    ///
    /// Il prestito è da `self` e non `'static` perché una grammatica
    /// **dichiarata dall'utente** non ha un nome noto a tempo di compilazione.
    fn format_name(&self) -> &str;

    /// Descrizione della forma attesa da una riga, mostrata all'utente quando
    /// il formato non viene riconosciuto. È testo rivolto all'utente, quindi in
    /// inglese (§5).
    fn expected_shape(&self) -> &str;

    /// Traduce una riga in una [`ObservedRequest`], o dice **perché** non ci
    /// riesce.
    ///
    /// Contratto (§P2): o la riga combacia per intero con la grammatica del
    /// formato e ogni campo viene letto dalla posizione che gli spetta, o il
    /// risultato è `Err`. Non è ammesso restituire una `ObservedRequest`
    /// parziale, con campi indovinati o letti da un'altra posizione.
    fn parse_line(&self, line: &str, source: SourceRef) -> Result<ObservedRequest, DiscardReason>;
}

/// Perché una riga è stata scartata (§6: "righe scartate **e perché**").
///
/// Questi valori diventano le chiavi di
/// [`RunCounts::discarded_by_reason`](shadow_core::RunCounts::discarded_by_reason)
/// nel manifest, quindi sono **stringhe in inglese** e stabili nel tempo (§5,
/// §P4): cambiarle cambia il contenuto di un file d'audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiscardReason {
    /// Riga vuota o composta di soli spazi.
    BlankLine,

    /// La riga non è UTF-8 valido. Non si tenta nessuna conversione
    /// "tollerante": sostituire i byte invalidi cambierebbe silenziosamente il
    /// dato analizzato (§P2).
    InvalidUtf8,

    /// La riga supera la lunghezza massima del ruleset
    /// ([`shadow_core::ruleset::MAX_LINE_BYTES`]). Scartarla è ciò che impedisce
    /// a un file senza ritorni a capo di diventare un'allocazione grande quanto
    /// il file (§P10).
    LineTooLong,

    /// La riga non combacia con la struttura generale del formato.
    MalformedLine,

    /// La riga combacia, ma la richiesta fra virgolette non ha la forma
    /// `METODO PATH HTTP/versione`.
    InvalidRequestLine,

    /// La riga combacia, ma il campo dello status non è un codice HTTP.
    InvalidStatusCode,

    /// La riga combacia, ma il campo del tempo non è una data valida nel
    /// formato atteso.
    InvalidTimestamp,
}

impl DiscardReason {
    /// Chiave canonica con cui il motivo compare nel `RunManifest`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BlankLine => "blank-line",
            Self::InvalidUtf8 => "invalid-utf8",
            Self::LineTooLong => "line-too-long",
            Self::MalformedLine => "malformed-line",
            Self::InvalidRequestLine => "invalid-request-line",
            Self::InvalidStatusCode => "invalid-status-code",
            Self::InvalidTimestamp => "invalid-timestamp",
        }
    }
}
