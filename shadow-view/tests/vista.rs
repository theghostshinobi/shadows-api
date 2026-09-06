//! Le verifiche della vista (§10, Fase 5).
//!
//! La più importante non riguarda il contenuto: riguarda il fatto che una
//! pagina HTML **viene eseguita** da chi la apre.

use shadow_history::{
    Classification, FindingRecord, HistoryStore, ObservedEndpointRecord, Target,
};
use shadow_view::Status;

fn store(name: &str) -> (HistoryStore, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("shadow-vista-{name}.db"));
    let _ = std::fs::remove_file(&path);
    let store = HistoryStore::open(&path, "<path>/x.db".to_string()).expect("si apre");
    (store, path)
}

fn record(store: &mut HistoryStore, target: &Target, pattern: &str, when: &str) {
    let endpoint = ObservedEndpointRecord {
        id: format!("id-{}", pattern.len()),
        redacted_pattern: pattern.to_string(),
        methods: vec!["GET".to_string()],
        observation_count: 1,
        auth_observed: "not observable".to_string(),
        auth_observable_count: 0,
        classification: Some(Classification::Shadow),
    };
    let finding = FindingRecord {
        endpoint_id: Some(endpoint.id.clone()),
        declared_path: None,
        classification: "Shadow".to_string(),
        confidence: "high".to_string(),
        severity: "medium".to_string(),
        evidence: "not present in the declared inventory".to_string(),
    };
    store
        .record_run(
            target,
            when,
            "0.0.0",
            "0.7.0",
            r#"{"counts":{"endpoints_found":1,"lines_total":10},"shadow_version":"0.0.0","ruleset_version":"0.7.0"}"#,
            &[endpoint],
            &[finding],
        )
        .expect("registra");
}

// --- l'avversaria che conta più di tutte -----------------------------------

/// **Un path costruito ad arte non deve diventare codice** nel browser di chi
/// legge il report.
///
/// È §P5 rovesciato: non un segreto che esce, un'iniezione che entra. Il
/// pattern di un endpoint viene dai dati analizzati, cioè da chi ha scritto le
/// richieste — che su uno strumento di sicurezza è, per definizione, qualcuno
/// di cui non ci si fida.
#[test]
fn avversaria_un_path_che_prova_a_chiudere_un_tag_non_diventa_codice() {
    let (mut store, path) = store("iniezione");
    let target = Target::default();
    let ostile = "/api/</table><script>alert(1)</script><table>/x";
    record(&mut store, &target, ostile, "2026-09-06T10:00:00+00:00");

    let status = Status::read(&store, &target).expect("legge");
    let pagina = shadow_view::page(&status, None);

    assert!(
        !pagina.contains("<script>"),
        "un tag aperto dai dati è arrivato nella pagina"
    );
    assert!(
        !pagina.contains("</table><script>"),
        "la sequenza ostile è passata intera"
    );
    // Il path resta **leggibile**: si neutralizza, non si cancella. Chi guarda
    // deve poter vedere che qualcuno ha provato.
    assert!(pagina.contains("&lt;script&gt;"), "{pagina}");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn le_virgolette_e_gli_apici_sono_neutralizzati_anche_fuori_dagli_attributi() {
    // Si neutralizzano anche dove «non servirebbero»: il giorno in cui una di
    // queste stringhe finisse dentro un attributo, il buco esisterebbe già.
    let neutro = shadow_view::escape("a\"b'c<d>e&f");
    assert_eq!(neutro, "a&quot;b&#39;c&lt;d&gt;e&amp;f");
}

// --- la pagina è davvero autonoma ------------------------------------------

#[test]
fn la_pagina_non_va_a_prendere_niente_da_nessuna_parte() {
    let (mut store, path) = store("autonoma");
    let target = Target::default();
    record(&mut store, &target, "/api/users/{id}", "2026-09-06T10:00:00+00:00");
    let status = Status::read(&store, &target).expect("legge");
    let pagina = shadow_view::page(&status, None);

    // Una pagina che scarica qualcosa all'apertura contraddice l'offline-first
    // davanti agli occhi di chi la guarda.
    for vietato in ["http://", "https://", "<script", "<link", "<iframe", "@import", "url("] {
        assert!(
            !pagina.contains(vietato),
            "la pagina contiene '{vietato}', quindi non è autonoma"
        );
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn il_meta_refresh_c_e_solo_se_lo_si_chiede() {
    let (mut store, path) = store("refresh");
    let target = Target::default();
    record(&mut store, &target, "/api/x", "2026-09-06T10:00:00+00:00");
    let status = Status::read(&store, &target).expect("legge");

    assert!(!shadow_view::page(&status, None).contains("http-equiv=\"refresh\""));
    let con = shadow_view::page(&status, Some(30));
    assert!(con.contains("content=\"30\""), "{con}");

    let _ = std::fs::remove_file(&path);
}

/// §P4: stesso storico, stessa pagina. Un cruscotto che cambia da solo fra due
/// letture non si può confrontare con niente.
#[test]
fn la_pagina_e_deterministica() {
    let (mut store, path) = store("deterministica");
    let target = Target::default();
    record(&mut store, &target, "/api/a", "2026-09-06T10:00:00+00:00");
    record(&mut store, &target, "/api/bb", "2026-09-06T11:00:00+00:00");
    let status = Status::read(&store, &target).expect("legge");

    assert_eq!(
        shadow_view::page(&status, None),
        shadow_view::page(&status, None)
    );

    let _ = std::fs::remove_file(&path);
}

// --- la vista dice le cose giuste ------------------------------------------

#[test]
fn cosa_e_cambiato_riguarda_l_ultimo_giro_e_non_tutta_la_storia() {
    let (mut store, path) = store("cambiato");
    let target = Target::default();
    record(&mut store, &target, "/api/a", "2026-09-06T10:00:00+00:00");
    record(&mut store, &target, "/api/bb", "2026-09-06T11:00:00+00:00");

    let status = Status::read(&store, &target).expect("legge");
    // Nel secondo giro è comparso un endpoint solo: il primo era già noto.
    assert_eq!(status.new_in_last_run.len(), 1);
    assert_eq!(status.new_in_last_run[0].redacted_pattern, "/api/bb");
    // Ma l'inventario li contiene entrambi.
    assert_eq!(status.inventory.len(), 2);
    assert_eq!(status.open_alerts.len(), 2);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn l_andamento_e_ordinato_per_numero_di_run_non_per_orologio() {
    let (mut store, path) = store("andamento");
    let target = Target::default();
    record(&mut store, &target, "/api/a", "2026-09-06T12:00:00+00:00");
    // Il secondo run dice di essere successo **prima** del primo.
    record(&mut store, &target, "/api/bb", "2026-09-06T09:00:00+00:00");

    let status = Status::read(&store, &target).expect("legge");
    let numeri: Vec<i64> = status.trend.iter().map(|p| p.run_id).collect();
    assert_eq!(
        numeri,
        vec![1, 2],
        "se l'orologio riordinasse la storia, l'andamento racconterebbe una cosa mai successa"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn le_categorie_vuote_restano_visibili() {
    let (mut store, path) = store("zeri");
    let target = Target::default();
    record(&mut store, &target, "/api/a", "2026-09-06T10:00:00+00:00");
    let status = Status::read(&store, &target).expect("legge");

    let nomi: Vec<&str> = status.by_classification().iter().map(|(n, _)| *n).collect();
    assert_eq!(nomi, vec!["Shadow", "Zombie", "Known", "Undetermined", "no verdict"]);
    // Una categoria che sparisce quando è vuota fa credere che non esista.
    assert!(status.by_classification().iter().any(|(_, c)| *c == 0));

    let _ = std::fs::remove_file(&path);
}
