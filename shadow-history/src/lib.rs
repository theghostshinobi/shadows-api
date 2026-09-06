//! `shadow-history` — lo storico persistente di Shadow (§9, Fase 5).
//!
//! # Cosa aggiunge, e cosa deliberatamente non aggiunge
//!
//! La Fase 5 è una **modalità d'uso** nuova, non funzioni di analisi nuove
//! (§9). Questo crate conserva ciò che un run ha concluso e riconosce ciò che
//! non aveva mai visto prima; **non** partecipa alla classificazione, non la
//! corregge e non la influenza.
//!
//! È la lettura stretta di §P4 con il tempo di mezzo: *a input costante il
//! verdetto deve restare identico*. Un demone introduce orologio, ordine di
//! arrivo e stato accumulato, e ognuna delle tre cose è un modo di far dipendere
//! un verdetto da qualcosa che non è l'input. Qui la separazione è strutturale:
//! `shadow-core` classifica senza sapere che questo crate esista, e la novità
//! («questo endpoint non c'era») è un'**annotazione sul finding**, mai un
//! ingrediente del finding.
//!
//! # §P5 — lo storico è una superficie nuova
//!
//! Un database su disco è un file che sopravvive al run, viene copiato nei
//! backup e finisce sulla macchina di qualcun altro. Quindi qui dentro entra
//! **solo roba già passata dal `Redactor`**: i pattern, l'evidenza, i percorsi.
//! Chi scrive lo storico passa i valori redatti, e il tipo lo rende esplicito
//! invece di affidarlo alla memoria di chi chiama.
//!
//! # L'ordine non lo decide l'orologio
//!
//! I run sono ordinati dal proprio identificatore progressivo, non dal
//! `run_timestamp`. È la difesa contro l'orologio che va all'indietro (§10,
//! fixture avversaria obbligatoria): un run registrato dopo è *dopo*, anche se
//! la macchina dice che è successo prima. Il timestamp resta un dato, e quando
//! contraddice l'ordine di registrazione lo si dice, non lo si aggiusta.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod schema;
mod store;

/// Riesportata perché chi scrive nello storico non debba dipendere anche dal
/// core solo per nominare una classificazione (§P7: il tipo resta uno solo).
pub use shadow_core::Classification;

pub use store::{
    EndpointState, FindingRecord, HistoryError, HistoryStore, InventoryRow,
    ObservedEndpointRecord, RecordedRun, RunSummary, Target,
};
