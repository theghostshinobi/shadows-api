//! Le verifiche dell'`Allowlist` (§5, Fase 4).
//!
//! Silenziare è una decisione di sicurezza. I test qui presidiano due cose: che
//! il silenzio non cancelli l'evidenza, e che una regola troppo larga non passi
//! inosservata.

use std::collections::BTreeSet;

use shadow_core::allowlist::{Allowlist, AllowlistError, ALLOWLIST_HEADER};
use shadow_core::{Classification, Confidence, EndpointId, Finding, FindingSubject, Severity};

fn finding(id: &str, classification: Classification) -> Finding {
    Finding {
        subject: FindingSubject::ObservedEndpoint(EndpointId::new(id)),
        classification,
        confidence: Confidence::High,
        is_ambiguous: false,
        evidence: vec!["evidenza".to_string()],
        severity: Severity::Medium,
    }
}

fn declared_finding(path: &str) -> Finding {
    Finding {
        subject: FindingSubject::DeclaredEndpoint {
            path_pattern: path.to_string(),
            methods: BTreeSet::new(),
        },
        classification: Classification::Zombie,
        confidence: Confidence::High,
        is_ambiguous: false,
        evidence: vec!["evidenza".to_string()],
        severity: Severity::Low,
    }
}

#[test]
fn un_file_senza_intestazione_di_versione_non_viene_interpretato() {
    // §P2: il formato è versionato perché un cambiamento futuro non possa
    // reinterpretare in silenzio le regole di qualcun altro.
    let error = Allowlist::parse("endpoint abc123").expect_err("manca l'intestazione");

    assert_eq!(error, AllowlistError::MissingHeader);
    assert!(error.to_string().contains("versioned"));
}

#[test]
fn commenti_e_righe_vuote_non_disturbano() {
    let allowlist = Allowlist::parse(&format!(
        "# rivisto dal team\n{ALLOWLIST_HEADER}\n\n# innocuo\nendpoint abc123  # verificato\n"
    ))
    .expect("allowlist valida");

    assert_eq!(allowlist.len(), 1);
}

#[test]
fn un_finding_silenziato_sparisce_dal_report_ma_non_dai_conteggi() {
    // La regola che rende l'allowlist accettabile: silenzia il rumore noto,
    // **non cancella l'evidenza**.
    let allowlist =
        Allowlist::parse(&format!("{ALLOWLIST_HEADER}\nendpoint abc123\n")).expect("valida");
    let findings = vec![
        finding("abc123", Classification::Shadow),
        finding("def456", Classification::Shadow),
    ];

    let outcome = allowlist.apply(&findings, |_| "/api/x".to_string());

    assert_eq!(outcome.visible.len(), 1);
    assert_eq!(outcome.silenced.len(), 1);
    // I conteggi del manifest si costruiscono su `findings`, che è intatto.
    assert_eq!(findings.len(), 2);
}

#[test]
fn una_regola_per_pattern_silenzia_anche_gli_zombie() {
    // Gli zombie hanno per soggetto un endpoint dichiarato, che non ha
    // identificatore stabile: l'unica chiave possibile è il pattern.
    let allowlist = Allowlist::parse(&format!(
        "{ALLOWLIST_HEADER}\npattern /api/legacy/export\n"
    ))
    .expect("valida");
    let findings = vec![declared_finding("/api/legacy/export")];

    let outcome = allowlist.apply(&findings, |subject| match subject {
        FindingSubject::DeclaredEndpoint { path_pattern, .. } => path_pattern.clone(),
        FindingSubject::ObservedEndpoint(_) => String::new(),
    });

    assert_eq!(outcome.silenced.len(), 1);
    assert!(outcome.visible.is_empty());
}

#[test]
fn una_regola_con_jolly_e_poco_contesto_viene_segnalata() {
    // Una wildcard larga zittisce interi rami e da fuori il report sembra
    // semplicemente pulito: è il falso negativo silenzioso entrato da un'altra
    // porta, e va detto ad alta voce.
    let allowlist =
        Allowlist::parse(&format!("{ALLOWLIST_HEADER}\npattern /api/*\n")).expect("valida");
    let findings = vec![finding("abc123", Classification::Shadow)];

    let outcome = allowlist.apply(&findings, |_| "/api/x".to_string());

    assert_eq!(outcome.broad_rules.len(), 1);
    assert_eq!(outcome.broad_rules[0].line_number, 2);
    assert!(outcome.broad_rules[0].reason.contains("fixed segment"));
}

#[test]
fn una_regola_precisa_non_viene_segnalata() {
    let allowlist = Allowlist::parse(&format!(
        "{ALLOWLIST_HEADER}\npattern /api/internal/metrics\n"
    ))
    .expect("valida");
    let findings = vec![finding("abc123", Classification::Shadow)];

    let outcome = allowlist.apply(&findings, |_| "/api/internal/metrics".to_string());

    assert!(outcome.broad_rules.is_empty());
    assert_eq!(outcome.silenced.len(), 1);
}

#[test]
fn una_regola_che_silenzia_troppo_viene_segnalata_anche_se_precisa() {
    let allowlist = Allowlist::parse(&format!(
        "{ALLOWLIST_HEADER}\npattern /api/things/{{id}}/*\n"
    ))
    .expect("valida");
    let findings: Vec<Finding> = (0..9)
        .map(|i| finding(&format!("id{i}"), Classification::Shadow))
        .collect();

    let outcome = allowlist.apply(&findings, |_| "/api/things/{id}/sub".to_string());

    assert_eq!(outcome.silenced.len(), 9);
    assert_eq!(outcome.broad_rules.len(), 1);
    assert!(outcome.broad_rules[0].reason.contains("9 findings"));
}
