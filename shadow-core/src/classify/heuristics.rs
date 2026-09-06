//! Le euristiche del caso **senza inventario dichiarato** (§9, Fase 3).
//!
//! # Il fallimento da evitare qui è opposto a quello della Fase 2
//!
//! Non è nascondere: è **gridare**. Un tool che segnala duecento cose di cui
//! centonovanta innocue viene spento alla terza esecuzione, e a quel punto non
//! segnala più niente — che è lo stesso risultato di non averlo mai scritto.
//!
//! Quindi: si segnala **solo** ciò per cui esiste un motivo dicibile, ogni
//! finding porta la sua confidenza e il suo perché, e chi non ha niente da dire
//! non produce finding. Un inventario di cento endpoint tranquilli deve
//! generare zero righe di rumore.
//!
//! # Perché nessun finding euristico è `Shadow`
//!
//! `Shadow` significa "assente dall'inventario dichiarato". Senza inventario
//! dichiarato quell'assenza non è verificabile: si può sospettare, non sapere.
//! Il sospetto è `Undetermined`, e la forza del sospetto la porta la confidenza.

use std::collections::BTreeMap;

use crate::inventory::ObservedInventory;
use crate::model::{
    AuthObservation, Classification, Confidence, EndpointPattern, Finding, FindingSubject, Severity,
};
use crate::ruleset::SUSPICIOUS_PATH_SEGMENTS;

/// Classifica senza specifica: solo sospetti, tutti `Undetermined`.
pub fn classify_without_spec(observed: &ObservedInventory) -> Vec<Finding> {
    let with_auth = prefixes_with_observed_auth(observed);

    observed
        .iter()
        .filter_map(|endpoint| {
            let mut evidence = extra_evidence(endpoint);
            let mut confidence = Confidence::Low;
            let mut severity = Severity::Medium;

            if let Some(reason) = auth_anomaly(endpoint, &with_auth) {
                evidence.push(reason);
                confidence = Confidence::Medium;
                severity = Severity::High;
            }

            // Nessun motivo, nessun finding: è così che l'inventario tranquillo
            // resta silenzioso.
            if evidence.is_empty() {
                return None;
            }

            evidence.push(
                "no declared inventory was provided, so this is a suspicion, not a verdict: supply an OpenAPI spec to get one".to_string(),
            );

            Some(Finding {
                subject: FindingSubject::ObservedEndpoint(endpoint.id.clone()),
                classification: Classification::Undetermined,
                confidence,
                is_ambiguous: true,
                evidence,
                severity,
            })
        })
        .collect()
}

/// Evidenza che vale la pena aggiungere a un finding comunque prodotto, anche
/// quando la specifica c'è.
pub fn extra_evidence(endpoint: &EndpointPattern) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(segment) = suspicious_segment(endpoint) {
        evidence.push(format!(
            "path contains the segment '{segment}', which suggests a non-production surface"
        ));
    }
    if answers_without_auth(endpoint) {
        evidence.push(format!(
            "no authentication observed on any of the {} observable request(s)",
            endpoint.auth_observable_count
        ));
    }
    evidence
}

/// L'endpoint risponde **senza autenticazione**, e lo sappiamo davvero.
///
/// Su `NotObservable` questa funzione dice sempre `false`, ed è il punto: se il
/// formato di log non trasporta l'informazione, un'euristica sull'assenza di
/// auth non sbaglierebbe ogni tanto, sbaglierebbe **sistematicamente su tutto
/// il file**. Meglio spenta che sempre in errore — eccezione consapevole a §P1,
/// decisa in v1.1.
pub fn answers_without_auth(endpoint: &EndpointPattern) -> bool {
    matches!(endpoint.auth_observed, AuthObservation::No)
}

fn suspicious_segment(endpoint: &EndpointPattern) -> Option<&'static str> {
    let segments: Vec<&str> = endpoint.path_pattern.split('/').collect();
    SUSPICIOUS_PATH_SEGMENTS
        .iter()
        .find(|suspicious| segments.contains(suspicious))
        .copied()
}

/// L'euristica di §6: "nessuna auth osservata mentre i pattern fratelli ce
/// l'hanno".
///
/// Da sola, "senza auth" non dice granché: un endpoint pubblico è senza auth per
/// disegno. Diventa un segnale quando i **fratelli** — gli endpoint sotto lo
/// stesso prefisso — l'autenticazione ce l'hanno: allora questo è l'unico a non
/// averla, ed è una domanda che vale la pena porsi.
fn auth_anomaly(endpoint: &EndpointPattern, with_auth: &BTreeMap<String, usize>) -> Option<String> {
    if !answers_without_auth(endpoint) {
        return None;
    }
    let prefix = parent_prefix(&endpoint.path_pattern);
    let siblings = with_auth.get(&prefix).copied().unwrap_or(0);
    if siblings == 0 {
        return None;
    }
    Some(format!(
        "no authentication observed here, while {siblings} sibling endpoint(s) under '{prefix}' do show it"
    ))
}

/// Quanti endpoint mostrano autenticazione, per prefisso padre.
fn prefixes_with_observed_auth(observed: &ObservedInventory) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for endpoint in observed.iter() {
        let shows_auth = matches!(
            endpoint.auth_observed,
            AuthObservation::Yes | AuthObservation::Mixed
        );
        if shows_auth {
            *counts.entry(parent_prefix(&endpoint.path_pattern)).or_insert(0) += 1;
        }
    }
    counts
}

fn parent_prefix(pattern: &str) -> String {
    match pattern.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => pattern[..index].to_string(),
    }
}
