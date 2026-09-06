//! Il collegamento fra la pipeline e lo storico persistente (§9, Fase 5).
//!
//! Qui vive **solo l'orchestrazione**: convertire ciò che il run ha prodotto
//! nella forma che `shadow-history` persiste, e stampare gli alert. La
//! grammatica del database sta nel suo crate, la redazione nel core, la
//! classificazione pure — e nessuna delle tre cambia perché esiste uno storico.
//!
//! # La riga che tiene in piedi §P4 con il tempo di mezzo
//!
//! Tutto ciò che passa di qui è **già stato deciso**: i finding sono quelli che
//! `shadow_core::classify` ha prodotto, identici a quelli di un run senza
//! storico. Lo storico aggiunge una sola informazione — *questo endpoint non
//! c'era* — e la aggiunge **accanto** al verdetto, mai dentro. Un run con
//! `--history` e uno senza, sullo stesso input, danno lo stesso report.

use std::io::Write;
use std::path::Path;

use shadow_core::{AuthObservation, Finding, FindingSubject, ObservedInventory, Redactor};
use shadow_history::{EndpointState, HistoryError, HistoryStore, ObservedEndpointRecord, Target};

use crate::report::Report;

/// Traduce l'inventario osservato nella forma che lo storico conserva.
///
/// La classificazione di ciascun endpoint viene dai finding: un endpoint senza
/// finding non esiste (§9, Fase 3, ogni osservato ne produce uno).
pub fn endpoint_records(
    inventory: &ObservedInventory,
    findings: &[Finding],
    redactor: Redactor,
) -> Vec<ObservedEndpointRecord> {
    inventory
        .iter()
        .map(|endpoint| {
            // **Ogni** endpoint osservato entra nello storico, anche quello su
            // cui il run non ha detto niente: è la memoria di ciò che è stato
            // visto, e serve a riconoscere il nuovo. Un endpoint dimenticato
            // perché «non aveva finding» tornerebbe nuovo a ogni esecuzione.
            let classification = findings.iter().find_map(|finding| match &finding.subject {
                FindingSubject::ObservedEndpoint(id) if id == &endpoint.id => {
                    Some(finding.classification)
                }
                _ => None,
            });
            ObservedEndpointRecord {
                id: endpoint.id.as_str().to_string(),
                methods: endpoint.methods.iter().cloned().collect(),
                observation_count: endpoint.observation_count,
                auth_observed: auth_name(endpoint.auth_observed).to_string(),
                auth_observable_count: endpoint.auth_observable_count,
                // **Redatto qui, una volta**, con lo stesso `Redactor` che ha
                // prodotto il report: uno storico su disco è una superficie
                // nuova, e §P5 non fa eccezioni per i file che restano (§P5).
                redacted_pattern: redactor.text(&endpoint.path_pattern),
                classification,
            }
        })
        .collect()
}

/// Traduce i finding nella forma che lo storico conserva.
///
/// Sono **tutti** i finding, anche quelli silenziati dall'allowlist: silenziare
/// toglie dal report, non dai conteggi né dalla memoria (§5, Fase 4).
pub fn finding_records(
    findings: &[Finding],
    report: &Report<'_>,
) -> Vec<shadow_history::FindingRecord> {
    findings
        .iter()
        .map(|finding| shadow_history::FindingRecord {
            endpoint_id: match &finding.subject {
                FindingSubject::ObservedEndpoint(id) => Some(id.as_str().to_string()),
                FindingSubject::DeclaredEndpoint { .. } => None,
            },
            declared_path: match &finding.subject {
                FindingSubject::DeclaredEndpoint { path_pattern, .. } => {
                    Some(report.redactor.text(path_pattern))
                }
                FindingSubject::ObservedEndpoint(_) => None,
            },
            classification: finding.classification.as_str().to_string(),
            confidence: format!("{:?}", finding.confidence).to_lowercase(),
            severity: format!("{:?}", finding.severity).to_lowercase(),
            evidence: finding
                .evidence
                .iter()
                .map(|line| report.redactor.text(line))
                .collect::<Vec<String>>()
                .join("\n"),
        })
        .collect()
}

/// Stampa su **stderr** ciò che il run ha scoperto di nuovo.
///
/// Su stderr e non su stdout perché è diagnostica: l'utente ha chiesto un
/// report, non un elenco di novità (§7). L'elenco lo chiede con `shadow alerts`,
/// e allora esce su stdout.
pub fn report_new_endpoints(new_endpoints: &[EndpointState], target: &Target, clock_back: bool) {
    if clock_back {
        // Non si aggiusta niente e non si rifiuta il run: si dice. L'ordine
        // dello storico non dipende dall'orologio, quindi il dato resta
        // utilizzabile — ma chi legge un archivio d'audit deve sapere che le
        // date non sono monotone.
        eprintln!(
            "{}: warning: this run's timestamp is earlier than the previous run on target '{}'; \
             the history is ordered by run number, not by clock",
            shadow_core::TOOL_NAME,
            target.as_str()
        );
    }
    if new_endpoints.is_empty() {
        return;
    }
    eprintln!(
        "{}: {} endpoint(s) never seen before on target '{}':",
        shadow_core::TOOL_NAME,
        new_endpoints.len(),
        target.as_str()
    );
    for endpoint in new_endpoints {
        eprintln!(
            "{}: new  {}  {}  ({})",
            shadow_core::TOOL_NAME,
            endpoint.id,
            endpoint.redacted_pattern,
            endpoint
                .last_classification
                .as_deref()
                .unwrap_or("no verdict")
        );
    }
}

/// Il sottocomando `shadow alerts`.
///
/// # Nessun manifest, nessun verdetto (§7)
///
/// Questo comando **non analizza niente**: legge una memoria. Quindi non emette
/// un `RunManifest` e non afferma «nessun finding». Esce con `0` in caso di
/// successo nello stesso senso in cui lo fanno `--help` e `--version`: ha fatto
/// ciò che gli era stato chiesto, non ha concluso niente su un'analisi.
///
/// L'elenco esce su **stdout**, perché è ciò che l'utente ha chiesto.
pub fn alerts(
    out: &mut impl Write,
    path: &Path,
    shown_path: String,
    target: &Target,
    acknowledge: Option<&str>,
) -> Result<bool, HistoryError> {
    // **Non** `open`: uno storico da leggere deve già esistere, o a un errore di
    // battitura si risponderebbe «niente da segnalare».
    let mut store = HistoryStore::open_existing(path, shown_path)?;

    // Un bersaglio che non c'è non è un bersaglio tranquillo. Senza questo
    // controllo, `--target produzine` rispondeva «nessun alert» con successo.
    if !store.has_target(target)? {
        let known = store.targets()?;
        let _ = writeln!(
            out,
            "no target named '{}' in this history.",
            target.as_str()
        );
        if known.is_empty() {
            let _ = writeln!(out, "It holds no runs yet.");
        } else {
            let _ = writeln!(
                out,
                "It holds: {}",
                known
                    .iter()
                    .map(|t| t.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }
        return Ok(false);
    }

    if let Some(id) = acknowledge {
        if store.acknowledge(target, id)? {
            let _ = writeln!(out, "acknowledged {id} on target '{}'", target.as_str());
            return Ok(true);
        }
        // Non ha fatto ciò che gli era stato chiesto: uno script deve poterlo
        // sapere dal codice di uscita, non solo leggendo la riga (§P2).
        let _ = writeln!(
            out,
            "no endpoint with id {id} on target '{}'",
            target.as_str()
        );
        return Ok(false);
    }

    let pending = store.pending_alerts(target)?;
    if pending.is_empty() {
        let _ = writeln!(
            out,
            "no unacknowledged shadow endpoints on target '{}'",
            target.as_str()
        );
        return Ok(true);
    }

    let _ = writeln!(
        out,
        "{} unacknowledged shadow endpoint(s) on target '{}'\n",
        pending.len(),
        target.as_str()
    );
    for endpoint in &pending {
        let _ = writeln!(
            out,
            "  {}  {}\n      first seen in run {}, last seen in run {}",
            endpoint.id, endpoint.redacted_pattern, endpoint.first_seen_run, endpoint.last_seen_run
        );
    }
    let _ = writeln!(
        out,
        "\nAcknowledge one with: shadow alerts --history <FILE> --acknowledge <ENDPOINT_ID>"
    );
    Ok(true)
}

/// Il nome canonico di uno stato di autenticazione, come finisce nello storico
/// e poi nel report d'audit.
///
/// Sono **quattro**, e si scrivono per esteso invece di derivarli dal `Debug`
/// del tipo: `notobservable` attaccato è illeggibile in una colonna che un
/// auditor deve capire al primo sguardo, e questa è la colonna su cui §9 vieta
/// esplicitamente di collassare i quattro valori in un binario.
fn auth_name(auth: AuthObservation) -> &'static str {
    match auth {
        AuthObservation::Yes => "yes",
        AuthObservation::No => "no",
        AuthObservation::Mixed => "mixed",
        AuthObservation::NotObservable => "not observable",
    }
}
