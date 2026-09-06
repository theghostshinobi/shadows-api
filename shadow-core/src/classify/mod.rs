//! La classificazione: da inventario a `Finding` (§9, Fase 3).
//!
//! # La posizione di riposo è l'incertezza
//!
//! Con una specifica dichiarata si producono `Shadow`, `Zombie`, `Known` e
//! `Undetermined`; senza, solo `Undetermined` euristici. In entrambi i casi vale
//! la stessa regola: **un `Known` sbagliato è peggio di cento `Undetermined`**.
//! Un `Known` sbagliato è un endpoint che nessuno guarderà più; un
//! `Undetermined` è un endpoint che qualcuno guarderà una volta di troppo.
//!
//! # Perché senza specifica non si dice mai `Shadow`
//!
//! `Shadow` significa "osservato e **assente dall'inventario dichiarato**".
//! Senza inventario dichiarato quell'assenza non è verificabile: si può
//! sospettare, non sapere. Il sospetto è un `Undetermined` con una confidenza
//! esplicita — la sfumatura la porta la confidenza, non la classificazione
//! (§9, Fase 3).

pub mod heuristics;
pub mod matching;

use chrono::{DateTime, Duration, Utc};

use crate::inventory::declared::DeclaredInventory;
use crate::inventory::ObservedInventory;
use crate::model::{
    Classification, Confidence, DeclaredEndpoint, EndpointPattern, Finding, FindingSubject,
    Severity,
};

use matching::{
    compare, declared_segments, divergence, observed_segments, partial_distance, spanned_segments,
    Divergence, PathMatch, Segment,
};

/// Un endpoint dichiarato che **potrebbe** avere a che fare con quello
/// osservato, con quanto ne dista e in che modo.
struct Candidate<'a> {
    endpoint: &'a DeclaredEndpoint,
    /// Distanza dall'osservato: più piccola, più vicino (vedi
    /// [`matching::partial_distance`]).
    distance: (usize, usize),
    /// Quanti segmenti la variabile finale del dichiarato dovrebbe inghiottire
    /// per coprire l'osservato; `None` se le due forme hanno la stessa
    /// lunghezza.
    spans: Option<usize>,
}

/// Classifica un inventario osservato, con o senza inventario dichiarato.
///
/// `staleness_days` è la finestra oltre cui un endpoint dichiarato e non
/// osservato diventa `Zombie` (`zombie_staleness_days`, §7).
///
/// L'ordine dei finding è deterministico (§P4): dipende solo dagli inventari,
/// non dall'ordine in cui il log è stato letto.
pub fn classify(
    observed: &ObservedInventory,
    declared: Option<&DeclaredInventory>,
    staleness_days: i64,
) -> Vec<Finding> {
    match declared {
        Some(declared) => classify_against_spec(observed, declared, staleness_days),
        None => heuristics::classify_without_spec(observed),
    }
}

fn classify_against_spec(
    observed: &ObservedInventory,
    declared: &DeclaredInventory,
    staleness_days: i64,
) -> Vec<Finding> {
    let declared_forms: Vec<(&DeclaredEndpoint, Vec<matching::Segment>)> = declared
        .iter()
        .map(|endpoint| (endpoint, declared_segments(&endpoint.path_pattern)))
        .collect();

    let mut findings = Vec::new();
    let mut matched_declared: Vec<bool> = vec![false; declared_forms.len()];

    for endpoint in observed.iter() {
        let form = observed_segments(&endpoint.path_pattern);
        let mut exact: Vec<&DeclaredEndpoint> = Vec::new();
        // Ogni candidato parziale porta con sé **quanto dista** dall'osservato:
        // senza, l'evidenza finirebbe per nominare il primo in ordine di
        // specifica, che è il difetto trovato dal collaudo.
        let mut partial: Vec<Candidate<'_>> = Vec::new();

        for (index, (candidate, candidate_form)) in declared_forms.iter().enumerate() {
            match compare(&form, candidate_form) {
                PathMatch::Exact => {
                    matched_declared[index] = true;
                    exact.push(candidate);
                }
                PathMatch::Partial => {
                    // Un match parziale **non** copre l'endpoint dichiarato: se
                    // fosse così, un `/api/users/admin` osservato basterebbe a
                    // far sembrare vivo un `/api/users/{id}` che nessuno chiama
                    // più.
                    partial.push(Candidate {
                        endpoint: candidate,
                        distance: partial_distance(&form, candidate_form),
                        spans: None,
                    });
                }
                // Il dichiarato è più corto e finisce con una variabile che
                // dovrebbe attraversare le barre. Non lo copre, e non è nemmeno
                // assente dall'inventario: non si sa, e non saperlo va detto
                // (§P9). Prima questo caso era una differenza di lunghezza come
                // un'altra, cioè `Shadow` conclamato.
                PathMatch::Spanning => {
                    partial.push(Candidate {
                        endpoint: candidate,
                        distance: partial_distance(&form, candidate_form),
                        spans: Some(spanned_segments(&form, candidate_form)),
                    });
                }
                PathMatch::None => {}
            }
        }

        // Il più vicino per primo; a parità di distanza decide il pattern in
        // ordine alfabetico, così l'esito non dipende dall'ordine della
        // specifica (§P4).
        partial.sort_by(|a, b| {
            a.distance
                .cmp(&b.distance)
                .then_with(|| a.endpoint.path_pattern.cmp(&b.endpoint.path_pattern))
        });

        findings.push(classify_observed(endpoint, &form, &exact, &partial));
    }

    for (index, (candidate, _)) in declared_forms.iter().enumerate() {
        if !matched_declared[index] {
            findings.push(classify_declared_but_unseen(
                candidate,
                observation_window(observed),
                staleness_days,
            ));
        }
    }

    findings
}

/// Il verdetto su un endpoint osservato.
fn classify_observed(
    endpoint: &EndpointPattern,
    observed_form: &[Segment],
    exact: &[&DeclaredEndpoint],
    partial: &[Candidate<'_>],
) -> Finding {
    let subject = FindingSubject::ObservedEndpoint(endpoint.id.clone());

    if !exact.is_empty() {
        let declared_methods: Vec<&String> =
            exact.iter().flat_map(|e| e.methods.iter()).collect();
        let undeclared: Vec<&str> = endpoint
            .methods
            .iter()
            .filter(|method| !declared_methods.contains(method))
            .map(String::as_str)
            .collect();

        if undeclared.is_empty() {
            return Finding {
                subject,
                classification: Classification::Known,
                confidence: Confidence::High,
                is_ambiguous: false,
                evidence: vec![format!(
                    "path and methods both declared in the spec as {}",
                    exact[0].path_pattern
                )],
                severity: Severity::Low,
            };
        }

        // Il path è dichiarato, un metodo no. Non è `Known` — sarebbe far
        // sparire un'operazione non documentata dentro un endpoint documentato.
        //
        // Il modello non ha un posto per un verdetto **per metodo**: il soggetto
        // di un `Finding` è l'endpoint, che porta un insieme di metodi. Invece
        // di inventarne uno, l'informazione passa dove c'è già posto —
        // l'evidenza nomina i metodi — e la severità sale di un livello, perché
        // un `DELETE` non documentato è più grave di un disallineamento di path
        // (§6, v1.6).
        return Finding {
            subject,
            classification: Classification::Undetermined,
            confidence: Confidence::High,
            is_ambiguous: true,
            evidence: vec![
                format!(
                    "path declared in the spec as {}, but method(s) {} were observed and are not declared",
                    exact[0].path_pattern,
                    undeclared.join(", ")
                ),
                "an undeclared method on a declared path is an undocumented operation, not a known endpoint".to_string(),
            ],
            severity: raise(Severity::Medium),
        };
    }

    if let Some(closest_candidate) = partial.first() {
        let closest = closest_candidate.endpoint;
        let distance = &closest_candidate.distance;
        // Il caso che §9 chiama per nome: potrebbe essere un'istanza
        // dell'endpoint dichiarato, o un endpoint che la specifica non nomina.
        //
        // `partial` è ordinato per distanza (vedi `partial_distance`), quindi
        // qui il primo è davvero **il più vicino**, non il primo che la
        // specifica elencava.
        // Il caso della variabile che dovrebbe attraversare le barre ha una
        // frase sua: qui non c'è un verso di divergenza da calcolare, c'è una
        // cosa che la specifica non è in grado di dire.
        if let Some(spans) = closest_candidate.spans {
            let mut evidence = vec![
                format!(
                    "no exact match in the spec; the declared path {} would cover this endpoint only if its final variable segment spanned {spans} path segments",
                    closest.path_pattern
                ),
                "OpenAPI cannot say whether a path parameter contains slashes, so whether this is that declared endpoint or an undocumented one cannot be read from the spec".to_string(),
                "reported as undetermined and not as shadow: absence from the declared inventory is not verifiable while a declared path could cover it".to_string(),
            ];
            evidence.extend(heuristics::extra_evidence(endpoint));
            return Finding {
                subject,
                classification: Classification::Undetermined,
                confidence: Confidence::Medium,
                is_ambiguous: true,
                evidence,
                severity: severity_for_exposure(endpoint, Severity::Medium),
            };
        }

        // Il verso della divergenza si **calcola**, non si asserisce: la
        // frase fissa che c'era qui prima era falsa ogni volta che a portare
        // la variabile era l'osservato — cioè nel caso centrale del prodotto,
        // un `/api/users/{id}` osservato contro un `/api/users/me` dichiarato.
        let differenza = match divergence(observed_form, &declared_segments(&closest.path_pattern)) {
            Some(Divergence::ObservedIsFixed) => {
                "which has a variable segment where this endpoint has a fixed one"
            }
            Some(Divergence::ObservedIsVariable) => {
                "which has a fixed segment where this endpoint has a variable one"
            }
            Some(Divergence::Both) => {
                "which differs in both directions: a variable segment where this endpoint has a fixed one, and a fixed segment where this endpoint has a variable one"
            }
            // Irraggiungibile per costruzione — un candidato senza divergenze
            // sarebbe un match esatto e non sarebbe finito qui — ma il verdetto
            // non si appoggia a un `unreachable!`: se mai ci arrivasse, dice
            // meno invece di dire il falso (§P9).
            None => "which does not line up with it segment by segment",
        };
        let mut evidence = vec![format!(
            "no exact match in the spec; the closest declared path is {}, {differenza}",
            closest.path_pattern
        )];
        // Se più di un path dichiarato è vicino uguale, sceglierne uno resta
        // arbitrario: allora si dice quanti sono, invece di far credere che ce
        // ne fosse uno solo (§P9).
        let equally_close = partial.iter().filter(|c| &c.distance == distance).count();
        if equally_close > 1 {
            evidence.push(format!(
                "{} other declared path(s) are just as close, so which one this endpoint belongs to cannot be read from the spec",
                equally_close - 1
            ));
        }
        evidence.push(
            "reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe".to_string(),
        );
        evidence.extend(heuristics::extra_evidence(endpoint));

        // Anche qui i metodi contano, e si guardano su **tutti** i candidati
        // parziali, non solo sul più vicino: bastano un dichiarato qualsiasi
        // che preveda quel metodo perché non si possa parlare di operazione non
        // documentata. È la posizione conservativa (§P3), ed è ciò che la
        // frase deve dire — prima diceva "il più vicino" mentre guardava tutti.
        let declared_methods: Vec<&String> = partial
            .iter()
            .flat_map(|c| c.endpoint.methods.iter())
            .collect();
        let undeclared: Vec<&str> = endpoint
            .methods
            .iter()
            .filter(|method| !declared_methods.contains(method))
            .map(String::as_str)
            .collect();
        let mut severity = severity_for_exposure(endpoint, Severity::Medium);
        if !undeclared.is_empty() {
            evidence.push(format!(
                "method(s) {} were observed and are not declared on any of the closely matching declared paths",
                undeclared.join(", ")
            ));
            severity = raise(severity);
        }

        return Finding {
            subject,
            classification: Classification::Undetermined,
            confidence: Confidence::Medium,
            is_ambiguous: true,
            evidence,
            severity,
        };
    }

    // Nessuna forma compatibile: questo endpoint la specifica non lo descrive.
    let mut evidence = vec!["not present in the declared inventory".to_string()];
    evidence.extend(heuristics::extra_evidence(endpoint));

    Finding {
        subject,
        classification: Classification::Shadow,
        confidence: Confidence::High,
        is_ambiguous: false,
        evidence,
        severity: severity_for_exposure(endpoint, Severity::Medium),
    }
}

/// Il verdetto su un endpoint dichiarato che il traffico non mostra.
///
/// Qui la tentazione è dire `Zombie` e basta. Ma "non più osservato **da oltre
/// la finestra di staleness**" (§5) è un'affermazione sul tempo, e un log che
/// copre un'ora non permette di farla: direbbe zombie di tutto ciò che
/// semplicemente non è stato chiamato stanotte.
fn classify_declared_but_unseen(
    declared: &DeclaredEndpoint,
    window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    staleness_days: i64,
) -> Finding {
    let subject = FindingSubject::DeclaredEndpoint {
        path_pattern: declared.path_pattern.clone(),
        methods: declared.methods.clone(),
    };

    let covered = window.map(|(first, last)| last - first).unwrap_or_default();
    let required = Duration::days(staleness_days);

    if covered >= required {
        return Finding {
            subject,
            classification: Classification::Zombie,
            confidence: Confidence::High,
            is_ambiguous: false,
            evidence: vec![format!(
                "declared in the spec but never observed in {} day(s) of traffic, more than the staleness window of {} day(s)",
                covered.num_days(),
                staleness_days
            )],
            severity: Severity::Low,
        };
    }

    Finding {
        subject,
        classification: Classification::Undetermined,
        confidence: Confidence::Low,
        is_ambiguous: true,
        evidence: vec![
            "declared in the spec but never observed in this log".to_string(),
            format!(
                "the analysed traffic covers {} day(s), less than the staleness window of {} day(s): too short to call it a zombie",
                covered.num_days(),
                staleness_days
            ),
        ],
        severity: Severity::Low,
    }
}

/// La finestra temporale coperta dal traffico analizzato.
fn observation_window(observed: &ObservedInventory) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let first = observed.iter().map(|e| e.first_seen).min()?;
    let last = observed.iter().map(|e| e.last_seen).max()?;
    Some((first, last))
}

/// Alza la severità quando l'endpoint risponde **senza autenticazione
/// osservata**: non documentato e senza auth è la combinazione peggiore.
///
/// Solo su auth osservabile: su `NotObservable` non si sa, e non sapere non
/// alza né abbassa niente (§6).
fn raise(severity: Severity) -> Severity {
    match severity {
        Severity::Low => Severity::Medium,
        Severity::Medium | Severity::High => Severity::High,
    }
}

fn severity_for_exposure(endpoint: &EndpointPattern, base: Severity) -> Severity {
    if heuristics::answers_without_auth(endpoint) {
        Severity::High
    } else {
        base
    }
}
