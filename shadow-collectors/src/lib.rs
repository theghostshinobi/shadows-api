//! `shadow-collectors` — i `Collector` di Shadow (§8 del blueprint).
//!
//! Un `Collector` traduce **un formato di log specifico** in
//! [`ObservedRequest`](shadow_core::ObservedRequest) (§5). Qui vive l'I/O di
//! lettura, che `shadow-core` non può avere.
//!
//! Regole che valgono per ogni Collector, presente e futuro:
//!
//! - **§P7 — nessun modello dati locale.** Un Collector *importa*
//!   [`shadow_core::ObservedRequest`] e i tipi correlati; non ne definisce
//!   versioni proprie, nemmeno "temporanee" o "di comodo". I tipi definiti in
//!   questo crate descrivono l'**ingestione** (esiti, errori, conteggi), mai
//!   una richiesta o un endpoint.
//! - **§P10 — lettura a flusso.** Il file si legge riga per riga e **una volta
//!   sola**: il digest SHA-256 dell'input si calcola nella stessa passata, mai
//!   con una seconda lettura. La memoria non cresce con la dimensione del file.
//! - **§7 — le soglie stanno in un posto solo.** Le costanti che decidono cosa
//!   viene scartato vivono in [`shadow_core::ruleset`] e sono versionate con il
//!   `RulesetVersion`: qui si applicano, non si ridefiniscono.
//! - **§P2 — mai best-effort silenzioso.** La grammatica di ogni formato è
//!   **stretta**: o una riga combacia per intero e ogni campo è letto dalla
//!   posizione che gli spetta, o la riga è scartata. Non esiste la via di mezzo
//!   in cui un campo viene letto dal posto sbagliato, che è il modo in cui un
//!   tool di sicurezza produce numeri inventati che sembrano veri.
//! - **Righe malformate:** si saltano, si contano con il **perché**, e i
//!   conteggi finiscono nel [`RunManifest`](shadow_core::RunManifest) — non
//!   fanno cadere il run.
//!
//! # Stato: Fase 1
//!
//! Esiste il contratto [`Collector`] e il suo primo e unico consumatore, il
//! formato `combined` di nginx ([`NginxCombinedCollector`]). Il contratto è
//! disegnato perché aggiungere un formato non tocchi il core, ma **non** è
//! generalizzato su formati ipotetici: il secondo formato è una decisione
//! ancora aperta (§3).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod collector;
pub mod ingest;
pub mod log_format;
pub mod nginx;

pub use collector::{Collector, DiscardReason};
pub use ingest::{ingest_file, ingest_tail, IngestError, IngestOutcome, TailOutcome, TailState};
pub use log_format::{DeclaredLogFormat, LogFormatError};
pub use nginx::NginxCombinedCollector;

/// Perché il valore di `log_format` (§7) non è utilizzabile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Un nome che non corrisponde a nessun formato conosciuto.
    UnknownName(String),
    /// Una dichiarazione di grammatica che non compila.
    Declaration(LogFormatError),
}

impl std::fmt::Display for FormatError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownName(given) => write!(
                f,
                "unknown log format '{given}'. Supported: {}. \
                 You can also declare your own by passing the log_format line from your server \
                 configuration, for example: \
                 --log-format '{}'",
                SUPPORTED_FORMATS.join(", "),
                nginx::NginxCombinedCollector::FORMAT_SPEC
            ),
            Self::Declaration(error) => write!(f, "{error}"),
        }
    }
}

/// Restituisce il Collector per il valore di `log_format` (§7), che è **o** un
/// nome canonico **o** una grammatica dichiarata dall'utente.
///
/// Le due si distinguono senza ambiguità: una dichiarazione contiene almeno una
/// variabile `$nome`, e nessun nome canonico contiene `$`. Non è una
/// convenzione di comodo — è ciò che permette a una sola chiave di §7 di
/// portare entrambe le cose senza che un valore possa essere letto per l'altra.
///
/// È l'unico punto in cui un valore di `log_format` diventa un Collector.
pub fn collector_for_format(value: &str) -> Result<Box<dyn Collector>, FormatError> {
    if value.contains('$') {
        return DeclaredLogFormat::parse(value)
            .map(|format| Box::new(format) as Box<dyn Collector>)
            .map_err(FormatError::Declaration);
    }
    match value {
        NginxCombinedCollector::FORMAT_NAME => Ok(Box::new(NginxCombinedCollector)),
        _ => Err(FormatError::UnknownName(value.to_string())),
    }
}

/// Nomi canonici dei formati di log preimpostati, in ordine deterministico (§P4).
///
/// Serve ai messaggi d'errore: quando l'utente dichiara un formato che non
/// esiste, gli si dice quali esistono invece di lasciarlo indovinare. **Non** è
/// l'elenco di ciò che Shadow sa leggere: una grammatica dichiarata non ha un
/// nome, e non compare qui.
pub const SUPPORTED_FORMATS: &[&str] = &[NginxCombinedCollector::FORMAT_NAME];
