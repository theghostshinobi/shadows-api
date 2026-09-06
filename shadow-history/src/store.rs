//! Apertura, scrittura e interrogazione dello storico.

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension};
use shadow_core::Classification;

use crate::schema::{self, SCHEMA_VERSION};

/// Quanto si aspetta che un altro processo liberi il database prima di
/// arrendersi.
///
/// Due esecuzioni concorrenti sullo stesso storico sono una fixture avversaria
/// obbligatoria (§10): senza attesa la seconda fallirebbe subito, con
/// un'attesa infinita si pianterebbe. Si aspetta un po' e poi si dice cosa è
/// successo.
const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Il bersaglio d'analisi a cui un run appartiene.
///
/// «Multi-tenancy» in uno strumento CLI-first e offline significa **separazione
/// dei bersagli** (decisione del fondatore, 2026-09-05): un demone sorveglia
/// più servizi, ciascuno con il proprio inventario, e i due inventari non si
/// mescolano mai. Non significa account, ruoli o login: quelli implicherebbero
/// un server, e contraddirebbero §2.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target(String);

impl Target {
    /// Nome del bersaglio usato quando l'utente non ne indica uno.
    pub const DEFAULT: &'static str = "default";

    /// Costruisce un bersaglio, rifiutando un nome vuoto o con spazi ai bordi.
    ///
    /// Il nome finisce in un file che viaggia, quindi non ci si mettono percorsi
    /// né altro che venga dai dati analizzati: è un'etichetta che sceglie chi
    /// esegue (§P5).
    pub fn new(name: &str) -> Result<Self, HistoryError> {
        if name.is_empty() || name.trim() != name {
            return Err(HistoryError::InvalidTarget(name.to_string()));
        }
        Ok(Self(name.to_string()))
    }

    /// Il nome, come compare nello storico e nei messaggi.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Target {
    fn default() -> Self {
        Self(Self::DEFAULT.to_string())
    }
}

/// Un endpoint osservato, nella forma **già redatta** in cui va persistito.
///
/// Il tipo chiede i valori redatti invece di redigerli qui: `shadow-history` non
/// conosce il `Redactor`, che vive nel core, e duplicarne la logica
/// significherebbe avere due regole di mascheramento che possono divergere
/// (§P7). Chi chiama è la CLI, che il `Redactor` ce l'ha già in mano.
#[derive(Debug, Clone)]
pub struct ObservedEndpointRecord {
    /// Identificatore stabile dell'endpoint (§6, Fase 2).
    pub id: String,
    /// Pattern del path, **già passato dal `Redactor`**.
    pub redacted_pattern: String,
    /// Metodi HTTP osservati, già in ordine deterministico.
    pub methods: Vec<String>,
    /// Quante richieste sono confluite in questo endpoint.
    pub observation_count: u64,
    /// Stato dell'autenticazione, uno dei **quattro** valori di §6, scritto
    /// così com'è: `yes`, `no`, `mixed`, `not observable`. Mai un binario.
    pub auth_observed: String,
    /// Su quante richieste **osservabili** quel verdetto si regge. Senza questo
    /// numero, «senza auth» e «senza auth, su 3 richieste su 5000» si
    /// assomigliano (§6).
    pub auth_observable_count: u64,
    /// Come il run corrente ha classificato questo endpoint, **se** l'ha
    /// classificato.
    ///
    /// `None` non è un dato mancante, è un'informazione: un run senza
    /// inventario dichiarato che non trova niente di sospetto **non dice
    /// niente** su quell'endpoint, e §9 lo vuole così («un inventario tranquillo
    /// produce zero righe»). Scriverci `Known` sarebbe inventare il verdetto che
    /// il tool si è rifiutato di dare; scriverci `Undetermined` sarebbe
    /// inventare un sospetto che nessuno ha avuto. Lo storico ricorda **che
    /// l'endpoint c'era**, che è ciò che serve a riconoscere il nuovo.
    pub classification: Option<Classification>,
}

/// Un finding, nella forma già redatta in cui va persistito.
#[derive(Debug, Clone)]
pub struct FindingRecord {
    /// Identificatore dell'endpoint osservato, se il soggetto è osservato.
    pub endpoint_id: Option<String>,
    /// Pattern dichiarato **già redatto**, se il soggetto è dichiarato.
    pub declared_path: Option<String>,
    /// Classificazione, confidenza e severità come stringhe canoniche (§5).
    pub classification: String,
    /// Livello di confidenza.
    pub confidence: String,
    /// Severità suggerita.
    pub severity: String,
    /// Righe di evidenza **già redatte**, unite da un ritorno a capo.
    pub evidence: String,
}

/// Cosa un run ha aggiunto allo storico.
#[derive(Debug, Clone)]
pub struct RecordedRun {
    /// Identificatore progressivo del run appena registrato.
    pub run_id: i64,
    /// Gli endpoint **mai visti prima** su questo bersaglio, in ordine
    /// deterministico (§P4).
    pub new_endpoints: Vec<EndpointState>,
    /// Vero se il timestamp di questo run precede quello del run registrato
    /// prima. Non si aggiusta niente: si dice.
    pub clock_went_backwards: bool,
}

/// Lo stato di un endpoint nello storico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointState {
    /// Identificatore stabile dell'endpoint.
    pub id: String,
    /// Pattern già redatto.
    pub redacted_pattern: String,
    /// L'ultima classificazione registrata, se il run ne ha data una.
    pub last_classification: Option<String>,
    /// Il run in cui è comparso per la prima volta.
    pub first_seen_run: i64,
    /// L'ultimo run in cui è stato osservato.
    pub last_seen_run: i64,
    /// Se qualcuno l'ha già preso in carico.
    pub acknowledged: bool,
}

/// Una riga dell'inventario, nella forma che serve a un **report d'audit**.
///
/// Porta i quattro valori dell'autenticazione e la loro base di calcolo, perché
/// §9 vieta di collassarli in un binario proprio qui: *«un report d'audit che
/// dichiara "senza auth" dove il dato non era osservabile è peggio di un report
/// che tace»*.
#[derive(Debug, Clone)]
pub struct InventoryRow {
    /// Identificatore stabile dell'endpoint.
    pub id: String,
    /// Pattern già redatto.
    pub redacted_pattern: String,
    /// Metodi HTTP osservati.
    pub methods: Vec<String>,
    /// Osservazioni complessive.
    pub observation_count: u64,
    /// Ultima classificazione registrata, se un run ne ha data una.
    pub last_classification: Option<String>,
    /// Stato dell'autenticazione: `yes`, `no`, `mixed`, `not observable`, o
    /// `None` se la riga è stata scritta prima che lo storico lo registrasse —
    /// che **non** è «non osservabile», è una terza cosa.
    pub auth_observed: Option<String>,
    /// Su quante richieste osservabili quel verdetto si regge.
    pub auth_observable_count: Option<u64>,
    /// Run in cui è comparso per la prima volta e in cui è stato visto l'ultima.
    pub first_seen_run: i64,
    /// Ultimo run in cui è stato osservato.
    pub last_seen_run: i64,
    /// Se ha fatto scattare un alert non ancora preso in carico.
    pub pending_alert: bool,
}

/// Un run registrato, come lo vede chi supervisiona.
#[derive(Debug, Clone)]
pub struct RunSummary {
    /// Identificatore progressivo.
    pub id: i64,
    /// Timestamp dichiarato dal run. **Non** decide l'ordine: quello lo decide
    /// `id`, ed è la difesa contro l'orologio che va all'indietro.
    pub timestamp: String,
    /// Il manifest redatto, così com'è stato scritto.
    pub manifest_json: String,
}

/// Perché lo storico non è utilizzabile.
#[derive(Debug)]
pub enum HistoryError {
    /// Il file non si apre, è corrotto, o è bloccato da un altro processo.
    ///
    /// I tre casi restano distinti nel messaggio, perché la cosa da fare è
    /// diversa: aspettare, ripristinare un backup, o controllare il percorso.
    Sqlite {
        /// Percorso del database, **già redatto** da chi chiama (§P5).
        shown_path: String,
        /// L'errore riportato da SQLite.
        source: rusqlite::Error,
    },

    /// Il database è stato scritto da una versione di Shadow più recente.
    SchemaTooNew {
        /// Percorso del database, già redatto.
        shown_path: String,
        /// Versione trovata nel file.
        found: i64,
        /// Versione che questo binario conosce.
        supported: i64,
    },

    /// Il percorso è un URI di SQLite, non un file.
    UriPath {
        /// Percorso del database, già redatto.
        shown_path: String,
    },

    /// Lo storico che si voleva **leggere** non esiste.
    Missing {
        /// Percorso del database, già redatto.
        shown_path: String,
    },

    /// Il nome del bersaglio non è utilizzabile.
    InvalidTarget(String),
}

impl fmt::Display for HistoryError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite { shown_path, source } => {
                let hint = match source {
                    rusqlite::Error::SqliteFailure(error, _)
                        if error.code == rusqlite::ErrorCode::DatabaseBusy
                            || error.code == rusqlite::ErrorCode::DatabaseLocked =>
                    {
                        "another Shadow run is holding it; wait for it to finish, or use a separate history file"
                    }
                    rusqlite::Error::SqliteFailure(error, _)
                        if error.code == rusqlite::ErrorCode::DatabaseCorrupt
                            || error.code == rusqlite::ErrorCode::NotADatabase =>
                    {
                        "it is not a readable Shadow history; restore a backup, or point --history at a new file"
                    }
                    _ => "check the path and that the file is writable",
                };
                write!(f, "cannot use the history {shown_path}: {source}\n  {hint}")
            }
            Self::SchemaTooNew {
                shown_path,
                found,
                supported,
            } => write!(
                f,
                "the history {shown_path} has schema version {found}, and this build knows {supported}.\n  \
                 It is refused instead of opened: reading or writing a layout this build does not \
                 know would corrupt an audit archive"
            ),
            Self::UriPath { shown_path } => write!(
                f,
                "the history path {shown_path} starts with 'file:', which SQLite reads as a URI \
                 and not as a file name.\n  \
                 It is refused because a URI can open a database that lives only in memory: \
                 Shadow would say it remembers and remember nothing, and every run would find \
                 everything new. Give a plain path"
            ),
            Self::Missing { shown_path } => write!(
                f,
                "there is no history at {shown_path}.\n  \
                 It is refused instead of created empty: answering 'nothing to report' about a \
                 file that was never there would turn a typo into an all-clear"
            ),
            Self::InvalidTarget(name) => write!(
                f,
                "'{name}' is not a usable target name: it must not be empty or padded with spaces"
            ),
        }
    }
}

/// Lo storico su SQLite.
pub struct HistoryStore {
    connection: Connection,
    shown_path: String,
}

impl HistoryStore {
    /// Apre (o crea) lo storico e lo porta allo schema corrente.
    ///
    /// `shown_path` è il percorso **già redatto** da mostrare nei messaggi: qui
    /// non si sa se l'utente l'ha digitato in questa invocazione, e quella è
    /// l'unica cosa che secondo §P5 autorizza a mostrarlo in chiaro.
    ///
    /// Si apre **presto**, prima di leggere il log: se lo storico non è
    /// utilizzabile è meglio saperlo prima di macinare un gigabyte.
    pub fn open(path: &Path, shown_path: String) -> Result<Self, HistoryError> {
        Self::open_with(path, shown_path, true)
    }

    /// Apre uno storico che **deve già esistere**.
    ///
    /// La differenza non è pignoleria. `shadow alerts` legge una memoria: se
    /// creasse il file quando il percorso è sbagliato, a un errore di battitura
    /// risponderebbe «nessun endpoint shadow da prendere in carico» e uscirebbe
    /// con successo — cioè direbbe *tutto a posto* proprio quando non ha
    /// guardato niente. È il falso negativo silenzioso di §P1, prodotto da una
    /// lettera sbagliata.
    pub fn open_existing(path: &Path, shown_path: String) -> Result<Self, HistoryError> {
        if !path.exists() {
            return Err(HistoryError::Missing { shown_path });
        }
        Self::open_with(path, shown_path, false)
    }

    fn open_with(path: &Path, shown_path: String, create: bool) -> Result<Self, HistoryError> {
        // Togliere `SQLITE_OPEN_URI` dai flag **non basta**: SQLite compilato
        // con `SQLITE_USE_URI` — come lo è quello incluso nel crate — interpreta
        // comunque un nome che comincia per `file:`. Quindi si rifiuta il
        // valore, rumorosamente, invece di lasciare che apra un database
        // volatile (§P2).
        // Il controllo è sull'**intera** stringa, come lo fa SQLite: un
        // `file:/tmp/x.db?mode=memory` ha un ultimo segmento innocuo e resta un
        // URI.
        if path.to_string_lossy().starts_with("file:") {
            return Err(HistoryError::UriPath { shown_path });
        }
        let sqlite = |source| HistoryError::Sqlite {
            shown_path: shown_path.clone(),
            source,
        };
        // Flag espliciti, e **senza `SQLITE_OPEN_URI`**, che rusqlite accende
        // per impostazione predefinita. Con gli URI accesi un `history_path`
        // come `file::memory:` apriva uno storico in memoria che spariva a fine
        // processo: il tool avrebbe detto di ricordare senza ricordare niente, e
        // ogni run avrebbe trovato tutto nuovo. Il valore di una chiave di §7 è
        // un percorso, non un piccolo linguaggio.
        let mut flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        if create {
            flags |= rusqlite::OpenFlags::SQLITE_OPEN_CREATE;
        }
        let mut connection = Connection::open_with_flags(path, flags).map_err(sqlite)?;
        connection
            .busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS.into()))
            .map_err(sqlite)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(sqlite)?;

        let version = schema::migrate(&mut connection).map_err(sqlite)?;
        if version != SCHEMA_VERSION {
            return Err(HistoryError::SchemaTooNew {
                shown_path,
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(Self {
            connection,
            shown_path,
        })
    }

    fn sqlite(&self, source: rusqlite::Error) -> HistoryError {
        HistoryError::Sqlite {
            shown_path: self.shown_path.clone(),
            source,
        }
    }

    /// Registra un run e dice quali endpoint non erano mai stati visti.
    ///
    /// Tutto dentro **una transazione**: o lo storico avanza per intero, o resta
    /// dov'era. Due esecuzioni concorrenti sullo stesso file si serializzano —
    /// una aspetta l'altra fino a [`BUSY_TIMEOUT_MS`] — e nessuna delle due può
    /// vedere metà del lavoro dell'altra.
    #[allow(clippy::too_many_arguments)]
    pub fn record_run(
        &mut self,
        target: &Target,
        run_timestamp: &str,
        shadow_version: &str,
        ruleset_version: &str,
        redacted_manifest_json: &str,
        endpoints: &[ObservedEndpointRecord],
        findings: &[FindingRecord],
    ) -> Result<RecordedRun, HistoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| HistoryError::Sqlite {
                shown_path: self.shown_path.clone(),
                source: e,
            })?;

        let shown_path = self.shown_path.clone();
        let sqlite = |source| HistoryError::Sqlite {
            shown_path: shown_path.clone(),
            source,
        };

        // L'orologio non decide l'ordine: lo decide l'identificatore
        // progressivo. Qui si guarda solo se il tempo si contraddice, per
        // poterlo dire.
        let previous_timestamp: Option<String> = transaction
            .query_row(
                "SELECT run_timestamp FROM runs WHERE target = ?1 ORDER BY id DESC LIMIT 1",
                (target.as_str(),),
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite)?;
        let clock_went_backwards = previous_timestamp
            .as_deref()
            .is_some_and(|previous| run_timestamp < previous);

        transaction
            .execute(
                "INSERT INTO runs (target, run_timestamp, shadow_version, ruleset_version, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    target.as_str(),
                    run_timestamp,
                    shadow_version,
                    ruleset_version,
                    redacted_manifest_json,
                ),
            )
            .map_err(sqlite)?;
        let run_id = transaction.last_insert_rowid();

        for finding in findings {
            transaction
                .execute(
                    "INSERT INTO run_findings
                     (run_id, endpoint_id, declared_path, classification, confidence, severity, evidence)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    (
                        run_id,
                        finding.endpoint_id.as_deref(),
                        finding.declared_path.as_deref(),
                        &finding.classification,
                        &finding.confidence,
                        &finding.severity,
                        &finding.evidence,
                    ),
                )
                .map_err(sqlite)?;
        }

        let mut new_endpoints = Vec::new();
        for endpoint in endpoints {
            let known: Option<i64> = transaction
                .query_row(
                    "SELECT first_seen_run FROM endpoints WHERE target = ?1 AND endpoint_id = ?2",
                    (target.as_str(), &endpoint.id),
                    |row| row.get(0),
                )
                .optional()
                .map_err(sqlite)?;

            let classification = endpoint.classification.map(classification_name);
            match known {
                Some(first_seen_run) => {
                    // `COALESCE`: un run che **non ha dato un verdetto** su
                    // questo endpoint non ne cancella uno precedente. Prima
                    // scriveva NULL incondizionatamente, e siccome l'alert si
                    // deduceva dall'ultima classificazione, un run senza
                    // inventario dichiarato — la modalità preimpostata —
                    // azzerava in silenzio la coda degli alert di chi la
                    // specifica ce l'aveva. Misurato eseguendo.
                    transaction
                        .execute(
                            "UPDATE endpoints
                             SET last_seen_run = ?3,
                                 last_classification = COALESCE(?4, last_classification),
                                 path_pattern = ?5,
                                 alerted = alerted | ?6,
                                 methods = ?7,
                                 observation_count = ?8,
                                 auth_observed = ?9,
                                 auth_observable_count = ?10
                             WHERE target = ?1 AND endpoint_id = ?2",
                            (
                                target.as_str(),
                                &endpoint.id,
                                run_id,
                                classification,
                                &endpoint.redacted_pattern,
                                i64::from(endpoint.classification == Some(Classification::Shadow)),
                                endpoint.methods.join(","),
                                endpoint.observation_count as i64,
                                &endpoint.auth_observed,
                                endpoint.auth_observable_count as i64,
                            ),
                        )
                        .map_err(sqlite)?;
                    let _ = first_seen_run;
                }
                None => {
                    transaction
                        .execute(
                            "INSERT INTO endpoints
                             (target, endpoint_id, path_pattern, first_seen_run, last_seen_run,
                              last_classification, alerted, methods, observation_count,
                              auth_observed, auth_observable_count)
                             VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                            (
                                target.as_str(),
                                &endpoint.id,
                                &endpoint.redacted_pattern,
                                run_id,
                                classification,
                                i64::from(endpoint.classification == Some(Classification::Shadow)),
                                endpoint.methods.join(","),
                                endpoint.observation_count as i64,
                                &endpoint.auth_observed,
                                endpoint.auth_observable_count as i64,
                            ),
                        )
                        .map_err(sqlite)?;
                    new_endpoints.push(EndpointState {
                        id: endpoint.id.clone(),
                        redacted_pattern: endpoint.redacted_pattern.clone(),
                        last_classification: classification.map(str::to_string),
                        first_seen_run: run_id,
                        last_seen_run: run_id,
                        acknowledged: false,
                    });
                }
            }
        }

        transaction.commit().map_err(sqlite)?;

        // Ordine deterministico, e non quello di lettura del log (§P4).
        new_endpoints.sort_by(|a, b| a.redacted_pattern.cmp(&b.redacted_pattern).then(a.id.cmp(&b.id)));
        Ok(RecordedRun {
            run_id,
            new_endpoints,
            clock_went_backwards,
        })
    }

    /// Gli endpoint che hanno fatto scattare un alert e **non sono ancora stati
    /// presi in carico**.
    ///
    /// È l'alert di §9 nella forma che P6 consente: una riga che si interroga,
    /// non un messaggio che parte.
    ///
    /// L'alert è un **fatto avvenuto** — questo endpoint è stato visto `Shadow`
    /// — e lo toglie solo chi lo prende in carico. Dedurlo dall'ultima
    /// classificazione lo rendeva cancellabile da un run che su quell'endpoint
    /// non aveva niente da dire.
    pub fn pending_alerts(&self, target: &Target) -> Result<Vec<EndpointState>, HistoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT endpoint_id, path_pattern, last_classification, first_seen_run, last_seen_run, acknowledged
                 FROM endpoints
                 WHERE target = ?1 AND acknowledged = 0 AND alerted = 1
                 ORDER BY path_pattern, endpoint_id",
            )
            .map_err(|e| self.sqlite(e))?;
        let rows = statement
            .query_map((target.as_str(),), |row| {
                Ok(EndpointState {
                    id: row.get(0)?,
                    redacted_pattern: row.get(1)?,
                    last_classification: row.get(2)?,
                    first_seen_run: row.get(3)?,
                    last_seen_run: row.get(4)?,
                    acknowledged: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(|e| self.sqlite(e))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| self.sqlite(e))
    }

    /// Segna un endpoint come preso in carico. Restituisce `false` se su quel
    /// bersaglio non esiste.
    pub fn acknowledge(&mut self, target: &Target, endpoint_id: &str) -> Result<bool, HistoryError> {
        let changed = self
            .connection
            .execute(
                "UPDATE endpoints SET acknowledged = 1 WHERE target = ?1 AND endpoint_id = ?2",
                (target.as_str(), endpoint_id),
            )
            .map_err(|e| self.sqlite(e))?;
        Ok(changed > 0)
    }

    /// Quanti run sono registrati su un bersaglio. Serve alla diagnostica e ai
    /// test; non entra in nessun verdetto.
    pub fn run_count(&self, target: &Target) -> Result<i64, HistoryError> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE target = ?1",
                (target.as_str(),),
                |row| row.get(0),
            )
            .map_err(|e| self.sqlite(e))
    }

    /// L'inventario completo di un bersaglio, in ordine deterministico (§P4).
    ///
    /// È la materia prima del report di compliance: **tutti** gli endpoint mai
    /// visti, non solo quelli che hanno prodotto un alert.
    pub fn inventory(&self, target: &Target) -> Result<Vec<InventoryRow>, HistoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT endpoint_id, path_pattern, methods, observation_count,
                        last_classification, auth_observed, auth_observable_count,
                        first_seen_run, last_seen_run, alerted, acknowledged
                 FROM endpoints
                 WHERE target = ?1
                 ORDER BY path_pattern, endpoint_id",
            )
            .map_err(|e| self.sqlite(e))?;
        let rows = statement
            .query_map((target.as_str(),), |row| {
                let methods: Option<String> = row.get(2)?;
                let alerted: i64 = row.get(9)?;
                let acknowledged: i64 = row.get(10)?;
                Ok(InventoryRow {
                    id: row.get(0)?,
                    redacted_pattern: row.get(1)?,
                    methods: methods
                        .filter(|m| !m.is_empty())
                        .map(|m| m.split(',').map(str::to_string).collect())
                        .unwrap_or_default(),
                    observation_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as u64,
                    last_classification: row.get(4)?,
                    auth_observed: row.get(5)?,
                    auth_observable_count: row
                        .get::<_, Option<i64>>(6)?
                        .map(|v| v.max(0) as u64),
                    first_seen_run: row.get(7)?,
                    last_seen_run: row.get(8)?,
                    pending_alert: alerted != 0 && acknowledged == 0,
                })
            })
            .map_err(|e| self.sqlite(e))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| self.sqlite(e))
    }

    /// Il timestamp e i metadati dell'ultimo run su un bersaglio, per intestare
    /// un report d'audit con ciò che l'ha prodotto.
    pub fn last_run(&self, target: &Target) -> Result<Option<(i64, String, String, String)>, HistoryError> {
        self.connection
            .query_row(
                "SELECT id, run_timestamp, shadow_version, ruleset_version
                 FROM runs WHERE target = ?1 ORDER BY id DESC LIMIT 1",
                (target.as_str(),),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| self.sqlite(e))
    }

    /// Gli ultimi run di un bersaglio, dal più recente.
    ///
    /// Ordinati per **identificatore**, non per timestamp: se l'ordine
    /// dipendesse dall'orologio, un run registrato dopo con una data più vecchia
    /// scavalcherebbe quello vero, e l'andamento racconterebbe una storia che
    /// non è successa.
    pub fn recent_runs(
        &self,
        target: &Target,
        limit: u32,
    ) -> Result<Vec<RunSummary>, HistoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, run_timestamp, manifest_json FROM runs
                 WHERE target = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| self.sqlite(e))?;
        let rows = statement
            .query_map((target.as_str(), limit), |row| {
                Ok(RunSummary {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    manifest_json: row.get(2)?,
                })
            })
            .map_err(|e| self.sqlite(e))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| self.sqlite(e))
    }

    /// Gli endpoint comparsi **per la prima volta** in un dato run.
    ///
    /// È la materia prima di «cosa è cambiato»: la sola domanda per cui si apre
    /// un cruscotto, e l'unica che oggi il demone dice su stderr, dove scorre
    /// via.
    pub fn first_seen_in(
        &self,
        target: &Target,
        run_id: i64,
    ) -> Result<Vec<EndpointState>, HistoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT endpoint_id, path_pattern, last_classification, first_seen_run,
                        last_seen_run, acknowledged
                 FROM endpoints
                 WHERE target = ?1 AND first_seen_run = ?2
                 ORDER BY path_pattern, endpoint_id",
            )
            .map_err(|e| self.sqlite(e))?;
        let rows = statement
            .query_map((target.as_str(), run_id), |row| {
                Ok(EndpointState {
                    id: row.get(0)?,
                    redacted_pattern: row.get(1)?,
                    last_classification: row.get(2)?,
                    first_seen_run: row.get(3)?,
                    last_seen_run: row.get(4)?,
                    acknowledged: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(|e| self.sqlite(e))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| self.sqlite(e))
    }

    /// Dice se un bersaglio esiste nello storico.
    ///
    /// Serve a non rispondere «nessun alert» a chi ha sbagliato a scrivere il
    /// nome del servizio: un bersaglio che non c'è non è un bersaglio tranquillo.
    pub fn has_target(&self, target: &Target) -> Result<bool, HistoryError> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE target = ?1",
                (target.as_str(),),
                |row| row.get(0),
            )
            .map_err(|e| self.sqlite(e))?;
        Ok(count > 0)
    }

    /// I bersagli presenti nello storico, in ordine deterministico.
    pub fn targets(&self) -> Result<Vec<Target>, HistoryError> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT target FROM runs ORDER BY target")
            .map_err(|e| self.sqlite(e))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| self.sqlite(e))?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| self.sqlite(e))?
            .into_iter()
            .map(Target)
            .collect())
    }
}

/// Il nome canonico di una classificazione, come compare nello storico.
///
/// Sono le stesse stringhe del report (§5): un archivio che le scrivesse
/// diversamente non sarebbe confrontabile con i report che l'hanno prodotto.
fn classification_name(classification: Classification) -> &'static str {
    classification.as_str()
}
