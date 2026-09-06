//! `EndpointPattern` — l'endpoint logico e il suo identificatore stabile (§5, §6).

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};

/// `EndpointPattern` — l'endpoint logico ottenuto dalla normalizzazione (§5).
///
/// È il risultato del raggruppamento delle [`ObservedRequest`](super::ObservedRequest)
/// che descrivono la stessa risorsa (es. `/api/users/1` e `/api/users/2` →
/// `/api/users/{id}`). L'insieme degli `EndpointPattern` ricavati dal traffico
/// è l'`ObservedInventory` (§5).
///
/// La logica che costruisce questi record è Fase 2 e deve essere
/// **conservativa** (§P3): nel dubbio non si aggrega, perché un falso
/// raggruppamento fa sparire un endpoint reale — il fallimento che il prodotto
/// esiste per evitare.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndpointPattern {
    /// Identificatore stabile dell'endpoint (§6). Vedi [`EndpointId`].
    pub id: EndpointId,

    /// Pattern del path normalizzato, con i segmenti variabili resi espliciti
    /// (es. `/api/users/{id}`).
    pub path_pattern: String,

    /// Insieme dei metodi HTTP osservati su questo pattern. Ordinato
    /// ([`BTreeSet`]) per determinismo dell'output (§P4).
    pub methods: BTreeSet<String>,

    /// Primo timestamp di osservazione.
    pub first_seen: DateTime<Utc>,

    /// Ultimo timestamp di osservazione. Insieme a `zombie_staleness_days` (§7)
    /// è ciò che permette di dire se un endpoint dichiarato è diventato `Zombie`.
    pub last_seen: DateTime<Utc>,

    /// Numero di richieste osservate su questo pattern.
    pub observation_count: u64,

    /// Autenticazione osservata sul pattern: sì / no / mista / non osservabile (§6).
    pub auth_observed: AuthObservation,

    /// Numero di richieste **osservabili** su cui il verdetto di
    /// [`Self::auth_observed`] si basa (§6).
    ///
    /// Va letto insieme a [`Self::observation_count`] e mostrato all'utente
    /// accanto al verdetto: "nessuna auth osservata" ricavato da 3 richieste
    /// osservabili su 5000 è un verdetto debole, e chi legge deve poterlo
    /// vedere invece di doverlo indovinare (§P9).
    ///
    /// Vale `0` se e solo se [`Self::auth_observed`] è
    /// [`AuthObservation::NotObservable`].
    pub auth_observable_count: u64,
}

/// Identificatore **stabile** di un [`EndpointPattern`] (§6).
///
/// Contratto: lo stesso endpoint riceve lo stesso identificatore in run
/// diversi (§P4). È ciò che permette di riconoscere "già visto" fra due
/// esecuzioni e di far funzionare l'`Allowlist` senza che l'utente rivedesse
/// lo stesso rumore a ogni run (§5, Fase 4).
///
/// **La regola di derivazione è assegnata in Fase 2** ed è parte del ruleset:
/// in Fase 0 esiste il tipo e il contratto, non il calcolo. Nessun componente
/// deve costruire un `EndpointId` con criteri propri.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EndpointId(String);

impl EndpointId {
    /// Costruisce un `EndpointId` da un valore già derivato secondo la regola
    /// di Fase 2. Non calcola nulla: è un contenitore tipizzato, non una
    /// funzione di derivazione.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Rappresentazione testuale dell'identificatore.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Autenticazione osservata su un [`EndpointPattern`]: sì / no / mista / non
/// osservabile (§6).
///
/// È un'osservazione sul traffico, non un giudizio: dire che un endpoint non ha
/// autenticazione è un'affermazione forte e va fatta solo sulla base di ciò che
/// il log mostra davvero (§P2, vedi [`ObservedAuth`](super::ObservedAuth)).
///
/// # Regola di propagazione dalle richieste al pattern (§6)
///
/// Ogni [`ObservedRequest`](super::ObservedRequest) porta un
/// [`ObservedAuth`](super::ObservedAuth) che può essere non osservabile. Il
/// verdetto del pattern si costruisce così:
///
/// - se **nessuna** richiesta del pattern è osservabile → [`Self::NotObservable`];
/// - se **almeno una** lo è → il verdetto ([`Self::Yes`] / [`Self::No`] /
///   [`Self::Mixed`]) si calcola **sulle sole richieste osservabili**, ignorando
///   le altre; quante fossero è registrato in
///   [`EndpointPattern::auth_observable_count`].
///
/// Il calcolo appartiene alla Fase 2: qui c'è la regola che dovrà rispettare.
///
/// # Conseguenza per le euristiche (Fase 3)
///
/// Le euristiche basate sull'**assenza** di autenticazione **non si attivano**
/// su [`Self::NotObservable`]. Se il formato di log non trasporta
/// l'informazione, un'euristica del tipo "endpoint senza auth" non sbaglierebbe
/// ogni tanto: sbaglierebbe sistematicamente, su tutto il file. Meglio spenta
/// che sempre in errore.
///
/// È un'eccezione consapevole a §P1 — qui si tace perché il segnale non esiste,
/// non perché sia scomodo — e il fatto che l'informazione manchi va comunque
/// detto all'utente, non nascosto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthObservation {
    /// Tutte le richieste **osservabili** del pattern presentavano autenticazione.
    Yes,

    /// Nessuna delle richieste **osservabili** del pattern presentava autenticazione.
    No,

    /// Fra le richieste osservabili, alcune con autenticazione e altre senza. È
    /// il caso che merita attenzione: la stessa risorsa risponde in entrambi i modi.
    Mixed,

    /// Nessuna richiesta del pattern era osservabile: il formato di log non
    /// trasporta l'informazione di autenticazione.
    ///
    /// **Non** significa "senza autenticazione". Significa che di questo
    /// endpoint non sappiamo dire nulla sull'autenticazione, ed è
    /// un'informazione diversa — che va detta come tale.
    NotObservable,
}
