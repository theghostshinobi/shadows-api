//! `RunManifest` — la carta d'identità del singolo run (§5, §6).

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use super::finding::Classification;

/// `RunManifest` — metadati del singolo run (§5, §6).
///
/// Senza manifest non c'è audit possibile: è ciò che permette di dire, a
/// distanza di mesi, con quale versione del tool e quale ruleset è stato
/// prodotto un certo verdetto, su quali input esatti, e quante righe erano
/// state scartate. È anche la controprova del determinismo (§P4): a input
/// costante, due run producono lo stesso manifest a parte il timestamp.
///
/// Il manifest viene emesso **ogni volta che un'analisi avviene**, anche quando
/// non produce nessun finding — e **mai quando un'analisi non avviene**. È il
/// vincolo "nessun manifest, nessun verdetto" di §7: un'invocazione che non
/// analizza nulla non deve poter affermare "nessun finding". Passa dal
/// `Redactor` come ogni altro output (§P5).
///
/// Stato Fase 0: scheletro. I campi ci sono tutti; chi li popola arriva con le
/// fasi successive (i conteggi di ingestione in Fase 1, i finding in Fase 3, la
/// scrittura insieme al report in Fase 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunManifest {
    /// Versione di Shadow che ha prodotto il run.
    pub shadow_version: String,

    /// Versione del ruleset usato nel run (§5, §7).
    pub ruleset_version: RulesetVersion,

    /// Hash SHA-256 dei file di input (log e OpenAPI), ciascuno con
    /// l'algoritmo registrato accanto (§6).
    pub inputs: Vec<InputDigest>,

    /// Conteggi del run (§6).
    pub counts: RunCounts,

    /// Configurazione **effettiva** del run: ogni chiave di §7 con il valore
    /// realmente usato e la sua provenienza (§6, v1.6).
    pub configuration: RunConfiguration,

    /// Timestamp del run. È l'**unico** campo che può legittimamente differire
    /// fra due run sullo stesso input (§P4).
    pub run_timestamp: DateTime<Utc>,
}

/// `RulesetVersion` — versione dell'insieme di regole/euristiche usato in un run (§5).
///
/// Il ruleset comprende le soglie e gli elenchi globali di §7 (soglia di
/// variabilità dei segmenti di path, campi sensibili per il `Redactor`, pattern
/// dei path sospetti). Cambiare una di quelle regole cambia i verdetti: per
/// questo la versione viaggia dentro il manifest.
///
/// Il valore corrente è [`crate::RULESET_VERSION`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RulesetVersion(String);

impl RulesetVersion {
    /// La versione del ruleset compilata in questo binario
    /// ([`crate::RULESET_VERSION`]).
    pub fn current() -> Self {
        Self(crate::RULESET_VERSION.to_string())
    }

    /// Rappresentazione testuale della versione.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RulesetVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hash di un file di input, per la riproducibilità del run (§6).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputDigest {
    /// Percorso del file di input.
    ///
    /// Come [`SourceRef::file`](super::SourceRef::file), è un dato **soggetto a
    /// redazione** (§P5): il manifest è il file che si consegna all'auditor, e
    /// un percorso rivela cliente e struttura interna quanto un token rivela
    /// una credenziale.
    pub path: PathBuf,

    /// Ruolo del file nell'analisi.
    pub role: InputRole,

    /// Algoritmo con cui il digest è stato calcolato (§6).
    ///
    /// Sta qui, accanto al digest, e non altrove nel manifest: un digest non
    /// deve mai poter essere letto senza sapere come è stato prodotto.
    pub algorithm: DigestAlgorithm,

    /// Digest del **contenuto** del file, in esadecimale minuscolo.
    ///
    /// Il calcolo è deliverable della **Fase 1** e avviene nella stessa passata
    /// in cui il file viene letto a flusso, mai con una seconda lettura (§P10).
    /// Qui c'è solo il risultato, con il suo [`DigestAlgorithm`] accanto.
    pub digest: String,
}

/// Algoritmo di hash usato per un [`InputDigest`] (§6).
///
/// Oggi ce n'è uno solo — SHA-256, fissato in §6 — e ciò nonostante viaggia
/// registrato dentro il manifest: se un domani cambiasse, i manifest già
/// prodotti resterebbero interpretabili senza doverlo indovinare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DigestAlgorithm {
    /// SHA-256, calcolato sul contenuto del file.
    Sha256,
}

impl DigestAlgorithm {
    /// Nome canonico dell'algoritmo, così come compare nel manifest.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

/// Ruolo di un file di input all'interno di un run (§6: "log e OpenAPI").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputRole {
    /// File di log da cui si ricava l'`ObservedInventory`.
    Log,

    /// Specifica OpenAPI/Swagger da cui si ricava il `DeclaredInventory`.
    OpenApiSpec,
}

/// La configurazione **effettiva** di un run (§6, v1.6).
///
/// Registra ogni chiave di configurazione di §7 con il valore realmente usato e
/// **da dove arriva**. Senza, due report divergenti sullo stesso servizio non
/// permettono di distinguere se è cambiato il traffico o la configurazione: è lo
/// stesso buco d'audit che l'hash degli input aveva chiuso per i file.
///
/// Le chiavi sono i **nomi canonici** di §7 (`log_format`, `openapi_spec`,
/// `zombie_staleness_days`, …), non i nomi dei flag: il flag è solo uno dei modi
/// per fornire il valore, e il manifest registra la configurazione, non
/// l'invocazione.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunConfiguration {
    /// Chiave canonica di §7 → valore usato e provenienza. Mappa ordinata per
    /// determinismo (§P4).
    pub settings: BTreeMap<String, ConfigSetting>,
}

/// Il valore effettivo di una chiave di configurazione, con la sua provenienza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSetting {
    /// Il valore realmente usato, reso come testo.
    ///
    /// Passa dal `Redactor` come ogni altro contenuto del manifest (§P5): il
    /// valore di `openapi_spec` è un percorso di filesystem, e un percorso
    /// rivela cliente e struttura interna.
    pub value: String,

    /// Da dove è arrivato. La provenienza conta quanto il valore: "90 giorni
    /// perché c'era un file di config che nessuno ricordava" è informazione
    /// d'audit.
    pub source: ConfigSource,
}

/// Da dove viene il valore di una chiave di configurazione (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigSource {
    /// Flag della riga di comando: il livello di precedenza più alto.
    CommandLine,
    /// Variabile d'ambiente.
    Environment,
    /// File di configurazione.
    ConfigFile,
    /// Nessuno l'ha fornito: è il valore preimpostato.
    Default,
}

impl ConfigSource {
    /// Nome canonico della provenienza, così come compare nel manifest.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommandLine => "command-line",
            Self::Environment => "environment",
            Self::ConfigFile => "config-file",
            Self::Default => "default",
        }
    }
}

/// Conteggi di un run (§6).
///
/// Le righe scartate e il **perché** sono parte del contratto, non un dettaglio
/// diagnostico: un run che scarta metà del file senza dirlo è un run che mente
/// per omissione (§P2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunCounts {
    /// Righe totali lette dagli input.
    pub total_lines: u64,

    /// Righe parsate con successo in una
    /// [`ObservedRequest`](super::ObservedRequest).
    pub parsed_lines: u64,

    /// Righe scartate.
    pub discarded_lines: u64,

    /// Righe scartate, per motivo. Mappa ordinata per determinismo (§P4).
    /// I motivi possibili nascono con l'ingestione, in Fase 1.
    ///
    /// Le chiavi sono stringhe **in inglese**: finiscono nel manifest, che
    /// l'utente legge (§5).
    pub discarded_by_reason: BTreeMap<String, u64>,

    /// Numero di endpoint trovati (§6).
    ///
    /// Cosa sia un "endpoint" dipende da quanto in là arriva la pipeline: in
    /// Fase 1 sono le **richieste osservate distinte**, che è ciò che il
    /// cancello di quella fase chiama "endpoint osservati"; dalla Fase 2 sono
    /// gli [`EndpointPattern`](super::EndpointPattern), e per lo stesso input
    /// il numero cambierà — è così che si vede che la normalizzazione è
    /// entrata in funzione.
    pub endpoints_found: u64,

    /// Finding per categoria (§6). La chiave è il verdetto, il valore il
    /// conteggio.
    ///
    /// I finding silenziati dall'`Allowlist` restano contati qui: l'allowlist
    /// silenzia il rumore noto, non cancella l'evidenza (§5, Fase 4).
    pub findings_by_classification: BTreeMap<Classification, u64>,
}
