//! La configurazione di un run: risoluzione con la precedenza di §7 e
//! **registrazione della provenienza** per il `RunManifest` (§6, v1.6).
//!
//! Le due cose stanno insieme di proposito. Se la risoluzione avvenisse in un
//! punto e la registrazione in un altro, prima o poi qualcuno aggiungerebbe una
//! chiave in uno solo dei due, e il manifest direbbe una configurazione diversa
//! da quella usata — che è peggio del non dirla affatto.

use std::path::PathBuf;

use clap::ArgMatches;
use shadow_core::ruleset::DEFAULT_ZOMBIE_STALENESS_DAYS;
use shadow_core::{ConfigSetting, ConfigSource, RunConfiguration};

/// Formato preimpostato dei log.
pub const DEFAULT_LOG_FORMAT: &str = "nginx-combined";

/// Prefisso delle variabili d'ambiente: `SHADOW_` più il nome canonico della
/// chiave di §7, in maiuscolo.
const ENV_PREFIX: &str = "SHADOW_";

/// Come deve uscire il report (§7, `output_format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Tabella leggibile su terminale.
    Terminal,
    /// Documento JSON, unico contenuto di stdout.
    Json,
    /// Documento Markdown, unico contenuto di stdout.
    Markdown,
}

impl OutputFormat {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "terminal" => Some(Self::Terminal),
            "json" => Some(Self::Json),
            "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    /// Valore canonico, come compare in §7 e nel manifest.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Json => "json",
            Self::Markdown => "markdown",
        }
    }
}

/// La configurazione risolta di un run.
pub struct Config {
    pub log_format: String,
    /// Percorso dello storico persistente (§7, Fase 5). Assente = nessuno
    /// storico, e Shadow si comporta esattamente come prima.
    pub history_path: Option<PathBuf>,
    /// Bersaglio d'analisi a cui appartiene il run (§7, Fase 5).
    pub target: String,
    /// Prefisso che delimita la parte di traffico da analizzare (§7). Assente =
    /// tutto il traffico è l'API, che è il comportamento di sempre.
    pub path_prefix: Option<String>,
    pub openapi_spec: Option<PathBuf>,
    pub zombie_staleness_days: i64,
    pub redaction_enabled: bool,
    pub allowlist_path: Option<PathBuf>,
    pub output_format: OutputFormat,
    pub fail_on_undetermined: bool,
    /// La stessa configurazione, con la provenienza di ogni valore, pronta per
    /// il `RunManifest` (§6).
    pub recorded: RunConfiguration,
}

/// Raccoglie valore e provenienza mentre si risolve, così che le due cose non
/// possano divergere.
struct Resolver<'a> {
    matches: &'a ArgMatches,
    recorded: RunConfiguration,
}

impl<'a> Resolver<'a> {
    fn new(matches: &'a ArgMatches) -> Self {
        Self {
            matches,
            recorded: RunConfiguration::default(),
        }
    }

    /// Precedenza di §7: flag > variabile d'ambiente > file di config > default.
    ///
    /// Il livello "file di config" non esiste ancora — nessun formato è stato
    /// deciso — e quando esisterà si inserisce qui, senza toccare il resto.
    fn resolve(&mut self, key: &str, default: String) -> Result<String, String> {
        let (value, source) = match self.matches.get_one::<String>(key) {
            Some(flag) => (flag.clone(), ConfigSource::CommandLine),
            None => match std::env::var(env_key(key)) {
                Ok(value) => (value, ConfigSource::Environment),
                Err(std::env::VarError::NotUnicode(_)) => {
                    return Err(not_unicode(&env_key(key)))
                }
                Err(_) => (default, ConfigSource::Default),
            },
        };
        self.record(key, &value, source);
        Ok(value)
    }

    /// Risolve una chiave booleana di §7.
    ///
    /// # Perché può fallire
    ///
    /// Prima questa funzione trattava come `false` **qualunque** valore che non
    /// fosse `1` o `true`. Quindi `SHADOW_FAIL_ON_UNDETERMINED=yes` **spegneva**
    /// il gate invece di accenderlo, in silenzio, e il manifest registrava
    /// `false` da `environment` senza un avviso: chi l'aveva scritto credeva di
    /// aver acceso una condizione di fallimento in CI e aveva ottenuto il
    /// contrario. È il falso negativo silenzioso che §P1 chiama il fallimento
    /// peggiore del prodotto, su una chiave di sicurezza.
    ///
    /// Ora un valore che non si riconosce **ferma il run** (§P2), come già fa
    /// `output_format` con un formato che non esiste. L'insieme dei valori
    /// ammessi è deliberatamente corto e senza varianti di maiuscole: i valori
    /// canonici di §7 sono scritti in una forma sola, e indovinare quale altra
    /// forma l'utente intendesse sarebbe di nuovo interpretare al posto suo.
    fn resolve_flag(
        &mut self,
        key: &str,
        recorded_key: &str,
        default: bool,
        invert: bool,
    ) -> Result<bool, String> {
        let (raw, source) = if self.matches.get_flag(key) {
            (!invert, ConfigSource::CommandLine)
        } else {
            match std::env::var(env_key(key)) {
                Ok(value) => (
                    parse_bool(&value, &env_key(key))? != invert,
                    ConfigSource::Environment,
                ),
                // `std::env::var` mette nello stesso `Err` "non c'è" e "c'è ma
                // non è UTF-8". Confonderli farebbe scivolare sul default un
                // valore che l'utente ha davvero impostato, e il manifest
                // scriverebbe `default` su una chiave che l'ambiente stava
                // cambiando: lo stesso silenzio di §P2 che questa funzione
                // esiste per chiudere, un passo più in là.
                Err(std::env::VarError::NotUnicode(_)) => {
                    return Err(not_unicode(&env_key(key)))
                }
                Err(_) => (default, ConfigSource::Default),
            }
        };
        self.record(recorded_key, &raw.to_string(), source);
        Ok(raw)
    }

    fn record(&mut self, key: &str, value: &str, source: ConfigSource) {
        self.recorded.settings.insert(
            key.to_string(),
            ConfigSetting {
                value: value.to_string(),
                source,
            },
        );
    }
}

/// I valori canonici di una chiave booleana di §7.
///
/// Non sono insensibili alle maiuscole e non hanno sinonimi: `yes`, `on`,
/// `TRUE` non sono ammessi, e chi li scrive lo scopre subito invece di
/// scoprirlo da un report che non segnala niente.
const TRUE_VALUES: &[&str] = &["true", "1"];
const FALSE_VALUES: &[&str] = &["false", "0"];

/// Il messaggio per una variabile d'ambiente che esiste ma non è testo valido.
///
/// Non si stampa il valore: non essendo UTF-8 non c'è modo di mostrarlo senza
/// inventarselo, ed è esattamente il genere di conversione tollerante che §P2
/// vieta sui dati.
fn not_unicode(shown_key: &str) -> String {
    format!(
        "{shown_key} is set but is not valid text, so its value cannot be read. \
         It is refused instead of ignored: falling back to the default would record \
         'default' in the manifest for a key the environment was actually setting"
    )
}

/// Legge un valore booleano, o dice perché non lo è. Messaggio in inglese (§5).
fn parse_bool(value: &str, shown_key: &str) -> Result<bool, String> {
    if TRUE_VALUES.contains(&value) {
        return Ok(true);
    }
    if FALSE_VALUES.contains(&value) {
        return Ok(false);
    }
    Err(format!(
        "{shown_key} must be one of: {} (true) or {} (false), got '{value}'. \
         It is refused instead of ignored: a value that silently means 'false' would turn a \
         switch off while you believed you had turned it on",
        TRUE_VALUES.join(", "),
        FALSE_VALUES.join(", ")
    ))
}

/// Risolve le due chiavi di §7 che servono a `shadow alerts`, con la **stessa
/// precedenza** di tutte le altre: flag > variabile d'ambiente > default.
///
/// Esiste perché il sottocomando ha un proprio insieme di argomenti e non può
/// passare da [`resolve`], che li cerca tutti. Senza, `SHADOW_TARGET` valeva per
/// un'analisi e non per la lettura degli alert — cioè la precedenza di §7 aveva
/// un'eccezione non scritta.
/// Restituisce anche **se il percorso è stato digitato in questa invocazione**:
/// è la sola cosa che secondo §P5 autorizza a mostrarlo in chiaro in un
/// messaggio. Un percorso arrivato dall'ambiente resta mascherato, qui come nel
/// percorso d'analisi.
pub fn resolve_for_alerts(matches: &ArgMatches) -> Result<(PathBuf, bool, String), String> {
    let (path, typed) = match matches.get_one::<String>("history_path") {
        Some(flag) => (flag.clone(), true),
        None => (
            std::env::var(env_key("history_path")).map_err(|_| {
                "no history given: pass --history <FILE> or set SHADOW_HISTORY_PATH".to_string()
            })?,
            false,
        ),
    };
    let target = match matches.get_one::<String>("target") {
        Some(flag) => flag.clone(),
        None => std::env::var(env_key("target"))
            .unwrap_or_else(|_| shadow_history::Target::DEFAULT.to_string()),
    };
    Ok((PathBuf::from(path), typed, target))
}

fn env_key(key: &str) -> String {
    format!("{ENV_PREFIX}{}", key.to_ascii_uppercase())
}

/// Risolve la configurazione, o spiega perché non è valida.
pub fn resolve(matches: &ArgMatches) -> Result<Config, String> {
    let mut resolver = Resolver::new(matches);

    let log_format = resolver.resolve("log_format", DEFAULT_LOG_FORMAT.to_string())?;
    let openapi_spec = resolver.resolve("openapi_spec", String::new())?;
    let staleness = resolver.resolve(
        "zombie_staleness_days",
        DEFAULT_ZOMBIE_STALENESS_DAYS.to_string(),
    )?;
    let allowlist_path = resolver.resolve("allowlist_path", String::new())?;
    let history_path = resolver.resolve("history_path", String::new())?;
    let path_prefix = resolver.resolve("path_prefix", String::new())?;
    let target = resolver.resolve("target", shadow_history::Target::DEFAULT.to_string())?;
    let output_format =
        resolver.resolve("output_format", OutputFormat::Terminal.as_str().to_string())?;
    // `--show-raw-values` è la deroga a §P5, quindi il flag **spegne** una
    // chiave che di suo è accesa: il manifest registra la chiave di §7
    // (`redaction_enabled`), non il nome del flag.
    let redaction_enabled =
        resolver.resolve_flag("show_raw_values", "redaction_enabled", true, true)?;
    let fail_on_undetermined =
        resolver.resolve_flag("fail_on_undetermined", "fail_on_undetermined", false, false)?;

    let zombie_staleness_days = staleness.parse::<i64>().ok().filter(|days| *days >= 0).ok_or_else(|| {
        format!("zombie_staleness_days must be a non-negative whole number, got '{staleness}'")
    })?;

    let output_format = OutputFormat::parse(&output_format).ok_or_else(|| {
        format!("unknown output_format '{output_format}'. Supported: terminal, json, markdown")
    })?;

    // Il nome del bersaglio si valida qui, con gli altri valori: uno storico
    // non deve aprirsi per scoprire poi che l'etichetta non era usabile.
    shadow_history::Target::new(&target).map_err(|e| e.to_string())?;

    // Un prefisso che non comincia con `/` non potrebbe combaciare con nessun
    // path, mai: accettarlo significherebbe analizzare zero richieste e non
    // dirlo. Si rifiuta (§P2).
    if !path_prefix.is_empty() && !path_prefix.starts_with('/') {
        return Err(format!(
            "path_prefix must start with '/', got '{path_prefix}'. \
             A prefix that cannot match any path would silently scope the analysis down to nothing"
        ));
    }

    Ok(Config {
        log_format,
        history_path: non_empty(history_path),
        target,
        path_prefix: if path_prefix.is_empty() {
            None
        } else {
            Some(path_prefix)
        },
        openapi_spec: non_empty(openapi_spec),
        zombie_staleness_days,
        redaction_enabled,
        allowlist_path: non_empty(allowlist_path),
        output_format,
        fail_on_undetermined,
        recorded: resolver.recorded,
    })
}

impl Config {
    /// Dice se il valore di una chiave è arrivato dalla **riga di comando in
    /// questa invocazione**.
    ///
    /// Serve all'eccezione di §P5 (v1.7): un percorso che l'utente ha appena
    /// digitato può comparire in chiaro in un messaggio diagnostico. La
    /// provenienza è già registrata per il manifest, quindi non va dedotta una
    /// seconda volta — e non possono divergere.
    pub fn typed_on_command_line(&self, key: &str) -> bool {
        self.recorded
            .settings
            .get(key)
            .is_some_and(|setting| setting.source == ConfigSource::CommandLine)
    }
}

fn non_empty(value: String) -> Option<PathBuf> {
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}
