//! `shadow-core` — il motore di Shadow (§8 del blueprint).
//!
//! Questo crate è la **fonte di verità unica** del modello dati canonico (§6, §P7):
//! i record che rappresentano richieste, endpoint e risultati sono definiti qui
//! una sola volta e importati da chiunque ne abbia bisogno. Nessun altro crate
//! definisce una propria versione di `ObservedRequest` o di qualunque altro
//! record del modello.
//!
//! Vincoli architetturali (§8), validi in ogni fase:
//! - zero I/O di rete, zero lettura o scrittura di file;
//! - nessuna dipendenza dalla CLI o da un formato di log specifico;
//! - libreria pura, testabile in isolamento.
//!
//! # Stato: Fase 0 — fondamenta e contratti
//!
//! Qui esistono **solo i contratti**: le strutture del modello dati (§6) e il
//! vocabolario canonico (§5). Non esiste ancora nessuna logica di analisi —
//! normalizzazione dei path (Fase 2) e classificazione (Fase 3) arrivano dopo.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod allowlist;
pub mod classify;
pub mod hex;
pub mod inventory;
pub mod model;
pub mod redact;
pub mod ruleset;
pub mod vocabulary;

pub use allowlist::Allowlist;
pub use classify::classify;
pub use redact::Redactor;
pub use inventory::declared::DeclaredInventory;
pub use inventory::{ObservedInventory, ObservedInventoryBuilder};
pub use model::{
    AuthObservation, Classification, ConfigSetting, ConfigSource, Confidence, DeclaredEndpoint,
    DigestAlgorithm, EndpointId, EndpointPattern, Finding, FindingSubject, InputDigest, InputRole,
    ObservedAuth, ObservedRequest, RunConfiguration, RunCounts, RunManifest, RulesetVersion,
    Severity, SourceRef,
};

/// Nome canonico del tool (§3: "Shadow" è **nome di lavoro**, il conflitto di
/// naming è una decisione ancora aperta e non va consolidata nel branding).
pub const TOOL_NAME: &str = "shadow";

pub use ruleset::RULESET_VERSION;
