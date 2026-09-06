//! `shadow-compliance` — il report d'audit esportabile (§9, Fase 5).
//!
//! # Cosa è, e cosa non è
//!
//! È l'inventario degli endpoint di un servizio, con la loro classificazione e
//! lo **stato dell'autenticazione**, in una forma che si consegna a chi fa una
//! verifica — orientato a requisiti tipo l'inventario endpoint di PCI DSS 4.0.
//!
//! **Non è una certificazione di sicurezza**, e il report lo dice di sé, in
//! testa, in ogni formato. §1 lo mette fra i non-obiettivi e chiede che
//! l'onestà si veda nell'output: un documento che arriva a un auditor senza
//! dichiarare cosa non ha guardato è peggio di nessun documento.
//!
//! # La regola che questo crate esiste per non violare
//!
//! **I quattro valori dell'autenticazione non collassano mai in un binario.**
//! §9 lo scrive per esteso proprio qui: *«un report d'audit che dichiara "senza
//! auth" dove il dato non era osservabile è peggio di un report che tace»*.
//!
//! Quindi `not observable` è una colonna con un valore suo, non una cella
//! vuota; e accanto c'è **su quante richieste osservabili** il verdetto si
//! regge, perché «senza auth» e «senza auth, su 3 richieste su 5000» non sono
//! la stessa affermazione (§6).
//!
//! C'è anche un quinto stato, che non è di §6 ed è dello storico: `not
//! recorded`, per le righe scritte prima che lo storico registrasse l'auth.
//! Non è «non osservabile» — è «non lo abbiamo scritto» — e confonderle
//! sarebbe la stessa perdita silenziosa in un piano diverso.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod render;

use shadow_history::InventoryRow;

pub use render::{csv, json, markdown, SCHEMA};

/// Il contenuto di un report d'audit: cosa si è guardato, quando, e cosa si è
/// trovato.
pub struct ComplianceReport<'a> {
    /// Il servizio a cui si riferisce.
    pub target: &'a str,
    /// Identificatore dell'ultimo run che ha alimentato lo storico.
    pub last_run_id: Option<i64>,
    /// Timestamp di quel run.
    pub last_run_timestamp: Option<&'a str>,
    /// Versione di Shadow e del ruleset che l'hanno prodotto.
    pub shadow_version: &'a str,
    /// Versione del ruleset.
    pub ruleset_version: &'a str,
    /// L'inventario, già redatto e in ordine deterministico.
    pub inventory: &'a [InventoryRow],
}

/// Quello che il report dichiara di sé, prima di ogni tabella.
///
/// Non è una formalità: §1 mette la certificazione fra i non-obiettivi e chiede
/// che l'onestà dell'output lo rifletta.
pub const DISCLAIMER: &[&str] = &[
    "Shadow is a discovery aid, not a security certification.",
    "This inventory covers only endpoints that appeared in the analysed logs. An endpoint that received no traffic in the covered period is not in it, and its absence here is not evidence that it does not exist.",
    "The authentication column carries four distinct values and never collapses them into present/absent. 'not observable' means the log format does not carry the information — it is not evidence that authentication was missing.",
    "'observable requests' is the number of requests the authentication verdict rests on. A verdict built on 3 observable requests out of 5000 is weak, and this column is how you can see that.",
];

impl ComplianceReport<'_> {
    /// Quanti endpoint per ciascuno stato di autenticazione.
    ///
    /// Restituisce le voci in ordine fisso e **include gli zeri**: uno stato che
    /// sparisce dalla tabella quando è vuoto fa sembrare che non esista.
    pub fn auth_summary(&self) -> Vec<(&'static str, usize)> {
        AUTH_STATES
            .iter()
            .map(|state| {
                (
                    *state,
                    self.inventory
                        .iter()
                        .filter(|row| auth_label(row) == *state)
                        .count(),
                )
            })
            .collect()
    }

    /// Quanti endpoint per ciascuna classificazione, zeri compresi.
    pub fn classification_summary(&self) -> Vec<(&'static str, usize)> {
        CLASSIFICATIONS
            .iter()
            .map(|name| {
                (
                    *name,
                    self.inventory
                        .iter()
                        .filter(|row| classification_label(row) == *name)
                        .count(),
                )
            })
            .collect()
    }
}

/// I cinque stati che la colonna dell'autenticazione può assumere, in ordine
/// fisso. I primi quattro sono quelli di §6; il quinto è dello storico.
pub const AUTH_STATES: &[&str] = &["yes", "no", "mixed", "not observable", "not recorded"];

/// Le classificazioni, più lo stato di chi non ne ha ricevuta nessuna.
pub const CLASSIFICATIONS: &[&str] = &["Shadow", "Zombie", "Known", "Undetermined", "no verdict"];

/// L'etichetta dell'autenticazione di una riga, mai binaria.
pub fn auth_label(row: &InventoryRow) -> &'static str {
    match row.auth_observed.as_deref() {
        Some("yes") => "yes",
        Some("no") => "no",
        Some("mixed") => "mixed",
        Some("notobservable") | Some("not observable") => "not observable",
        // Sconosciuto o assente: **non** si indovina, e soprattutto non si
        // sceglie fra "sì" e "no" (§9).
        _ => "not recorded",
    }
}

/// L'etichetta della classificazione, con un valore esplicito per «nessun
/// verdetto» invece di una cella vuota.
pub fn classification_label(row: &InventoryRow) -> &'static str {
    match row.last_classification.as_deref() {
        Some("Shadow") => "Shadow",
        Some("Zombie") => "Zombie",
        Some("Known") => "Known",
        Some("Undetermined") => "Undetermined",
        _ => "no verdict",
    }
}
