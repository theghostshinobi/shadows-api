//! Test dei contratti di Fase 0.
//!
//! Non testano logica di analisi (in Fase 0 non ne esiste): verificano che il
//! modello dati di §6 sia costruibile per intero dall'esterno del crate e che i
//! termini canonici di §5 escano esattamente come il blueprint li scrive.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use shadow_core::{
    AuthObservation, Classification, ConfigSetting, ConfigSource, Confidence, DeclaredEndpoint,
    DigestAlgorithm, EndpointId, EndpointPattern, Finding, FindingSubject, InputDigest, InputRole,
    ObservedAuth, ObservedRequest, RunConfiguration, RunCounts, RunManifest, RulesetVersion,
    Severity, SourceRef,
};

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("timestamp valido")
}

#[test]
fn il_modello_di_par_6_e_costruibile_per_intero() {
    let request = ObservedRequest {
        method: "GET".to_string(),
        raw_path: "/api/users/42".to_string(),
        query_params: BTreeMap::from([("page".to_string(), vec!["1".to_string()])]),
        auth: ObservedAuth::Present {
            scheme: "Bearer".to_string(),
        },
        status_code: 200,
        timestamp: ts(1_700_000_000),
        source: SourceRef {
            file: PathBuf::from("access.log"),
            line_number: 1,
        },
    };

    let pattern = EndpointPattern {
        id: EndpointId::new("endpoint-di-esempio"),
        path_pattern: "/api/users/{id}".to_string(),
        methods: BTreeSet::from(["GET".to_string()]),
        first_seen: ts(1_700_000_000),
        last_seen: ts(1_700_000_100),
        observation_count: 2,
        auth_observed: AuthObservation::Mixed,
        auth_observable_count: 2,
    };

    let declared = DeclaredEndpoint {
        path_pattern: "/api/users/{id}".to_string(),
        methods: BTreeSet::from(["GET".to_string()]),
        declared_auth_schemes: BTreeSet::from(["bearerAuth".to_string()]),
    };

    let finding = Finding {
        subject: FindingSubject::ObservedEndpoint(pattern.id.clone()),
        classification: Classification::Undetermined,
        confidence: Confidence::Low,
        is_ambiguous: true,
        evidence: vec!["esempio di evidenza".to_string()],
        severity: Severity::Medium,
    };

    let manifest = RunManifest {
        shadow_version: "0.0.0".to_string(),
        ruleset_version: RulesetVersion::current(),
        inputs: vec![InputDigest {
            path: PathBuf::from("access.log"),
            role: InputRole::Log,
            algorithm: DigestAlgorithm::Sha256,
            digest: "571c5da449f5c2ca0ed93ce0a293d345c6bc2a5709e62817a73e0996fd4f37e3".to_string(),
        }],
        counts: RunCounts::default(),
        configuration: RunConfiguration {
            settings: BTreeMap::from([(
                "zombie_staleness_days".to_string(),
                ConfigSetting {
                    value: "30".to_string(),
                    source: ConfigSource::Default,
                },
            )]),
        },
        run_timestamp: ts(1_700_000_200),
    };

    // I record esistono, sono raggiungibili dall'esterno e conservano i valori.
    assert_eq!(request.method, "GET");
    assert_eq!(pattern.id.as_str(), "endpoint-di-esempio");
    assert_eq!(pattern.path_pattern, declared.path_pattern);
    assert!(finding.is_ambiguous);
    assert_eq!(manifest.inputs[0].role, InputRole::Log);
    // §6, v1.6: il manifest dice anche **con quale configurazione** il verdetto
    // è stato prodotto, e da dove ogni valore arrivava.
    let staleness = &manifest.configuration.settings["zombie_staleness_days"];
    assert_eq!(staleness.value, "30");
    assert_eq!(staleness.source, ConfigSource::Default);
    assert_eq!(staleness.source.as_str(), "default");
}

#[test]
fn classification_ha_le_quattro_categorie_canoniche_di_par_5() {
    assert_eq!(Classification::Shadow.as_str(), "Shadow");
    assert_eq!(Classification::Zombie.as_str(), "Zombie");
    assert_eq!(Classification::Known.as_str(), "Known");
    assert_eq!(Classification::Undetermined.as_str(), "Undetermined");

    // `Undetermined` è una categoria a sé, non un alias di `Known` (§P9).
    assert_ne!(Classification::Undetermined, Classification::Known);
}

#[test]
fn un_run_appena_iniziato_non_ha_nessun_verdetto_implicito() {
    // I conteggi partono a zero e la mappa dei verdetti è vuota: nessuna
    // categoria — `Known` in testa — viene assunta per default (§P9).
    let counts = RunCounts::default();

    assert_eq!(counts.total_lines, 0);
    assert_eq!(counts.discarded_lines, 0);
    assert!(counts.findings_by_classification.is_empty());
}

#[test]
fn auth_ha_quattro_stati_e_non_osservabile_non_significa_assente() {
    // §6: il verdetto di autenticazione di un `EndpointPattern` ha quattro
    // valori, non tre. "Non osservabile" è uno stato a sé.
    let stati = [
        AuthObservation::Yes,
        AuthObservation::No,
        AuthObservation::Mixed,
        AuthObservation::NotObservable,
    ];
    assert_eq!(stati.len(), 4);
    assert_ne!(AuthObservation::NotObservable, AuthObservation::No);

    // Regola di propagazione (§6): se nessuna richiesta del pattern è
    // osservabile, il verdetto è `NotObservable` e non si basa su nulla.
    let pattern = EndpointPattern {
        id: EndpointId::new("endpoint-su-log-senza-header"),
        path_pattern: "/api/internal/flush".to_string(),
        methods: BTreeSet::from(["POST".to_string()]),
        first_seen: ts(1_700_000_000),
        last_seen: ts(1_700_000_100),
        observation_count: 5_000,
        auth_observed: AuthObservation::NotObservable,
        auth_observable_count: 0,
    };

    assert_eq!(pattern.auth_observable_count, 0);
    // Il verdetto non nasconde su quante richieste si regge: 0 su 5000.
    assert!(pattern.observation_count > pattern.auth_observable_count);
}

#[test]
fn ogni_digest_viaggia_con_il_proprio_algoritmo() {
    // §6: hash SHA-256 sul contenuto, con l'algoritmo registrato accanto al
    // digest, cosi' che non si possa leggere l'uno senza l'altro.
    let digest = InputDigest {
        path: PathBuf::from("openapi.yaml"),
        role: InputRole::OpenApiSpec,
        algorithm: DigestAlgorithm::Sha256,
        digest: "ceac043abc3329531b697afac47148df86181461a1e8c20d89d37a3ee460a4ad".to_string(),
    };

    assert_eq!(digest.algorithm, DigestAlgorithm::Sha256);
    assert_eq!(DigestAlgorithm::Sha256.as_str(), "sha256");
}

#[test]
fn la_ruleset_version_corrente_e_quella_compilata_nel_binario() {
    assert_eq!(
        RulesetVersion::current().as_str(),
        shadow_core::RULESET_VERSION
    );
}
