//! `Allowlist` — l'elenco locale dei finding noti-innocui da **silenziare, non
//! cancellare** (§5, Fase 4).
//!
//! # A cosa serve, e a cosa non serve
//!
//! Serve contro la *noise fatigue*: se a ogni esecuzione l'utente rivede gli
//! stessi dieci finding che ha già esaminato e archiviato, alla terza
//! esecuzione smette di leggere — e da quel momento il tool non segnala più
//! niente, anche quando avrebbe qualcosa da dire.
//!
//! **Non** serve a far sparire l'evidenza. Un finding silenziato non compare nel
//! report ma **resta nei conteggi del `RunManifest`**: chi legge il manifest
//! vede che c'erano dodici finding e che due sono stati silenziati, e può
//! chiedere perché. Un'allowlist che cancellasse i conteggi ricreerebbe il falso
//! negativo silenzioso da un'altra porta.
//!
//! # Il formato è esplicito e versionato di proposito
//!
//! Silenziare un finding è una decisione di sicurezza, e va rivista da un
//! collega come si rivede il codice. Per questo il formato è testo, una regola
//! per riga, con i commenti: sta in una pull request e si legge in un diff.

use std::collections::BTreeSet;
use std::fmt;

use crate::model::{EndpointId, Finding, FindingSubject};

/// Intestazione obbligatoria: il formato è versionato, e un file senza versione
/// non viene interpretato a caso (§P2).
pub const ALLOWLIST_HEADER: &str = "shadow-allowlist 1";

/// Quanti segmenti fissi deve avere almeno una regola con jolly prima di essere
/// considerata troppo ampia.
///
/// Non è una soglia del ruleset e non fa salire la `RulesetVersion` (§7):
/// cambia un **avviso**, non un verdetto. Un finding silenziato resta silenziato
/// e resta nei conteggi, qualunque valore abbia questa costante.
const BROAD_RULE_MIN_LITERAL_SEGMENTS: usize = 2;

/// Oltre quanti finding silenziati da una **sola** regola scatta l'avviso.
const BROAD_RULE_MAX_SILENCED: usize = 5;

/// Una regola di allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowlistRule {
    /// Silenzia un endpoint per **identificatore stabile**: la forma precisa,
    /// che resta valida fra run diversi e non silenzia niente d'altro.
    Endpoint(EndpointId),

    /// Silenzia per pattern del path, con un eventuale `*` finale.
    ///
    /// È la forma comoda, ed è quella che può diventare pericolosa: una regola
    /// con jolly e poco contesto zittisce interi rami, e da fuori il report
    /// sembra semplicemente pulito. Per questo viene segnalata (vedi
    /// [`BroadRule`]).
    Pattern(String),
}

/// Una regola con il contesto che serve a spiegarla a chi la legge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowlistEntry {
    /// La regola.
    pub rule: AllowlistRule,
    /// Riga del file da cui proviene, per poterla indicare in un avviso.
    pub line_number: u64,
}

/// Perché un file di allowlist non è stato accettato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowlistError {
    /// Manca l'intestazione di versione.
    MissingHeader,
    /// Una riga non è una regola riconosciuta.
    UnknownRule {
        /// Riga incriminata.
        line_number: u64,
    },
}

impl fmt::Display for AllowlistError {
    /// Messaggi in inglese: testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader => write!(
                f,
                "the allowlist must start with '{ALLOWLIST_HEADER}': the format is versioned so that a future change cannot silently reinterpret your rules"
            ),
            Self::UnknownRule { line_number } => write!(
                f,
                "line {line_number} is not a rule; each rule is either 'endpoint <id>' or 'pattern <path>'"
            ),
        }
    }
}

/// Una regola troppo ampia, da segnalare a chi ha scritto l'allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadRule {
    /// Riga del file.
    pub line_number: u64,
    /// La regola, resa leggibile.
    pub rule: String,
    /// Quanti finding ha silenziato.
    pub silenced: usize,
    /// Perché è considerata ampia.
    pub reason: String,
}

/// L'`Allowlist` (§5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowlist {
    entries: Vec<AllowlistEntry>,
}

impl Allowlist {
    /// Interpreta il contenuto di un file di allowlist.
    ///
    /// Il file si legge altrove: `shadow-core` non fa I/O (§8). Qui c'è solo la
    /// grammatica, che è pura e quindi verificabile in isolamento.
    pub fn parse(contents: &str) -> Result<Self, AllowlistError> {
        let mut lines = contents.lines().enumerate().map(|(i, l)| (i as u64 + 1, l));
        let mut entries = Vec::new();

        let header_found = lines
            .by_ref()
            .find(|(_, line)| !is_blank_or_comment(line))
            .is_some_and(|(_, line)| line.trim() == ALLOWLIST_HEADER);
        if !header_found {
            return Err(AllowlistError::MissingHeader);
        }

        for (line_number, line) in lines {
            if is_blank_or_comment(line) {
                continue;
            }
            let without_comment = line.split('#').next().unwrap_or("").trim();
            let Some((keyword, value)) = without_comment.split_once(char::is_whitespace) else {
                return Err(AllowlistError::UnknownRule { line_number });
            };
            let value = value.trim();
            let rule = match keyword {
                "endpoint" => AllowlistRule::Endpoint(EndpointId::new(value)),
                "pattern" => AllowlistRule::Pattern(value.to_string()),
                _ => return Err(AllowlistError::UnknownRule { line_number }),
            };
            entries.push(AllowlistEntry { rule, line_number });
        }

        Ok(Self { entries })
    }

    /// Quante regole contiene.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Dice se un finding è silenziato, dato il pattern del suo soggetto.
    ///
    /// Il pattern arriva da fuori perché il soggetto di un finding osservato è
    /// un identificatore, e risolverlo richiede l'inventario: l'allowlist non
    /// ha bisogno di conoscerlo.
    fn matching_entry(&self, id: Option<&EndpointId>, pattern: &str) -> Option<&AllowlistEntry> {
        self.entries.iter().find(|entry| match &entry.rule {
            AllowlistRule::Endpoint(wanted) => id == Some(wanted),
            AllowlistRule::Pattern(rule) => pattern_matches(rule, pattern),
        })
    }

    /// Divide i finding fra quelli da mostrare e quelli silenziati, e segnala le
    /// regole troppo ampie.
    ///
    /// `pattern_of` risolve il soggetto di un finding nel pattern leggibile.
    pub fn apply<'a>(
        &self,
        findings: &'a [Finding],
        pattern_of: impl Fn(&FindingSubject) -> String,
    ) -> AllowlistOutcome<'a> {
        let mut visible = Vec::new();
        let mut silenced = Vec::new();
        let mut per_entry: Vec<usize> = vec![0; self.entries.len()];

        for finding in findings {
            let id = match &finding.subject {
                FindingSubject::ObservedEndpoint(id) => Some(id),
                FindingSubject::DeclaredEndpoint { .. } => None,
            };
            let pattern = pattern_of(&finding.subject);
            match self.matching_entry(id, &pattern) {
                Some(entry) => {
                    if let Some(index) = self.entries.iter().position(|e| e == entry) {
                        per_entry[index] += 1;
                    }
                    silenced.push(finding);
                }
                None => visible.push(finding),
            }
        }

        AllowlistOutcome {
            broad_rules: self.broad_rules(&per_entry),
            silenced,
            visible,
        }
    }

    /// Le regole che silenziano troppo, o che potrebbero.
    ///
    /// Una regola con jolly e poco contesto — `/*`, `/api/*` — può zittire interi
    /// rami di un'API, e chi l'ha scritta non se ne accorge: da fuori il report
    /// sembra semplicemente pulito. È la stessa dinamica del falso negativo
    /// silenzioso, entrata da una porta diversa, e va detta ad alta voce.
    fn broad_rules(&self, per_entry: &[usize]) -> Vec<BroadRule> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let silenced = per_entry.get(index).copied().unwrap_or(0);
                let (rule, reason) = match &entry.rule {
                    AllowlistRule::Pattern(pattern) if pattern.ends_with('*') => {
                        let literal_segments = pattern
                            .trim_end_matches('*')
                            .split('/')
                            .filter(|segment| !segment.is_empty())
                            .count();
                        if literal_segments < BROAD_RULE_MIN_LITERAL_SEGMENTS {
                            (
                                format!("pattern {pattern}"),
                                Some(format!(
                                    "it has only {literal_segments} fixed segment(s) before the wildcard, so it can silence whole branches of the API"
                                )),
                            )
                        } else if silenced > BROAD_RULE_MAX_SILENCED {
                            (
                                format!("pattern {pattern}"),
                                Some(format!("it silenced {silenced} findings at once")),
                            )
                        } else {
                            (String::new(), None)
                        }
                    }
                    _ if silenced > BROAD_RULE_MAX_SILENCED => (
                        describe(&entry.rule),
                        Some(format!("it silenced {silenced} findings at once")),
                    ),
                    _ => (String::new(), None),
                };
                reason.map(|reason| BroadRule {
                    line_number: entry.line_number,
                    rule,
                    silenced,
                    reason,
                })
            })
            .collect()
    }
}

/// Il risultato dell'applicazione dell'allowlist.
#[derive(Debug, Clone, Default)]
pub struct AllowlistOutcome<'a> {
    /// I finding che restano nel report.
    pub visible: Vec<&'a Finding>,
    /// I finding silenziati: **restano nei conteggi del manifest**.
    pub silenced: Vec<&'a Finding>,
    /// Le regole troppo ampie, da segnalare.
    pub broad_rules: Vec<BroadRule>,
}

fn describe(rule: &AllowlistRule) -> String {
    match rule {
        AllowlistRule::Endpoint(id) => format!("endpoint {}", id.as_str()),
        AllowlistRule::Pattern(pattern) => format!("pattern {pattern}"),
    }
}

fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Confronto fra una regola e un pattern, con `*` finale come unico jolly.
///
/// Un solo jolly, e solo in fondo: una sintassi più ricca sarebbe più comoda da
/// scrivere e molto più facile da sbagliare, e ogni errore qui è un endpoint che
/// smette di essere guardato.
fn pattern_matches(rule: &str, pattern: &str) -> bool {
    match rule.strip_suffix('*') {
        Some(prefix) => pattern.starts_with(prefix),
        None => rule == pattern,
    }
}

/// Gli identificatori citati dalle regole `endpoint`, per poter avvisare quando
/// una regola non corrisponde più a niente.
///
/// Una regola che non silenzia più nulla è di solito un endpoint sparito o
/// rinominato: vale la pena dirlo, perché un'allowlist che accumula regole morte
/// diventa illeggibile e nessuno la rivede più.
pub fn referenced_endpoints(allowlist: &Allowlist) -> BTreeSet<EndpointId> {
    allowlist
        .entries
        .iter()
        .filter_map(|entry| match &entry.rule {
            AllowlistRule::Endpoint(id) => Some(id.clone()),
            AllowlistRule::Pattern(_) => None,
        })
        .collect()
}
