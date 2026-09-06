//! `ObservedRequest` — l'unità atomica del modello (§5, §6).

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

/// `ObservedRequest` — una singola richiesta estratta da **una riga di log** (§5).
///
/// È il prodotto di un `Collector` (Fase 1) e l'input della normalizzazione
/// (Fase 2). I campi conservano il dato **grezzo** com'era nel log: qui non si
/// normalizza, non si decodifica e non si interpreta nulla. Ogni
/// trasformazione appartiene a una fase successiva, così che l'evidenza
/// originale resti sempre ricostruibile (§P2).
///
/// I contenuti che possono trasportare segreti o dati personali
/// (valori di query in primis) passano dal `Redactor` prima di qualunque
/// stampa o persistenza (§P5, Fase 4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObservedRequest {
    /// Metodo HTTP così come compare nel log, senza normalizzazione.
    pub method: String,

    /// Path **grezzo**, esattamente come si presentava nella riga di log:
    /// nessuna decodifica percentuale, nessuna normalizzazione dei segmenti.
    ///
    /// La normalizzazione conservativa (che produce l'`EndpointPattern`) è
    /// Fase 2; qui il dato resta intatto perché è l'evidenza di partenza.
    pub raw_path: String,

    /// Parametri di query: chiavi e valori **grezzi** (§6).
    ///
    /// Una chiave che compare più volte conserva tutti i suoi valori: nessun
    /// dato viene perso in silenzio (§P2). La mappa è ordinata
    /// ([`BTreeMap`]) perché lo stesso input deve produrre lo stesso output,
    /// iterazione compresa (§P4).
    pub query_params: BTreeMap<String, Vec<String>>,

    /// Presenza e tipo dell'header di autenticazione — **mai il valore** del
    /// token (§6). Vedi [`ObservedAuth`].
    pub auth: ObservedAuth,

    /// Status code della risposta registrato nel log.
    pub status_code: u16,

    /// Timestamp della richiesta, normalizzato a UTC per rendere confrontabili
    /// righe provenienti da fusi diversi (serve alla finestra di staleness
    /// dello `Zombie`, §7).
    pub timestamp: DateTime<Utc>,

    /// Provenienza della richiesta (file + numero di riga), per tracciabilità (§6).
    pub source: SourceRef,
}

/// Presenza e tipo dell'autenticazione osservata su una [`ObservedRequest`] (§6).
///
/// Il valore del token non entra **mai** in questo tipo: si registra che
/// l'header c'era e di che tipo era, non cosa conteneva (§6, §P5).
///
/// La distinzione fra [`Self::NotObservable`] e [`Self::Absent`] è deliberata e
/// discende da §P2/§P9: un formato di log che non trasporta affatto
/// l'informazione di autenticazione (il caso comune del `combined` di nginx)
/// **non** è una prova che l'autenticazione mancasse. Collassare i due casi
/// significherebbe produrre un dato plausibile ma inventato — esattamente il
/// fallimento che il blueprint vieta.
///
/// Come questi tre stati diventano il verdetto di un
/// [`EndpointPattern`](super::EndpointPattern) è fissato dalla regola di
/// propagazione documentata su
/// [`AuthObservation`](super::AuthObservation) (§6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObservedAuth {
    /// Il formato di log non trasporta l'informazione: non si può dire nulla.
    /// **Non** equivale ad "autenticazione assente".
    NotObservable,

    /// Il formato trasporta l'informazione e l'header di autenticazione non c'era.
    Absent,

    /// Il formato trasporta l'informazione e l'header di autenticazione c'era.
    Present {
        /// Tipo/schema dell'header così come osservato (es. `Bearer`, `Basic`),
        /// oppure [`ObservedAuth::SCHEME_UNSPECIFIED`] quando il formato dice
        /// che l'autenticazione c'era ma non dice quale schema fosse.
        /// Contiene solo lo schema, mai il credenziale che lo segue.
        scheme: String,
    },
}

impl ObservedAuth {
    /// Schema canonico da usare quando l'autenticazione è **osservata** ma il
    /// formato di log non dice quale schema fosse (§6, v1.4).
    ///
    /// Serve a tenere separati un fatto e un'inferenza. Scrivere `Basic` perché
    /// è lo schema più probabile significherebbe far dire al campo qualcosa che
    /// non è stato osservato — e il campo si chiama `scheme`, non
    /// `probable_scheme` (§P2). Il fatto che conta, cioè che l'autenticazione
    /// ci fosse, resta interamente conservato.
    pub const SCHEME_UNSPECIFIED: &'static str = "unspecified";
}

/// Riferimento alla provenienza di una [`ObservedRequest`] (§6).
///
/// Serve alla tracciabilità: ogni dato mostrato all'utente deve poter essere
/// ricondotto alla riga esatta che l'ha generato.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceRef {
    /// File di log da cui proviene la riga.
    ///
    /// È un dato **soggetto a redazione** (§P5): un percorso come
    /// `/home/utente/clienti/bancaXYZ/logs/access.log` rivela il nome del
    /// cliente e la struttura interna dell'organizzazione, e viaggia dritto nel
    /// report che si consegna a un terzo.
    pub file: PathBuf,

    /// Numero di riga all'interno del file, **a partire da 1** (come lo conta
    /// un essere umano che apre il file in un editor).
    pub line_number: u64,
}
