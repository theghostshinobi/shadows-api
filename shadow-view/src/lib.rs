//! `shadow-view` — la vista di supervisione (§9, Fase 5).
//!
//! # Una vista sola, quattro modi di guardarla
//!
//! Terminale, pagina statica, server e applicazione macOS mostrano **lo stesso
//! [`Status`]**. Non è economia di codice: è l'unico modo perché non dicano
//! numeri diversi sullo stesso storico. Chi guarda il cruscotto e chi guarda il
//! terminale devono poter discutere della stessa cosa.
//!
//! # Cosa c'è dentro una vista, e in che ordine
//!
//! L'ordine dei campi è l'ordine in cui uno se li chiede aprendo il cruscotto:
//!
//! 1. **cosa è cambiato** — gli endpoint comparsi nell'ultimo giro. È la sola
//!    domanda per cui si apre un cruscotto, e oggi vive solo su stderr del
//!    demone, dove scorre via;
//! 2. **gli alert aperti** — cosa aspetta una decisione, e da quanto;
//! 3. **l'inventario** — cosa c'è, con i quattro valori dell'autenticazione;
//! 4. **l'andamento** — come si è mosso nel tempo;
//! 5. **la provenienza** — chi l'ha prodotta, con quale ruleset, su quale input.
//!
//! L'ultimo punto è ciò che la rende un documento invece di una schermata, ed è
//! la stessa ragione per cui esiste il `RunManifest` (§6).
//!
//! # Zero rete
//!
//! Questo crate non apre niente. Serve una struttura e delle stringhe; chi le
//! mette su un socket è `shadow-serve`, che sta da solo proprio perché la
//! superficie di rete di questo progetto sia **un posto solo** e si possa
//! guardare tutta insieme.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod html;
mod terminal;

use shadow_history::{EndpointState, HistoryError, HistoryStore, InventoryRow, Target};

pub use html::{dashboard, escape, page, DEFAULT_REFRESH_SECONDS};
pub use terminal::{json, terminal};

/// Quanti run si guardano indietro per l'andamento.
///
/// Venti: abbastanza per vedere una tendenza, abbastanza pochi da restare una
/// riga di testo in un terminale.
pub const TREND_RUNS: u32 = 20;

/// Un giro del demone, come compare nell'andamento.
#[derive(Debug, Clone)]
pub struct TrendPoint {
    /// Identificatore progressivo del run.
    pub run_id: i64,
    /// Quando il run dice di essere avvenuto.
    pub timestamp: String,
    /// Endpoint conosciuti a quel giro.
    pub endpoints: u64,
    /// Righe di log analizzate fino a quel giro.
    pub lines: u64,
}

/// Lo stato di un bersaglio: tutto ciò che una supervisione mostra.
#[derive(Debug, Clone)]
pub struct Status {
    /// Il servizio sorvegliato.
    pub target: String,
    /// L'ultimo run registrato, se ce n'è uno.
    pub last_run: Option<TrendPoint>,
    /// Versione di Shadow e del ruleset dell'ultimo run.
    pub shadow_version: String,
    /// Versione del ruleset.
    pub ruleset_version: String,
    /// Gli endpoint comparsi **nell'ultimo giro**.
    pub new_in_last_run: Vec<EndpointState>,
    /// Gli alert aperti, cioè comparsi come `Shadow` e non presi in carico.
    pub open_alerts: Vec<EndpointState>,
    /// L'inventario completo.
    pub inventory: Vec<InventoryRow>,
    /// L'andamento, dal più vecchio al più recente.
    pub trend: Vec<TrendPoint>,
}

impl Status {
    /// Costruisce la vista leggendo lo storico.
    ///
    /// **Non calcola verdetti**: li legge. Un cruscotto che classificasse per
    /// conto suo sarebbe una seconda pipeline, e prima o poi direbbe una cosa
    /// diversa dal report (§P7, nello spirito).
    pub fn read(store: &HistoryStore, target: &Target) -> Result<Self, HistoryError> {
        let runs = store.recent_runs(target, TREND_RUNS)?;
        let inventory = store.inventory(target)?;
        let open_alerts = store.pending_alerts(target)?;

        let mut trend: Vec<TrendPoint> = runs
            .iter()
            .map(|run| {
                let (endpoints, lines) = counts_of(&run.manifest_json);
                TrendPoint {
                    run_id: run.id,
                    timestamp: run.timestamp.clone(),
                    endpoints,
                    lines,
                }
            })
            .collect();
        // Dal più vecchio al più recente: è il verso in cui si legge un
        // andamento.
        trend.reverse();

        let last_run = trend.last().cloned();
        let new_in_last_run = match &last_run {
            Some(point) => store.first_seen_in(target, point.run_id)?,
            None => Vec::new(),
        };
        let (shadow_version, ruleset_version) = runs
            .first()
            .map(|run| versions_of(&run.manifest_json))
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));

        Ok(Self {
            target: target.as_str().to_string(),
            last_run,
            shadow_version,
            ruleset_version,
            new_in_last_run,
            open_alerts,
            inventory,
            trend,
        })
    }

    /// Quanti endpoint per ciascuna classificazione, **zeri compresi**.
    ///
    /// Gli zeri ci sono per la stessa ragione del report d'audit: una categoria
    /// che sparisce quando è vuota fa credere che non esista.
    pub fn by_classification(&self) -> Vec<(&'static str, usize)> {
        ["Shadow", "Zombie", "Known", "Undetermined", "no verdict"]
            .iter()
            .map(|name| {
                (
                    *name,
                    self.inventory
                        .iter()
                        .filter(|row| shadow_compliance_label(row) == *name)
                        .count(),
                )
            })
            .collect()
    }
}

/// La stessa etichetta che usa il report d'audit.
///
/// Duplicarla qui sarebbe il modo di far divergere il cruscotto dal documento
/// che si consegna: si ricalca la regola, non si inventa.
fn shadow_compliance_label(row: &InventoryRow) -> &'static str {
    match row.last_classification.as_deref() {
        Some("Shadow") => "Shadow",
        Some("Zombie") => "Zombie",
        Some("Known") => "Known",
        Some("Undetermined") => "Undetermined",
        _ => "no verdict",
    }
}

/// Legge endpoint e righe dal manifest già redatto di un run.
///
/// Se il manifest non è leggibile si restituiscono zeri invece di indovinare:
/// un cruscotto che inventa un numero è peggio di un cruscotto che ne mostra
/// uno mancante (§P2).
fn counts_of(manifest_json: &str) -> (u64, u64) {
    let parsed: serde_json::Value = match serde_json::from_str(manifest_json) {
        Ok(value) => value,
        Err(_) => return (0, 0),
    };
    let counts = &parsed["counts"];
    (
        counts["endpoints_found"].as_u64().unwrap_or(0),
        counts["lines_total"].as_u64().unwrap_or(0),
    )
}

fn versions_of(manifest_json: &str) -> (String, String) {
    let parsed: serde_json::Value = match serde_json::from_str(manifest_json) {
        Ok(value) => value,
        Err(_) => return ("unknown".to_string(), "unknown".to_string()),
    };
    (
        parsed["shadow_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        parsed["ruleset_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
    )
}
