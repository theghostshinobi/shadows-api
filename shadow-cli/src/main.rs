//! `shadow-cli` — l'entrypoint di Shadow (§8 del blueprint).
//!
//! È l'orchestratore: legge la configurazione, invoca il `Collector` giusto,
//! passa i dati al core e produce l'output. Tutto ciò che il progetto
//! costruisce deve essere raggiungibile da qui (§P8).
//!
//! # Contratti che questo binario deve rispettare in ogni fase
//!
//! - **stdout = ciò che l'utente ha chiesto** (il report, ma anche l'aiuto di
//!   `--help` e la versione di `--version`: sono output richiesti
//!   esplicitamente, non diagnostica); **stderr = tutto ciò che non ha
//!   chiesto** (progressi, avvisi, conteggi diagnostici, errori, e l'aiuto che
//!   il tool stampa di sua iniziativa) (§7). Il contratto regge in entrambe le
//!   direzioni: `shadow ... > report.txt` deve dare un file pulito, e
//!   `shadow --help | less` deve funzionare.
//! - **Precedenza di configurazione:** flag CLI > variabile d'ambiente > file
//!   di config > default, senza eccezioni (§7).
//! - **Offline:** nessuna connessione di rete non richiesta, nessuna telemetria
//!   (§P6).
//!
//! # Contratto dei codici di uscita (§7) — valori fissati
//!
//! | Codice | Significato |
//! |---|---|
//! | `0` | **analisi eseguita e completata**, nessun finding `Shadow`/`Zombie` |
//! | `1` | errore di esecuzione (input illeggibile, formato non riconosciuto) |
//! | `2` | errore d'uso (argomenti della riga di comando non validi) |
//! | `3` | analisi eseguita e completata **ma** trovati `Shadow`/`Zombie` |
//!
//! **Nessun manifest, nessun verdetto.** I codici con semantica di finding
//! (`0` e `3`) sono emessi **solo insieme a un `RunManifest`**. Un'invocazione
//! che non analizza nulla — `--help`, `--version` — termina con successo ma
//! **non afferma "nessun finding"**: non avendo prodotto un manifest, non ha
//! prodotto nessun verdetto.
//!
//! Gli `Undetermined` non attivano il `3`, che scatta solo su `Shadow`/`Zombie`
//! conclamati (§7, Fase 3). Il `3` arriva con la Fase 3, che è la prima a
//! produrre finding classificati.
//!
//! # Stato: Fase 5 — storico, demone, alert, report d'audit
//!
//! La pipeline attraversata è: log nginx → Collector →
//! [`shadow_core::ObservedRequest`] → normalizzazione →
//! [`shadow_core::ObservedInventory`] → **classificazione** →
//! [`shadow_core::Finding`], più un [`shadow_core::RunManifest`] con conteggi,
//! digest e finding per categoria.
//!
//! Con una specifica dichiarata (`--openapi-spec`) i finding sono etichettati
//! `Shadow` / `Zombie` / `Known` / `Undetermined`; senza, sono sospetti
//! euristici, tutti `Undetermined`. I finding puntano agli `EndpointPattern`
//! della Fase 2: è così che si vede che la fase si è innestata sulla pipeline
//! esistente (§9).
//!
//! Il report esce in tre forme — terminale, JSON, Markdown — e **niente esce
//! senza passare dal `Redactor`** (§P5): un report finisce incollato in un
//! ticket condiviso, ed è lì che un token in chiaro smette di essere un
//! dettaglio. L'allowlist silenzia il rumore noto **senza** toglierlo dai
//! conteggi del manifest.
//!
//! Con `--history` il run **ricorda**: quali endpoint aveva già visto, su quale
//! bersaglio, in quale esecuzione. Ciò che non aveva mai visto lo dice su
//! stderr, e `shadow alerts` lo rilegge quando serve. L'alert è una riga che si
//! interroga, non un messaggio che parte: Shadow non apre nessuna connessione,
//! né qui né altrove (§P6, decisione 36).
//!
//! **Lo storico non tocca il verdetto** (§P4). La classificazione è quella
//! della Fase 3, identica a un run senza storico; la novità è
//! un'informazione **accanto** al finding, mai dentro.
//!
//! Quello che qui **non** c'è: nessun demone, nessun report di compliance,
//! nessuna retention. Sono il resto della Fase 5.

#![forbid(unsafe_code)]

mod config;
mod history;
mod report;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Utc;
use clap::{Arg, ArgAction, Command};
use shadow_collectors::{ingest_file, TailState, SUPPORTED_FORMATS};
use shadow_daemon::Schedule;
use shadow_history::{HistoryStore, Target};
use shadow_core::{
    Allowlist, Classification, Finding, FindingSubject, InputDigest, InputRole, ObservedInventory,
    ObservedInventoryBuilder, Redactor, RulesetVersion, RunManifest, TOOL_NAME,
};

use config::{Config, OutputFormat, DEFAULT_LOG_FORMAT};

/// Descrizione breve del tool, coerente con la missione di §1.
const ABOUT: &str = "Find API endpoints that answer but are not documented (Shadow), \
and endpoints that are documented but no longer answer (Zombie).";

/// Nota mostrata in coda all'aiuto.
const AFTER_HELP: &str = "\
Status: phase 5 (persistent history). With --openapi-spec, findings are labelled
Shadow / Zombie / Known / Undetermined. Without it, Shadow still reports what
it observed, but every finding is a suspicion (Undetermined) with an explicit
confidence: absence from a declared inventory cannot be verified without one.

Two rules worth knowing before you read a report:
  - a partial or uncertain match is never reported as Known;
  - path segments are only collapsed into {id} on strong evidence, so a segment
    such as 'admin' is never absorbed into a group of identifiers.

Shadow works offline on logs you already have: it never contacts the network
and never sends your data anywhere.
Shadow is a discovery aid, not a security certification.";

/// Ogni quanto il demone guarda il log, se non glielo si dice.
///
/// Un minuto: abbastanza spesso da accorgersi di un endpoint nuovo mentre
/// qualcuno lo sta ancora usando, abbastanza raro da non rileggere un file
/// grande in continuazione.
const DEFAULT_INTERVAL_SECONDS: u64 = 60;

/// Dove si mette in ascolto `shadow serve` se non glielo si dice.
///
/// **Solo questa macchina.** Un preimpostato raggiungibile dalla rete sarebbe
/// una superficie aperta da chi non ha chiesto di aprirla, e su uno strumento
/// di sicurezza il valore di riposo è quello che non sorprende nessuno.
const DEFAULT_BIND: &str = "127.0.0.1:8787";

/// Codici di uscita di §7.
mod exit_code {
    /// Analisi eseguita e completata, nessun finding `Shadow`/`Zombie`.
    pub const OK: u8 = 0;
    /// Errore di esecuzione.
    pub const RUNTIME_ERROR: u8 = 1;
    /// Errore d'uso: argomenti della riga di comando non validi.
    pub const USAGE_ERROR: u8 = 2;
    /// Analisi eseguita e completata, ma trovati finding `Shadow`/`Zombie` —
    /// così una pipeline CI può fallire di proposito (§7).
    pub const FINDINGS: u8 = 3;
}

/// Versione estesa: versione del tool + versione del ruleset in uso.
fn long_version() -> String {
    format!(
        "{}\nruleset: {}\nphase:   5 (history, daemon, alerts, compliance report)",
        env!("CARGO_PKG_VERSION"),
        RulesetVersion::current()
    )
}

/// Definizione della riga di comando.
///
/// I nomi degli argomenti coincidono con le **chiavi canoniche di §7**, così che
/// la risoluzione della configurazione e la sua registrazione nel manifest
/// possano lavorare sulla stessa stringa senza tabelle di conversione.
fn command() -> Command {
    Command::new(TOOL_NAME)
        .version(env!("CARGO_PKG_VERSION"))
        .long_version(long_version())
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("log")
                .value_name("LOG_FILE")
                // Non più obbligatorio per clap, perché `shadow alerts` non
                // analizza niente. L'assenza resta un errore d'uso, e lo dice
                // `main` con il codice `2` di §7.
                .required(false)
                .help("Web server log file to analyse"),
        )
        .arg(
            Arg::new("log_format")
                .long("log-format")
                .value_name("FORMAT")
                .action(ArgAction::Set)
                .help("Expected log format, or your own log_format line; never guesses")
                .long_help(format!(
                    "Expected log format. Built-in: {}. Defaults to {DEFAULT_LOG_FORMAT}.\n\n\
                     If your server adds fields — $request_time, $upstream_response_time,\n\
                     $http_x_forwarded_for, or anything else — pass the log_format line from\n\
                     your server configuration instead of a name, and Shadow will read exactly\n\
                     that shape:\n\n  \
                     --log-format '{} $request_time'\n\n\
                     The declared grammar is just as strict as the built-in one: a line that does\n\
                     not match it is discarded and counted, never read field by field from the\n\
                     wrong positions.",
                    SUPPORTED_FORMATS.join(", "),
                    shadow_collectors::NginxCombinedCollector::FORMAT_SPEC
                )),
        )
        .arg(
            Arg::new("openapi_spec")
                .long("openapi-spec")
                .value_name("FILE")
                .action(ArgAction::Set)
                .help("Declared inventory (OpenAPI/Swagger, JSON or YAML) to compare against"),
        )
        .arg(
            Arg::new("zombie_staleness_days")
                .long("zombie-staleness-days")
                .value_name("DAYS")
                .action(ArgAction::Set)
                .help("How long a declared endpoint must go unobserved to count as Zombie"),
        )
        .arg(
            Arg::new("allowlist_path")
                .long("allowlist")
                .value_name("FILE")
                .action(ArgAction::Set)
                .help("Findings to silence; they stay counted in the manifest"),
        )
        .arg(
            Arg::new("output_format")
                .long("output-format")
                .value_name("FORMAT")
                .action(ArgAction::Set)
                .help("terminal, json, or markdown")
                .long_help(
                    "How the report is written to stdout: terminal, json, or markdown.\n\
                     With json or markdown, stdout contains the document and nothing else,\n\
                     so 'shadow ... > report.json' gives you a clean file.",
                ),
        )
        .arg(
            Arg::new("show_raw_values")
                .long("show-raw-values")
                .action(ArgAction::SetTrue)
                .help("Turn redaction OFF: secrets will appear in the report (opt-in)")
                .long_help(
                    "Turn redaction off. Redaction is on by default because logs carry tokens in\n\
                     URLs, emails in query strings, and occasionally passwords sent by mistake,\n\
                     and a report gets pasted into shared tickets. Only pass this if you know\n\
                     where the output is going to end up.",
                ),
        )
        .arg(
            Arg::new("path_prefix")
                .long("path-prefix")
                .value_name("PREFIX")
                .action(ArgAction::Set)
                .help("Analyse only requests under this path; the rest is counted, not hidden")
                .long_help(
                    "Which part of the traffic is the API. A log that carries both web pages and\n\
                     API calls makes every page look like an undocumented endpoint, because the\n\
                     spec describes only the API.\n\n\
                     Requests outside the prefix are NOT silently dropped: they are counted, and\n\
                     the count is reported. Shadow will not decide for you which traffic is the\n\
                     API — leaving this unset analyses everything, which is the safe default.",
                ),
        )
        .arg(
            Arg::new("history_path")
                .long("history")
                .value_name("FILE")
                .action(ArgAction::Set)
                .help("Persistent history: remembers endpoints between runs, offline")
                .long_help(
                    "SQLite file where Shadow records each run and remembers which endpoints it\n\
                     has already seen, so that a NEW shadow endpoint can be told apart from one\n\
                     you already looked at.\n\n\
                     It never leaves the machine: Shadow opens no network connection, and the\n\
                     alert is a row you query with 'shadow alerts', not a message that departs.\n\
                     Everything written there is redacted exactly like the report.",
                ),
        )
        .arg(
            Arg::new("target")
                .long("target")
                .value_name("NAME")
                .action(ArgAction::Set)
                .help("Which service this run is about; histories of different targets never mix"),
        )
        .subcommand(
            Command::new("status")
                .about("One screen: what changed, what is open, what is known")
                .arg(history_arg())
                .arg(target_arg())
                .arg(
                    Arg::new("format")
                        .long("format")
                        .value_name("FORMAT")
                        .action(ArgAction::Set)
                        .help("terminal or json (default: terminal)"),
                ),
        )
        .subcommand(
            Command::new("dashboard")
                .about("Write the same view as a self-contained HTML page")
                .long_about(
                    "One file, no server, no network: open it with a double click, send it to\n\
                     someone, archive it with a date on it.\n\n\
                     With --refresh the page re-reads itself, so a browser left open follows the\n\
                     daemon without anything listening on a port: it re-reads a file from disk.\n\
                     Point the daemon at the same file with --dashboard and it stays current.",
                )
                .arg(history_arg())
                .arg(target_arg())
                .arg(
                    Arg::new("out")
                        .long("out")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("Where to write it; without it, the page goes to stdout"),
                )
                .arg(
                    Arg::new("refresh")
                        .long("refresh")
                        .value_name("SECONDS")
                        .action(ArgAction::Set)
                        .help("Make the page re-read itself every N seconds"),
                ),
        )
        .subcommand(
            Command::new("serve")
                .about("Serve the same view over HTTP, read-only, on this machine")
                .long_about(
                    "This is the ONLY command in Shadow that opens a socket, and it opens one\n\
                     only because you asked for it. It listens on 127.0.0.1 unless you say\n\
                     otherwise, it is read-only — acknowledging an alert stays a terminal\n\
                     thing — and it says out loud when it is reachable from beyond this machine.",
                )
                .arg(history_arg())
                .arg(target_arg())
                .arg(
                    Arg::new("bind")
                        .long("bind")
                        .value_name("HOST:PORT")
                        .action(ArgAction::Set)
                        .help("Where to listen (default: 127.0.0.1:8787)"),
                )
                .arg(
                    Arg::new("refresh")
                        .long("refresh")
                        .value_name("SECONDS")
                        .action(ArgAction::Set)
                        .help("Make the served page re-read itself every N seconds"),
                )
                .arg(
                    Arg::new("requests")
                        .long("requests")
                        .value_name("N")
                        .action(ArgAction::Set)
                        .help("Stop after serving N requests; without it, serves until stopped"),
                ),
        )
        .subcommand(
            Command::new("daemon")
                .about("Keep watching a log over time instead of analysing it once")
                .long_about(
                    "Reads what has grown in the log, adds it to what it had already seen, and\n\
                     records a run in the history each time. What is new since last time is said\n\
                     on stderr; the report is read with 'shadow alerts' and 'shadow compliance'.\n\n\
                     The verdict does not depend on the passing of time: at any cycle it is the\n\
                     one a single run would give on the file read so far. A rotated or truncated\n\
                     log is detected and said, never silently restarted.\n\n\
                     It opens no network connection, here as everywhere else.",
                )
                .arg(
                    Arg::new("log")
                        .value_name("LOG_FILE")
                        .required(true)
                        .help("Web server log file to keep watching"),
                )
                .arg(
                    Arg::new("interval")
                        .long("interval")
                        .value_name("SECONDS")
                        .action(ArgAction::Set)
                        .help("How long to wait between looks (default: 60)"),
                )
                .arg(
                    Arg::new("cycles")
                        .long("cycles")
                        .value_name("N")
                        .action(ArgAction::Set)
                        .help("Stop after N looks; without it, keeps going until stopped"),
                )
                .arg(
                    Arg::new("log_format")
                        .long("log-format")
                        .value_name("FORMAT")
                        .action(ArgAction::Set)
                        .help("Expected log format, or your own log_format line"),
                )
                .arg(
                    Arg::new("openapi_spec")
                        .long("openapi-spec")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("Declared inventory to compare against"),
                )
                .arg(
                    Arg::new("history_path")
                        .long("history")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("Where to remember what has been seen"),
                )
                .arg(
                    Arg::new("target")
                        .long("target")
                        .value_name("NAME")
                        .action(ArgAction::Set)
                        .help("Which service this is about"),
                )
                .arg(
                    Arg::new("path_prefix")
                        .long("path-prefix")
                        .value_name("PREFIX")
                        .action(ArgAction::Set)
                        .help("Analyse only requests under this path"),
                )
                .arg(
                    Arg::new("zombie_staleness_days")
                        .long("zombie-staleness-days")
                        .value_name("DAYS")
                        .action(ArgAction::Set)
                        .help("How long a declared endpoint must go unobserved to count as Zombie"),
                )
                .arg(
                    Arg::new("allowlist_path")
                        .long("allowlist")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("Findings to silence; they stay counted in the manifest"),
                )
                .arg(
                    Arg::new("show_raw_values")
                        .long("show-raw-values")
                        .action(ArgAction::SetTrue)
                        .help("Turn redaction OFF (opt-in)"),
                )
                .arg(
                    Arg::new("fail_on_undetermined")
                        .long("fail-on-undetermined")
                        .action(ArgAction::SetTrue)
                        .help("Also exit 3 when only uncertain findings were produced"),
                )
                .arg(
                    Arg::new("dashboard")
                        .long("dashboard")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("Rewrite this HTML page after every cycle, so a browser can follow"),
                )
                .arg(
                    Arg::new("output_format")
                        .long("output-format")
                        .value_name("FORMAT")
                        .action(ArgAction::Set)
                        .hide(true)
                        .help("Unused by the daemon; accepted so the configuration resolves alike"),
                ),
        )
        .subcommand(
            Command::new("compliance")
                .about("Export the endpoint inventory of a target as an audit document")
                .long_about(
                    "The endpoint inventory of a service, with each endpoint's classification and\n\
                     authentication state, in a form you hand to whoever runs a review.\n\n\
                     The authentication column carries FOUR values and never collapses them into\n\
                     present/absent, because 'not observable' means the log format does not carry\n\
                     the information — it is not evidence that authentication was missing. Next to\n\
                     it is the number of observable requests the verdict rests on.\n\n\
                     Shadow is a discovery aid, not a security certification, and the document\n\
                     says so about itself in every format.",
                )
                .arg(
                    Arg::new("history_path")
                        .long("history")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("The history to export; also SHADOW_HISTORY_PATH"),
                )
                .arg(
                    Arg::new("target")
                        .long("target")
                        .value_name("NAME")
                        .action(ArgAction::Set)
                        .help("Which service to export; defaults to the default target"),
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .value_name("FORMAT")
                        .action(ArgAction::Set)
                        .help("csv, markdown, or json")
                        .long_help(
                            "csv       what an auditor actually opens: filter, sort, attach\n\
                             markdown  the readable version, with the limits stated up front\n\
                             json      a versioned public contract (shadow-compliance/1)\n\n\
                             Defaults to csv. The document goes to stdout, so\n\
                             'shadow compliance ... > inventory.csv' gives a clean file.",
                        ),
                ),
        )
        .subcommand(
            Command::new("alerts")
                .about("List shadow endpoints that appeared and have not been acknowledged")
                .arg(
                    Arg::new("history_path")
                        .long("history")
                        .value_name("FILE")
                        .action(ArgAction::Set)
                        .help("The history file to read; also SHADOW_HISTORY_PATH"),
                )
                .arg(
                    Arg::new("target")
                        .long("target")
                        .value_name("NAME")
                        .action(ArgAction::Set)
                        .help("Which service to list; defaults to the default target"),
                )
                .arg(
                    Arg::new("acknowledge")
                        .long("acknowledge")
                        .value_name("ENDPOINT_ID")
                        .action(ArgAction::Set)
                        .help("Mark one endpoint as taken care of, so it stops being listed"),
                ),
        )
        .arg(
            Arg::new("fail_on_undetermined")
                .long("fail-on-undetermined")
                .action(ArgAction::SetTrue)
                .help("Also exit 3 when only uncertain findings were produced (opt-in)"),
        )
}

fn main() -> ExitCode {
    let mut command = command();

    if std::env::args_os().len() <= 1 {
        // Qui l'aiuto lo stampa il tool di sua iniziativa, non l'utente
        // chiedendolo, quindi esce su **stderr** (§7).
        let mut stderr = io::stderr();
        let _ = command.write_help(&mut stderr);
        let _ = writeln!(stderr);
        return ExitCode::SUCCESS;
    }

    let matches = command.get_matches();

    // `shadow alerts` legge una memoria, non analizza: non produce manifest e
    // non afferma «nessun finding» (§7). Sta prima di tutto il resto perché non
    // ha bisogno né di un log né di una configurazione d'analisi.
    if let Some(sub) = matches.subcommand_matches("alerts") {
        return run_alerts(sub);
    }
    if let Some(sub) = matches.subcommand_matches("compliance") {
        return run_compliance(sub);
    }
    if let Some(sub) = matches.subcommand_matches("daemon") {
        return run_daemon(sub);
    }
    if let Some(sub) = matches.subcommand_matches("status") {
        return run_status(sub);
    }
    if let Some(sub) = matches.subcommand_matches("dashboard") {
        return run_dashboard(sub);
    }
    if let Some(sub) = matches.subcommand_matches("serve") {
        return run_serve(sub);
    }

    let Some(log) = matches.get_one::<String>("log") else {
        // Errore d'uso, non di esecuzione: `2` (§7).
        eprintln!("{TOOL_NAME}: a log file is required. Try 'shadow --help', or 'shadow alerts --history <FILE>' to read the history.");
        return ExitCode::from(exit_code::USAGE_ERROR);
    };
    let log_path = PathBuf::from(log);

    let config = match config::resolve(&matches) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };

    match run(&log_path, &config) {
        Ok(outcome) => ExitCode::from(outcome.exit_code(config.fail_on_undetermined)),
        Err(message) => {
            // Diagnostica: stderr, sempre (§7).
            eprintln!("{TOOL_NAME}: {message}");
            ExitCode::from(exit_code::RUNTIME_ERROR)
        }
    }
}

/// `shadow alerts`: legge lo storico e stampa gli endpoint shadow non ancora
/// presi in carico.
///
/// Il percorso dello storico è arrivato dalla riga di comando in questa stessa
/// invocazione, quindi nei messaggi compare in chiaro: è l'eccezione mirata di
/// §P5 (v1.7), e vale solo perché `--history` qui è obbligatorio.
fn run_alerts(matches: &clap::ArgMatches) -> ExitCode {
    // Stessa precedenza di §7 del resto della configurazione — flag > ambiente
    // > default — invece di leggere direttamente da clap: senza, `SHADOW_TARGET`
    // valeva per un'analisi e non per la lettura degli alert.
    let (path, typed, target) = match config::resolve_for_alerts(matches) {
        Ok(resolved) => resolved,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let target = match Target::new(&target) {
        Ok(target) => target,
        Err(e) => {
            // Lo stesso rifiuto che nel percorso d'analisi esce con `1`: il
            // valore può venire dall'ambiente, non solo dalla riga di comando,
            // quindi non è per forza un errore d'uso.
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    // §P5: in chiaro solo se l'utente l'ha scritto lui in questa invocazione.
    let redactor = Redactor::enabled();
    let shown = if typed {
        path.display().to_string()
    } else {
        redactor.path(&path)
    };
    match history::alerts(
        &mut out,
        &path,
        shown,
        &target,
        matches.get_one::<String>("acknowledge").map(String::as_str),
    ) {
        // `false` = il comando non ha fatto ciò che gli era stato chiesto: il
        // bersaglio non esiste, o l'identificatore da prendere in carico non
        // esiste. Uno script deve poterlo sapere dal codice di uscita (§P2).
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(exit_code::RUNTIME_ERROR),
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            ExitCode::from(exit_code::RUNTIME_ERROR)
        }
    }
}

/// Gli argomenti che ogni comando di lettura condivide, definiti una volta
/// sola: se divergessero, `--history` vorrebbe dire cose diverse a seconda del
/// sottocomando.
fn history_arg() -> Arg {
    Arg::new("history_path")
        .long("history")
        .value_name("FILE")
        .action(ArgAction::Set)
        .help("The history to read; also SHADOW_HISTORY_PATH")
}

fn target_arg() -> Arg {
    Arg::new("target")
        .long("target")
        .value_name("NAME")
        .action(ArgAction::Set)
        .help("Which service; defaults to the default target")
}

/// Apre lo storico per un comando di sola lettura, con la portata di §P5.
fn open_for_reading(
    matches: &clap::ArgMatches,
) -> Result<(shadow_history::HistoryStore, Target), String> {
    let (path, typed, target) = config::resolve_for_alerts(matches)?;
    let target = Target::new(&target).map_err(|e| e.to_string())?;
    let redactor = Redactor::enabled();
    let shown = if typed {
        path.display().to_string()
    } else {
        redactor.path(&path)
    };
    let store =
        shadow_history::HistoryStore::open_existing(&path, shown).map_err(|e| e.to_string())?;
    if !store.has_target(&target).map_err(|e| e.to_string())? {
        return Err(format!(
            "no target named '{}' in this history",
            target.as_str()
        ));
    }
    Ok((store, target))
}

/// `shadow status`: una schermata sola, e la stessa che vedono la pagina, il
/// server e l'applicazione macOS.
fn run_status(matches: &clap::ArgMatches) -> ExitCode {
    let (store, target) = match open_for_reading(matches) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let status = match shadow_view::Status::read(&store, &target) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match matches.get_one::<String>("format").map(String::as_str) {
        Some("json") => shadow_view::json(&mut out, &status),
        Some("terminal") | None => shadow_view::terminal(&mut out, &status),
        Some(other) => {
            eprintln!("{TOOL_NAME}: unknown format '{other}'. Supported: terminal, json");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    }
    ExitCode::SUCCESS
}

/// `shadow dashboard`: la stessa vista, come pagina autonoma.
fn run_dashboard(matches: &clap::ArgMatches) -> ExitCode {
    let (store, target) = match open_for_reading(matches) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let refresh = match parse_seconds(matches, "refresh") {
        Ok(refresh) => refresh,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let status = match shadow_view::Status::read(&store, &target) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };

    match matches.get_one::<String>("out") {
        Some(path) => match write_dashboard(Path::new(path), &status, refresh) {
            Ok(()) => {
                eprintln!("{TOOL_NAME}: wrote {}", Redactor::enabled().path(Path::new(path)));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{TOOL_NAME}: cannot write the page: {e}");
                ExitCode::from(exit_code::RUNTIME_ERROR)
            }
        },
        None => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            shadow_view::dashboard(&mut out, &status, refresh);
            ExitCode::SUCCESS
        }
    }
}

/// Scrive la pagina **atomicamente**: prima su un file accanto, poi si sposta.
///
/// Un browser che si rilegge da solo può leggere il file esattamente mentre lo
/// stiamo riscrivendo, e mostrerebbe mezza pagina. Scrivere e poi spostare fa sì
/// che chi legge veda o la pagina di prima o quella nuova, mai una a metà.
fn write_dashboard(
    path: &Path,
    status: &shadow_view::Status,
    refresh: Option<u32>,
) -> std::io::Result<()> {
    let temporary = path.with_extension("html.tmp");
    {
        let mut file = std::fs::File::create(&temporary)?;
        shadow_view::dashboard(&mut file, status, refresh);
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)
}

fn parse_seconds(matches: &clap::ArgMatches, key: &str) -> Result<Option<u32>, String> {
    match matches.get_one::<String>(key) {
        Some(value) => value
            .parse::<u32>()
            .map(Some)
            .map_err(|_| format!("{key} must be a whole number of seconds, got '{value}'")),
        None => Ok(None),
    }
}

/// `shadow serve`: la stessa vista su HTTP, in sola lettura.
fn run_serve(matches: &clap::ArgMatches) -> ExitCode {
    let (store, target) = match open_for_reading(matches) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let refresh = match parse_seconds(matches, "refresh") {
        Ok(refresh) => refresh.or(Some(shadow_view::DEFAULT_REFRESH_SECONDS)),
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let requests = match matches.get_one::<String>("requests") {
        Some(value) => match value.parse::<u64>() {
            Ok(n) if n > 0 => Some(n),
            _ => {
                eprintln!("{TOOL_NAME}: requests must be a whole number greater than zero");
                return ExitCode::from(exit_code::RUNTIME_ERROR);
            }
        },
        None => None,
    };
    let bind = matches
        .get_one::<String>("bind")
        .map(String::as_str)
        .unwrap_or(DEFAULT_BIND);

    let server = match shadow_serve::Server::bind(bind) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };

    eprintln!(
        "{TOOL_NAME}: serving the '{}' view, read-only, on http://{}/",
        target.as_str(),
        server.address()
    );
    if !server.is_local_only() {
        // Legarsi oltre la macchina locale è legittimo e va **detto**: è
        // l'unico momento in cui questo strumento diventa raggiungibile da
        // qualcun altro, e nessuno deve scoprirlo dopo (§P6).
        eprintln!(
            "{TOOL_NAME}: warning: this address is reachable from outside this machine. \
             Shadow serves a redacted, read-only view, and it has no authentication of its own"
        );
    }

    match server.serve(&store, &target, refresh, requests) {
        Ok(served) => {
            eprintln!("{TOOL_NAME}: served {served} request(s)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            ExitCode::from(exit_code::RUNTIME_ERROR)
        }
    }
}

/// `shadow daemon`: guarda il log nel tempo invece che una volta sola.
///
/// # Perché l'inventario è accumulato e non incrementale
///
/// A ogni giro si legge **solo ciò che è cresciuto**, ma si classifica
/// l'inventario **dall'inizio**. Classificare il solo pezzo nuovo darebbe
/// verdetti diversi da quelli di un comando singolo sullo stesso file — un
/// endpoint visto ieri risulterebbe assente oggi — e §P4 dice che a input
/// costante il verdetto non cambia. Il costo è la memoria dell'inventario, che
/// cresce con il numero di **endpoint distinti** e non con la dimensione del
/// log (§P10).
///
/// Il fondatore l'ha detto in una riga: *«se questo costa in prestazioni,
/// paga»*.
fn run_daemon(matches: &clap::ArgMatches) -> ExitCode {
    let config = match config::resolve(matches) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let log_path = PathBuf::from(
        matches
            .get_one::<String>("log")
            .expect("clap garantisce l'argomento obbligatorio"),
    );
    let interval = match matches.get_one::<String>("interval") {
        Some(value) => match value.parse::<u64>() {
            Ok(seconds) => seconds,
            Err(_) => {
                eprintln!("{TOOL_NAME}: interval must be a whole number of seconds, got '{value}'");
                return ExitCode::from(exit_code::RUNTIME_ERROR);
            }
        },
        None => DEFAULT_INTERVAL_SECONDS,
    };
    let cycles = match matches.get_one::<String>("cycles") {
        Some(value) => match value.parse::<u64>() {
            Ok(n) if n > 0 => Some(n),
            _ => {
                eprintln!("{TOOL_NAME}: cycles must be a whole number greater than zero, got '{value}'");
                return ExitCode::from(exit_code::RUNTIME_ERROR);
            }
        },
        None => None,
    };
    let schedule = match Schedule::new(std::time::Duration::from_secs(interval), cycles) {
        Ok(schedule) => schedule,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };

    match watch(
        &log_path,
        &config,
        schedule,
        matches.get_one::<String>("dashboard").cloned(),
    ) {
        Ok(outcome) => ExitCode::from(outcome.exit_code(config.fail_on_undetermined)),
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            ExitCode::from(exit_code::RUNTIME_ERROR)
        }
    }
}

/// Il ciclo del demone.
fn watch(
    log_path: &Path,
    config: &Config,
    mut schedule: Schedule,
    matches_dashboard: Option<String>,
) -> Result<RunOutcome, String> {
    let mut typed: Vec<&Path> = vec![log_path];
    if config.typed_on_command_line("openapi_spec") {
        typed.extend(config.openapi_spec.as_deref());
    }
    if config.typed_on_command_line("allowlist_path") {
        typed.extend(config.allowlist_path.as_deref());
    }
    if config.typed_on_command_line("history_path") {
        typed.extend(config.history_path.as_deref());
    }
    let typed = typed;

    let collector =
        shadow_collectors::collector_for_format(&config.log_format).map_err(|e| e.to_string())?;
    let redactor = if config.redaction_enabled {
        Redactor::enabled()
    } else {
        eprintln!(
            "{TOOL_NAME}: warning: redaction is OFF, what is written to the history may contain secrets"
        );
        Redactor::disabled()
    };
    let mut store = match &config.history_path {
        Some(path) => Some(
            HistoryStore::open(path, message_path(path, redactor, &typed))
                .map_err(|e| redact_paths(e.to_string(), redactor, &[path], &typed))?,
        ),
        None => None,
    };
    let target = Target::new(&config.target).map_err(|e| e.to_string())?;
    let declared = read_declared_inventory(config, redactor, &typed)?;
    let allowlist = read_allowlist(config, redactor, &typed)?;

    eprintln!(
        "{TOOL_NAME}: watching {} as '{}', every {} second(s), {} cycle(s)",
        message_path(log_path, redactor, &typed),
        collector.format_name(),
        schedule.interval().as_secs(),
        schedule.limit_description()
    );

    let mut builder = ObservedInventoryBuilder::new();
    let mut tail = TailState::new();
    let mut out_of_scope: u64 = 0;
    let mut outcome = RunOutcome {
        confirmed: 0,
        undetermined: 0,
    };

    while schedule.next_cycle() {
        let prefix = config.path_prefix.clone();
        let mut collect = |request: shadow_core::ObservedRequest| match &prefix {
            Some(prefix) if !request.raw_path.starts_with(prefix.as_str()) => out_of_scope += 1,
            _ => builder.observe(request),
        };
        let tail_outcome =
            shadow_collectors::ingest_tail(log_path, &mut tail, collector.as_ref(), &mut collect)
                .map_err(|e| redact_paths(e.to_string(), redactor, &[log_path], &typed))?;

        if tail_outcome.restarted {
            // Non si riparte in silenzio: se lo si facesse, tutto ciò che era
            // già stato visto risulterebbe nuovo, e l'alert perderebbe senso.
            eprintln!(
                "{TOOL_NAME}: the log was rotated or truncated; reading it again from the start"
            );
        }

        // L'inventario si costruisce da **tutto** ciò che si è visto finora: il
        // `clone` è ciò che permette di continuare ad accumulare dopo (§P4).
        let inventory = builder.clone().build();
        let findings = shadow_core::classify(
            &inventory,
            declared.as_ref().map(|spec| &spec.inventory),
            config.zombie_staleness_days,
        );
        let silenced = allowlist.apply(&findings, |subject| pattern_of(&inventory, subject));
        let manifest = build_manifest(
            tail.counts(),
            &tail_outcome.digest,
            inventory.len() as u64,
            declared.as_ref().map(|spec| spec.digest.clone()),
            &findings,
            config,
        );

        let report = report::Report {
            inventory: &inventory,
            out_of_scope,
            visible: silenced.visible,
            silenced: silenced.silenced.len(),
            manifest: &manifest,
            redactor,
        };

        if let Some(store) = store.as_mut() {
            let recorded = store
                .record_run(
                    &target,
                    &manifest.run_timestamp.to_rfc3339(),
                    &manifest.shadow_version,
                    manifest.ruleset_version.as_str(),
                    &report::manifest_json(&report).to_string(),
                    &history::endpoint_records(&inventory, &findings, redactor),
                    &history::finding_records(&findings, &report),
                )
                .map_err(|e| e.to_string())?;
            history::report_new_endpoints(
                &recorded.new_endpoints,
                &target,
                recorded.clock_went_backwards,
            );
        }

        // La pagina si riscrive **dopo** aver registrato: se lo storico non si
        // è lasciato scrivere, non si pubblica una pagina che dice il contrario.
        if let (Some(page), Some(store)) = (matches_dashboard.as_deref(), store.as_ref()) {
            match shadow_view::Status::read(store, &target)
                .map_err(|e| e.to_string())
                .and_then(|status| {
                    write_dashboard(Path::new(page), &status, Some(shadow_view::DEFAULT_REFRESH_SECONDS))
                        .map_err(|e| e.to_string())
                }) {
                Ok(()) => {}
                // Una pagina che non si riesce a scrivere non ferma la
                // sorveglianza: si dice e si va avanti. Il dato è nello storico,
                // che è la cosa che conta.
                Err(e) => eprintln!("{TOOL_NAME}: warning: cannot refresh the dashboard: {e}"),
            }
        }

        eprintln!(
            "{TOOL_NAME}: cycle {}: {} new line(s), {} endpoint(s) known, {} finding(s)",
            schedule.completed(),
            tail_outcome.new_requests,
            inventory.len(),
            findings.len()
        );

        outcome = RunOutcome {
            confirmed: count_of(&manifest, Classification::Shadow)
                + count_of(&manifest, Classification::Zombie),
            undetermined: count_of(&manifest, Classification::Undetermined),
        };
    }

    Ok(outcome)
}

/// `shadow compliance`: esporta l'inventario di un bersaglio come documento
/// d'audit.
///
/// Come `alerts`, **non analizza niente**: legge una memoria. Quindi non emette
/// un `RunManifest` e non afferma «nessun finding» (§7).
fn run_compliance(matches: &clap::ArgMatches) -> ExitCode {
    let (path, typed, target) = match config::resolve_for_alerts(matches) {
        Ok(resolved) => resolved,
        Err(message) => {
            eprintln!("{TOOL_NAME}: {message}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let target = match Target::new(&target) {
        Ok(target) => target,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let format = matches
        .get_one::<String>("format")
        .map(String::as_str)
        .unwrap_or("csv");
    if !matches!(format, "csv" | "markdown" | "json") {
        eprintln!("{TOOL_NAME}: unknown format '{format}'. Supported: csv, markdown, json");
        return ExitCode::from(exit_code::RUNTIME_ERROR);
    }

    let redactor = Redactor::enabled();
    let shown = if typed {
        path.display().to_string()
    } else {
        redactor.path(&path)
    };

    let store = match shadow_history::HistoryStore::open_existing(&path, shown) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    match store.has_target(&target) {
        Ok(true) => {}
        Ok(false) => {
            // Un bersaglio che non c'è non è un bersaglio vuoto: consegnare un
            // inventario vuoto a un auditor per un refuso sarebbe la peggiore
            // delle risposte possibili.
            eprintln!(
                "{TOOL_NAME}: no target named '{}' in this history; nothing was exported",
                target.as_str()
            );
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    }

    let inventory = match store.inventory(&target) {
        Ok(inventory) => inventory,
        Err(e) => {
            eprintln!("{TOOL_NAME}: {e}");
            return ExitCode::from(exit_code::RUNTIME_ERROR);
        }
    };
    let last_run = store.last_run(&target).unwrap_or(None);

    let report = shadow_compliance::ComplianceReport {
        target: target.as_str(),
        last_run_id: last_run.as_ref().map(|r| r.0),
        last_run_timestamp: last_run.as_ref().map(|r| r.1.as_str()),
        shadow_version: last_run
            .as_ref()
            .map(|r| r.2.as_str())
            .unwrap_or(env!("CARGO_PKG_VERSION")),
        ruleset_version: last_run.as_ref().map(|r| r.3.as_str()).unwrap_or("unknown"),
        inventory: &inventory,
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    match format {
        "csv" => shadow_compliance::csv(&mut out, &report),
        "markdown" => shadow_compliance::markdown(&mut out, &report),
        _ => shadow_compliance::json(&mut out, &report),
    }
    ExitCode::SUCCESS
}

/// Il verdetto complessivo del run, da cui discende il codice di uscita (§7).
struct RunOutcome {
    confirmed: u64,
    undetermined: u64,
}

impl RunOutcome {
    /// **Nessun manifest, nessun verdetto** (§7): questo tipo esiste solo se
    /// un'analisi è stata eseguita, quindi qui un `0` significa davvero
    /// "analisi completata, nessun finding".
    ///
    /// I finding silenziati dall'allowlist **contano** ai fini del codice di
    /// uscita? No: silenziare significa "ho visto, ho deciso che va bene". Ma
    /// restano nei conteggi del manifest, quindi la decisione resta ispezionabile.
    fn exit_code(&self, fail_on_undetermined: bool) -> u8 {
        if self.confirmed > 0 || (fail_on_undetermined && self.undetermined > 0) {
            exit_code::FINDINGS
        } else {
            exit_code::OK
        }
    }
}

/// Il run vero e proprio: legge, normalizza, classifica, maschera, stampa.
fn run(log_path: &Path, config: &Config) -> Result<RunOutcome, String> {
    // I percorsi che l'utente ha scritto lui stesso in questa invocazione: nei
    // messaggi diagnostici si mostrano in chiaro (§P5, v1.7). Il log è sempre
    // posizionale, quindi sempre digitato; gli altri solo se sono arrivati dal
    // flag e non dall'ambiente o dal default — ed è la provenienza registrata
    // nel manifest a dirlo, senza doverla dedurre una seconda volta.
    let mut typed: Vec<&Path> = vec![log_path];
    if config.typed_on_command_line("openapi_spec") {
        typed.extend(config.openapi_spec.as_deref());
    }
    if config.typed_on_command_line("allowlist_path") {
        typed.extend(config.allowlist_path.as_deref());
    }
    if config.typed_on_command_line("history_path") {
        typed.extend(config.history_path.as_deref());
    }
    let typed = typed;
    // Un valore di `log_format` non utilizzabile si scopre **prima** di aprire
    // il file: una grammatica ambigua o incompleta non deve arrivare ai dati.
    let collector =
        shadow_collectors::collector_for_format(&config.log_format).map_err(|e| e.to_string())?;

    let redactor = if config.redaction_enabled {
        Redactor::enabled()
    } else {
        // Chi legge il report deve sapere che non è stato mascherato: senza
        // questa riga, un report grezzo è indistinguibile da uno redatto in cui
        // non c'era niente da mascherare (§P9).
        eprintln!(
            "{TOOL_NAME}: warning: redaction is OFF, this report may contain secrets in clear text"
        );
        Redactor::disabled()
    };

    // Lo storico si apre **prima** di leggere il log, non dopo: se non è
    // utilizzabile — corrotto, bloccato da un altro processo, scritto da una
    // versione più nuova — è meglio saperlo prima di macinare un gigabyte, e
    // soprattutto prima di stampare un report che poi non verrebbe ricordato.
    let mut store = match &config.history_path {
        Some(path) => Some(
            // `redact_paths` come per l'ingestione e per la specifica: il
            // messaggio di SQLite riporta il percorso **grezzo** al suo interno,
            // quindi mascherare solo la metà che scriviamo noi lasciava l'altra
            // metà in chiaro due caratteri dopo (§P5).
            HistoryStore::open(path, message_path(path, redactor, &typed))
                .map_err(|e| redact_paths(e.to_string(), redactor, &[path], &typed))?,
        ),
        None => None,
    };
    let target = Target::new(&config.target).map_err(|e| e.to_string())?;

    eprintln!(
        "{TOOL_NAME}: reading {} as '{}'",
        message_path(log_path, redactor, &typed),
        collector.format_name()
    );

    let mut builder = ObservedInventoryBuilder::new();
    // Il filtro di portata: una richiesta fuori dal prefisso **non viene
    // osservata**, ma viene contata e il conteggio si dice. La differenza fra
    // «non l'ho guardata» e «non c'era» è tutta qui, ed è §P1: un endpoint che
    // sparisce senza che nessuno lo sappia è il fallimento peggiore del
    // prodotto.
    let mut out_of_scope: u64 = 0;
    let prefix = config.path_prefix.clone();
    let mut collect = |request: shadow_core::ObservedRequest| match &prefix {
        Some(prefix) if !request.raw_path.starts_with(prefix.as_str()) => out_of_scope += 1,
        _ => builder.observe(request),
    };
    let outcome = ingest_file(log_path, InputRole::Log, collector.as_ref(), &mut collect)
        .map_err(|e| redact_paths(e.to_string(), redactor, &[log_path], &typed))?;
    let inventory = builder.build();
    if let Some(prefix) = &config.path_prefix {
        eprintln!(
            "{TOOL_NAME}: scope: only requests under {prefix} were analysed; \
             {out_of_scope} request(s) fell outside it and were not examined"
        );
    }

    let declared = read_declared_inventory(config, redactor, &typed)?;
    let findings = shadow_core::classify(
        &inventory,
        declared.as_ref().map(|spec| &spec.inventory),
        config.zombie_staleness_days,
    );

    // L'allowlist si applica **dopo** la classificazione e **prima** della
    // stampa: i conteggi del manifest vedono tutto, il report vede ciò che
    // l'utente ha deciso di continuare a guardare (§5, Fase 4).
    let allowlist = read_allowlist(config, redactor, &typed)?;
    let silenced_report = allowlist.apply(&findings, |subject| pattern_of(&inventory, subject));
    for broad in &silenced_report.broad_rules {
        eprintln!(
            "{TOOL_NAME}: warning: allowlist line {} ({}) is broad: {}",
            broad.line_number, broad.rule, broad.reason
        );
    }

    let manifest = build_manifest(
        &outcome.counts,
        &outcome.digest,
        inventory.len() as u64,
        declared.as_ref().map(|spec| spec.digest.clone()),
        &findings,
        config,
    );
    report_diagnostics(&manifest, silenced_report.silenced.len(), redactor);

    let report = report::Report {
        inventory: &inventory,
        out_of_scope,
        visible: silenced_report.visible,
        silenced: silenced_report.silenced.len(),
        manifest: &manifest,
        redactor,
    };

    // Si registra **prima** di stampare. Se lo storico non si lascia scrivere,
    // il run non ha prodotto nulla: uscita `1`, stdout vuoto, nessun manifest e
    // quindi nessun verdetto (§7). Stampare il report e poi fallire lascerebbe
    // credere che la memoria si sia aggiornata quando non è successo, e al run
    // dopo *tutto* risulterebbe nuovo.
    if let Some(store) = store.as_mut() {
        let recorded = store
            .record_run(
                &target,
                &manifest.run_timestamp.to_rfc3339(),
                &manifest.shadow_version,
                manifest.ruleset_version.as_str(),
                &report::manifest_json(&report).to_string(),
                &history::endpoint_records(&inventory, &findings, redactor),
                &history::finding_records(&findings, &report),
            )
            .map_err(|e| e.to_string())?;
        history::report_new_endpoints(&recorded.new_endpoints, &target, recorded.clock_went_backwards);
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    match config.output_format {
        OutputFormat::Terminal => report::terminal(&mut out, &report),
        OutputFormat::Json => report::json(&mut out, &report),
        OutputFormat::Markdown => report::markdown(&mut out, &report),
    }

    Ok(RunOutcome {
        confirmed: count_of(&manifest, Classification::Shadow)
            + count_of(&manifest, Classification::Zombie),
        undetermined: count_of(&manifest, Classification::Undetermined),
    })
}

/// Legge la specifica dichiarata, se fornita.
///
/// Se non è leggibile il run si ferma: classificare contro mezza specifica
/// produrrebbe finding `Shadow` inventati (§P2).
fn read_declared_inventory(
    config: &Config,
    redactor: Redactor,
    typed: &[&Path],
) -> Result<Option<shadow_spec::SpecOutcome>, String> {
    match &config.openapi_spec {
        Some(path) => {
            let spec = shadow_spec::read_spec(path)
                .map_err(|e| redact_paths(e.to_string(), redactor, &[path], typed))?;
            eprintln!(
                "{TOOL_NAME}: declared inventory: {} endpoint(s) from {}",
                spec.inventory.len(),
                message_path(path, redactor, typed)
            );
            if spec.inventory.len() == 0 {
                eprintln!("{TOOL_NAME}: warning: the spec declares no endpoints, so everything observed will look undocumented");
            }
            Ok(Some(spec))
        }
        None => {
            eprintln!("{TOOL_NAME}: no declared inventory given: findings will be suspicions, not verdicts");
            Ok(None)
        }
    }
}

/// Legge l'allowlist, se fornita. Il file lo apre la CLI; la grammatica sta in
/// `shadow-core`, che non fa I/O ma può benissimo interpretare una stringa (§8).
fn read_allowlist(
    config: &Config,
    redactor: Redactor,
    typed: &[&Path],
) -> Result<Allowlist, String> {
    match &config.allowlist_path {
        Some(path) => {
            let shown = message_path(path, redactor, typed);
            let contents = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read the allowlist {shown}: {e}"))?;
            let allowlist = Allowlist::parse(&contents).map_err(|e| format!("{shown}: {e}"))?;
            eprintln!("{TOOL_NAME}: allowlist: {} rule(s)", allowlist.len());
            Ok(allowlist)
        }
        None => Ok(Allowlist::default()),
    }
}

/// Il pattern leggibile di un soggetto, per far combaciare le regole
/// dell'allowlist.
fn pattern_of(inventory: &ObservedInventory, subject: &FindingSubject) -> String {
    match subject {
        FindingSubject::ObservedEndpoint(id) => inventory
            .iter()
            .find(|endpoint| &endpoint.id == id)
            .map(|endpoint| endpoint.path_pattern.clone())
            .unwrap_or_default(),
        FindingSubject::DeclaredEndpoint { path_pattern, .. } => path_pattern.clone(),
    }
}

/// Costruisce il `RunManifest` del run (§6).
fn build_manifest(
    counts: &shadow_core::RunCounts,
    digest: &InputDigest,
    endpoints_found: u64,
    spec_digest: Option<InputDigest>,
    findings: &[Finding],
    config: &Config,
) -> RunManifest {
    let mut counts = counts.clone();
    counts.endpoints_found = endpoints_found;
    // **Tutti** i finding, anche quelli silenziati dall'allowlist: silenziare
    // toglie dal report, non dai conteggi (§5, Fase 4).
    for finding in findings {
        *counts
            .findings_by_classification
            .entry(finding.classification)
            .or_insert(0) += 1;
    }

    let mut inputs = vec![digest.clone()];
    inputs.extend(spec_digest);

    RunManifest {
        shadow_version: env!("CARGO_PKG_VERSION").to_string(),
        ruleset_version: RulesetVersion::current(),
        inputs,
        counts,
        configuration: config.recorded.clone(),
        run_timestamp: Utc::now(),
    }
}

fn count_of(manifest: &RunManifest, classification: Classification) -> u64 {
    manifest
        .counts
        .findings_by_classification
        .get(&classification)
        .copied()
        .unwrap_or(0)
}

/// Come mostrare un percorso in un **messaggio** diagnostico (§P5, v1.7).
///
/// Gli errori e gli avvisi escono su stderr, e stderr finisce nei log di una
/// pipeline CI, che vengono incollati nei ticket esattamente come i report:
/// quindi di norma anche lì i percorsi si mascherano.
///
/// **L'eccezione, una sola:** un percorso che l'utente ha scritto sulla riga di
/// comando in *questa stessa invocazione* si mostra in chiaro. Non gli si rivela
/// nulla che non abbia appena digitato, e un errore che dice
/// `cannot read <path>/spec.json` non aiuta chi ha appena sbagliato a scrivere
/// il percorso. Tutto il resto — percorsi da variabile d'ambiente o da file di
/// config, e qualunque cosa provenga dai dati analizzati — resta mascherato.
///
/// L'eccezione vale solo per i messaggi. Report e manifest non ne hanno: sono i
/// file che viaggiano.
fn message_path(path: &Path, redactor: Redactor, typed_by_user: &[&Path]) -> String {
    if typed_by_user.contains(&path) {
        path.display().to_string()
    } else {
        redactor.path(path)
    }
}

/// Sostituisce i percorsi dentro un messaggio d'errore con la forma che gli
/// spetta secondo [`message_path`].
fn redact_paths(
    message: String,
    redactor: Redactor,
    paths: &[&Path],
    typed_by_user: &[&Path],
) -> String {
    paths.iter().fold(message, |message, path| {
        message.replace(
            &path.display().to_string(),
            &message_path(path, redactor, typed_by_user),
        )
    })
}

/// Avvisi e conteggi: **stderr**, perché l'utente non li ha chiesti (§7).
fn report_diagnostics(manifest: &RunManifest, silenced: usize, redactor: Redactor) {
    let counts = &manifest.counts;
    eprintln!(
        "{TOOL_NAME}: {} line(s) read, {} parsed, {} discarded",
        counts.total_lines, counts.parsed_lines, counts.discarded_lines
    );
    if counts.total_lines == 0 {
        eprintln!("{TOOL_NAME}: warning: the input file is empty");
    }
    for (reason, count) in &counts.discarded_by_reason {
        eprintln!("{TOOL_NAME}: warning: {count} line(s) discarded: {reason}");
    }
    if silenced > 0 {
        eprintln!("{TOOL_NAME}: {silenced} finding(s) silenced by the allowlist, still counted in the manifest");
    }
    if !redactor.is_enabled() {
        eprintln!("{TOOL_NAME}: warning: this report was produced with redaction OFF");
    }
}
