//! Il modello dati centrale — la spina dorsale del progetto (§6).
//!
//! Regola (§P7 — singola fonte di verità): questi record sono definiti **una
//! volta sola**, qui, e importati ovunque servano. `shadow-collectors` e
//! `shadow-cli` non definiscono record propri per rappresentare richieste o
//! endpoint: importano questi.
//!
//! Il *contenuto* dei record è fissato da §6 del blueprint ed è un contratto
//! immutabile salvo revisione esplicita del fondatore (§0). Le fasi successive
//! popolano questi campi, non li ridisegnano.
//!
//! Mappa dei record:
//!
//! | Record | Modulo | Chi lo produce |
//! |---|---|---|
//! | [`ObservedRequest`] | [`observed_request`] | i `Collector` (Fase 1) |
//! | [`EndpointPattern`] | [`endpoint_pattern`] | la normalizzazione (Fase 2) |
//! | [`DeclaredEndpoint`] | [`declared_endpoint`] | il parsing dell'OpenAPI (Fase 3) |
//! | [`Finding`] | [`finding`] | la classificazione (Fase 3) |
//! | [`RunManifest`] | [`run_manifest`] | l'orchestratore, a ogni run (Fase 1→4) |

pub mod declared_endpoint;
pub mod endpoint_pattern;
pub mod finding;
pub mod observed_request;
pub mod run_manifest;

pub use declared_endpoint::DeclaredEndpoint;
pub use endpoint_pattern::{AuthObservation, EndpointId, EndpointPattern};
pub use finding::{Classification, Confidence, Finding, FindingSubject, Severity};
pub use observed_request::{ObservedAuth, ObservedRequest, SourceRef};
pub use run_manifest::{
    ConfigSetting, ConfigSource, DigestAlgorithm, InputDigest, InputRole, RunConfiguration,
    RunCounts, RunManifest, RulesetVersion,
};
