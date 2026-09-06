//! Lettura della specifica dichiarata (§9, Fase 3).

use std::path::{Path, PathBuf};

use shadow_core::ruleset::{MAX_SPEC_BYTES, MAX_SPEC_DEPTH};
use shadow_core::{DigestAlgorithm, InputRole};
use shadow_spec::{read_spec, SpecError};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/openapi")
        .join(name)
}

#[test]
fn una_specifica_openapi_produce_gli_endpoint_dichiarati() {
    let spec = read_spec(&fixture("current.json")).expect("specifica leggibile");
    let paths: Vec<&str> = spec
        .inventory
        .iter()
        .map(|e| e.path_pattern.as_str())
        .collect();

    assert_eq!(
        paths,
        vec![
            "/api/reports/{reportId}",
            "/api/sessions/{sessionId}",
            "/api/users/{userId}"
        ]
    );

    let users = spec
        .inventory
        .iter()
        .find(|e| e.path_pattern == "/api/users/{userId}")
        .expect("endpoint presente");
    assert!(users.methods.contains("GET"));
    assert!(users.methods.contains("POST"));
    // Lo schema di sicurezza globale si applica a chi non ne dichiara uno suo.
    assert!(users.declared_auth_schemes.contains("bearerAuth"));
}

#[test]
fn il_digest_della_specifica_accompagna_il_verdetto() {
    // §6: un verdetto prodotto confrontando una specifica deve dire **quale**
    // specifica, o due run che divergono non sono spiegabili.
    let spec = read_spec(&fixture("current.json")).expect("specifica leggibile");

    assert_eq!(spec.digest.role, InputRole::OpenApiSpec);
    assert_eq!(spec.digest.algorithm, DigestAlgorithm::Sha256);
    assert_eq!(spec.digest.digest.len(), 64);
}

#[test]
fn una_specifica_yaml_viene_letta() {
    // La maggior parte degli OpenAPI reali è in YAML: un tool che li rifiuta
    // viene chiuso al primo tentativo (v1.6).
    let spec = read_spec(&fixture("spec.yaml")).expect("YAML leggibile");
    let endpoint = spec.inventory.iter().next().expect("un endpoint");

    assert_eq!(endpoint.path_pattern, "/api/users/{userId}");
    assert!(endpoint.methods.contains("GET"));
}

#[test]
fn avversaria_una_bomba_di_espansione_yaml_viene_rifiutata_prima_di_esplodere() {
    // Il vettore: anchor e alias che si moltiplicano a ogni livello. Poche righe
    // di file diventano gigabyte in memoria — e quando il parser li ha espansi è
    // già troppo tardi, quindi la difesa deve stare **prima** del parser (§7).
    let error = read_spec(&fixture("bomb.yaml")).expect_err("la bomba va rifiutata");

    match &error {
        SpecError::YamlAliases { line_number, .. } => assert!(*line_number > 0),
        other => panic!("atteso YamlAliases, ottenuto {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("expansion bombs"));
    assert!(message.contains("$ref"));
}

#[test]
fn una_specifica_troppo_grande_viene_rifiutata() {
    let path = std::env::temp_dir().join("shadow-fase4-enorme.json");
    let mut contents = String::from("{\"paths\":{");
    while contents.len() <= MAX_SPEC_BYTES {
        contents.push_str("\"/api/x\":{\"get\":{}},");
    }
    contents.push_str("\"/api/y\":{\"get\":{}}}}");
    std::fs::write(&path, &contents).expect("file temporaneo");

    let error = read_spec(&path).expect_err("oltre il limite");
    assert!(matches!(error, SpecError::TooLarge { limit: "size in bytes", .. }));

    std::fs::remove_file(&path).ok();
}

#[test]
fn una_specifica_troppo_annidata_viene_rifiutata() {
    // La profondità si misura senza ricorsione: misurarla non deve essere il
    // modo in cui il processo cade.
    let path = std::env::temp_dir().join("shadow-fase4-profonda.json");
    let depth = MAX_SPEC_DEPTH + 10;
    let mut contents = String::from("{\"paths\":{},\"deep\":");
    contents.push_str(&"[".repeat(depth));
    contents.push_str(&"]".repeat(depth));
    contents.push('}');
    std::fs::write(&path, &contents).expect("file temporaneo");

    let error = read_spec(&path).expect_err("oltre il limite");
    assert!(matches!(error, SpecError::TooLarge { limit: "nesting depth", .. }));

    std::fs::remove_file(&path).ok();
}

#[test]
fn un_json_che_non_e_una_specifica_viene_rifiutato() {
    let path = std::env::temp_dir().join("shadow-fase3-non-spec.json");
    std::fs::write(&path, br#"{"hello":"world"}"#).expect("file temporaneo");

    let error = read_spec(&path).expect_err("non è una specifica");
    assert!(matches!(error, SpecError::NotASpec { .. }));
    assert!(error.to_string().contains("no 'paths' object"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn il_prefisso_dei_server_entra_nei_path_dichiarati() {
    // Se la specifica serve i propri path sotto un prefisso, ignorarlo
    // significherebbe non riconoscerli nel traffico e segnalarli come `Shadow`
    // pur essendo dichiarati.
    let path = std::env::temp_dir().join("shadow-fase3-baseurl.json");
    std::fs::write(
        &path,
        br#"{"servers":[{"url":"https://api.example.com/v2"}],"paths":{"/users/{id}":{"get":{}}}}"#,
    )
    .expect("file temporaneo");

    let spec = read_spec(&path).expect("specifica leggibile");
    let paths: Vec<&str> = spec
        .inventory
        .iter()
        .map(|e| e.path_pattern.as_str())
        .collect();
    assert_eq!(paths, vec!["/v2/users/{id}"]);

    std::fs::remove_file(&path).ok();
}

#[test]
fn swagger_2_con_basepath_e_letto_come_openapi_3() {
    let path = std::env::temp_dir().join("shadow-fase3-swagger2.json");
    std::fs::write(
        &path,
        br#"{"swagger":"2.0","basePath":"/api","paths":{"/things":{"get":{},"post":{}}}}"#,
    )
    .expect("file temporaneo");

    let spec = read_spec(&path).expect("specifica leggibile");
    let endpoint = spec.inventory.iter().next().expect("un endpoint");
    assert_eq!(endpoint.path_pattern, "/api/things");
    assert_eq!(endpoint.methods.len(), 2);

    std::fs::remove_file(&path).ok();
}
