//! `ObservedInventory` — l'insieme degli `EndpointPattern` ricavati dal
//! traffico (§5, Fase 2).
//!
//! È il punto in cui le richieste smettono di essere righe e diventano
//! **endpoint logici**: `/api/users/1`, `/api/users/2` e `/api/users/3` sono
//! tre osservazioni della stessa risorsa, e da qui in poi il tool ragiona su
//! quella. La decisione su *quali* segmenti siano variabili sta in
//! [`variability`], con tutte le cautele che merita.

pub mod declared;
pub mod path;
pub mod variability;

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::hex;
use crate::model::{AuthObservation, EndpointId, EndpointPattern, ObservedAuth, ObservedRequest};

use path::canonical_segments;
use variability::PatternTree;

/// Quanti caratteri esadecimali del digest formano l'identificatore stabile.
///
/// 16 caratteri sono 64 bit: abbastanza perché una collisione fra endpoint di
/// uno stesso inventario non sia una preoccupazione, pochi abbastanza perché
/// l'identificatore resti leggibile in un report e incollabile in un'allowlist.
const ENDPOINT_ID_HEX_LEN: usize = 16;

/// `ObservedInventory` — l'insieme degli [`EndpointPattern`] ricavati dal
/// traffico (§5).
#[derive(Debug, Clone, Default)]
pub struct ObservedInventory {
    /// Indicizzato per pattern del path, non per identificatore: l'ordine di
    /// iterazione così è quello che un essere umano si aspetta leggendo un
    /// inventario, ed è comunque deterministico (§P4). L'identificatore stabile
    /// resta dentro ogni [`EndpointPattern`].
    patterns: BTreeMap<String, EndpointPattern>,
}

impl ObservedInventory {
    /// Quanti endpoint logici contiene.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Scorre gli endpoint in ordine deterministico (§P4).
    pub fn iter(&self) -> impl Iterator<Item = &EndpointPattern> {
        self.patterns.values()
    }
}

/// Costruisce un [`ObservedInventory`] osservando una richiesta per volta.
///
/// Accumula le richieste per **path canonico** e decide la normalizzazione solo
/// in [`Self::build`], quando ha visto tutto: la cardinalità di una posizione
/// non si può giudicare a metà file, e giudicarla in anticipo significherebbe
/// che l'esito dipende dall'ordine delle righe (§P4).
///
/// La memoria cresce con il numero di path **distinti**, non con la dimensione
/// del log: è il minimo indispensabile per poter contare valori distinti.
#[derive(Debug, Clone, Default)]
pub struct ObservedInventoryBuilder {
    tree: PatternTree,
}

impl ObservedInventoryBuilder {
    /// Un costruttore vuoto.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra una richiesta osservata.
    pub fn observe(&mut self, request: ObservedRequest) {
        let segments = canonical_segments(&request.raw_path);
        self.tree.observe(&segments, &request);
    }

    /// Produce l'inventario.
    ///
    /// La normalizzazione è già avvenuta mentre le richieste arrivavano (vedi
    /// [`variability`]): qui restano solo la fusione delle osservazioni
    /// confluite nello stesso pattern e l'assegnazione degli identificatori.
    pub fn build(self) -> ObservedInventory {
        let patterns = self
            .tree
            .collect()
            .into_iter()
            .map(|(pattern, observations)| {
                (pattern.clone(), observations.into_endpoint(pattern))
            })
            .collect();

        ObservedInventory { patterns }
    }
}

/// Ciò che si accumula su un path prima di sapere in quale pattern finirà.
///
/// Non è un record del modello dati (§P7): è un accumulatore interno, e i campi
/// che sopravvivono sono quelli che §6 assegna a `EndpointPattern`.
#[derive(Debug, Clone, Default)]
pub(crate) struct PathObservations {
    methods: BTreeSet<String>,
    first_seen: Option<DateTime<Utc>>,
    last_seen: Option<DateTime<Utc>>,
    count: u64,
    /// Richieste la cui autenticazione era **osservabile**: è la base su cui il
    /// verdetto si regge, e §6 vuole che sia visibile.
    auth_observable: u64,
    /// Fra quelle osservabili, quante presentavano autenticazione.
    auth_present: u64,
}

impl PathObservations {
    pub(crate) fn add(&mut self, request: &ObservedRequest) {
        self.methods.insert(request.method.clone());
        self.count += 1;
        self.first_seen = Some(match self.first_seen {
            Some(current) => current.min(request.timestamp),
            None => request.timestamp,
        });
        self.last_seen = Some(match self.last_seen {
            Some(current) => current.max(request.timestamp),
            None => request.timestamp,
        });
        match request.auth {
            ObservedAuth::NotObservable => {}
            ObservedAuth::Absent => self.auth_observable += 1,
            ObservedAuth::Present { .. } => {
                self.auth_observable += 1;
                self.auth_present += 1;
            }
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.methods.extend(other.methods);
        self.count += other.count;
        self.auth_observable += other.auth_observable;
        self.auth_present += other.auth_present;
        self.first_seen = min_option(self.first_seen, other.first_seen);
        self.last_seen = max_option(self.last_seen, other.last_seen);
    }

    /// Applica la **regola di propagazione dell'autenticazione** di §6.
    ///
    /// Se nessuna richiesta era osservabile il pattern è `NotObservable`: che il
    /// log taccia non è una prova che l'autenticazione mancasse. Se almeno una
    /// lo era, il verdetto si calcola **sulle sole osservabili**, e quante
    /// fossero resta scritto accanto — perché "nessuna auth" su 3 richieste
    /// osservabili di 5000 è un verdetto debole e chi legge deve poterlo vedere.
    fn auth_verdict(&self) -> AuthObservation {
        if self.auth_observable == 0 {
            AuthObservation::NotObservable
        } else if self.auth_present == self.auth_observable {
            AuthObservation::Yes
        } else if self.auth_present == 0 {
            AuthObservation::No
        } else {
            AuthObservation::Mixed
        }
    }

    fn into_endpoint(self, pattern: String) -> EndpointPattern {
        let epoch = DateTime::from_timestamp(0, 0).expect("epoch è un timestamp valido");
        EndpointPattern {
            id: endpoint_id(&pattern),
            auth_observed: self.auth_verdict(),
            auth_observable_count: self.auth_observable,
            first_seen: self.first_seen.unwrap_or(epoch),
            last_seen: self.last_seen.unwrap_or(epoch),
            observation_count: self.count,
            methods: self.methods,
            path_pattern: pattern,
        }
    }
}

fn min_option(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (only, None) | (None, only) => only,
    }
}

fn max_option(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (only, None) | (None, only) => only,
    }
}

/// Deriva l'**identificatore stabile** di un endpoint dal suo pattern (§6).
///
/// Dipende **solo** dal pattern del path: non dai metodi osservati, non dai
/// conteggi, non dai timestamp. È ciò che rende l'identificatore lo stesso fra
/// run diversi (§P4) anche quando il traffico cambia — senza, un endpoint su
/// cui domani compare un `POST` cambierebbe identità e uscirebbe dall'allowlist
/// in cui l'utente l'aveva messo (§5, Fase 4).
fn endpoint_id(pattern: &str) -> EndpointId {
    let digest = Sha256::digest(pattern.as_bytes());
    EndpointId::new(hex::encode(&digest[..ENDPOINT_ID_HEX_LEN / 2]))
}
