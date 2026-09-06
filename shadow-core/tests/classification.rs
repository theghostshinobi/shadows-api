//! Le verifiche della Fase 3 — la fase in cui il tool dà verdetti.
//!
//! La regola che tutti questi test presidiano è una sola: **un `Known`
//! sbagliato è peggio di cento `Undetermined`**. Un `Known` sbagliato è un
//! endpoint che nessuno guarderà più.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use shadow_core::{
    classify, Classification, Confidence, DeclaredEndpoint, DeclaredInventory,
    Finding, FindingSubject, ObservedAuth, ObservedInventory, ObservedInventoryBuilder,
    ObservedRequest, Severity, SourceRef,
};

const DAY: i64 = 86_400;

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("timestamp valido")
}

fn request(method: &str, raw_path: &str, auth: ObservedAuth, at: i64) -> ObservedRequest {
    ObservedRequest {
        method: method.to_string(),
        raw_path: raw_path.to_string(),
        query_params: BTreeMap::new(),
        auth,
        status_code: 200,
        timestamp: ts(at),
        source: SourceRef {
            file: PathBuf::from("access.log"),
            line_number: 1,
        },
    }
}

/// Un inventario osservato da coppie (metodo, path), tutte alla stessa ora.
fn observed(entries: &[(&str, &str)]) -> ObservedInventory {
    observed_spanning(entries, 0)
}

/// Come sopra, ma spalmando le richieste su `days` giorni.
fn observed_spanning(entries: &[(&str, &str)], days: i64) -> ObservedInventory {
    let mut builder = ObservedInventoryBuilder::new();
    for (i, (method, path)) in entries.iter().enumerate() {
        let offset = if entries.len() > 1 {
            days * DAY * i as i64 / (entries.len() as i64 - 1)
        } else {
            0
        };
        builder.observe(request(method, path, ObservedAuth::NotObservable, offset));
    }
    builder.build()
}

fn declared(entries: &[(&str, &[&str])]) -> DeclaredInventory {
    DeclaredInventory::from_endpoints(entries.iter().map(|(path, methods)| DeclaredEndpoint {
        path_pattern: path.to_string(),
        methods: methods.iter().map(|m| m.to_string()).collect(),
        declared_auth_schemes: BTreeSet::new(),
    }))
}

fn of(findings: &[Finding], classification: Classification) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|f| f.classification == classification)
        .collect()
}

fn subject_paths(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .filter_map(|f| match &f.subject {
            FindingSubject::DeclaredEndpoint { path_pattern, .. } => Some(path_pattern.clone()),
            FindingSubject::ObservedEndpoint(_) => None,
        })
        .collect()
}

// --- con specifica dichiarata ----------------------------------------------

#[test]
fn un_endpoint_osservato_e_assente_dalla_specifica_e_shadow() {
    let observed = observed(&[("GET", "/api/admin/reset")]);
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);
    let shadow = of(&findings, Classification::Shadow);

    assert_eq!(shadow.len(), 1);
    assert_eq!(shadow[0].confidence, Confidence::High);
    assert!(!shadow[0].is_ambiguous);
    assert!(shadow[0].evidence.iter().any(|e| e.contains("not present in the declared inventory")));
}

#[test]
fn un_endpoint_che_combacia_per_intero_e_known() {
    let observed = observed(&[("GET", "/api/users/1"), ("GET", "/api/users/2"), ("GET", "/api/users/3")]);
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);

    assert_eq!(of(&findings, Classification::Known).len(), 1);
    assert!(of(&findings, Classification::Shadow).is_empty());
}

#[test]
fn avversaria_un_openapi_stantio_non_fa_passare_per_known_un_endpoint_pericoloso() {
    // La fixture avversaria obbligatoria di §9: la specifica dichiara
    // `/api/users/{userId}`, e nel traffico compare `/api/users/admin`. Un
    // confronto permissivo lo direbbe `Known`, il team si fiderebbe, e un
    // endpoint amministrativo resterebbe fuori da ogni controllo per sempre.
    let observed = observed(&[
        ("GET", "/api/users/1"),
        ("GET", "/api/users/2"),
        ("GET", "/api/users/3"),
        ("GET", "/api/users/admin"),
    ]);
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);
    let undetermined = of(&findings, Classification::Undetermined);

    assert_eq!(undetermined.len(), 1, "l'endpoint amministrativo deve essere segnalato");
    assert!(undetermined[0].is_ambiguous);
    assert!(undetermined[0]
        .evidence
        .iter()
        .any(|e| e.contains("would hide an endpoint the spec does not describe")));

    // E soprattutto: non è finito fra i `Known`.
    let known: Vec<&str> = of(&findings, Classification::Known)
        .iter()
        .flat_map(|f| f.evidence.iter().map(String::as_str))
        .collect();
    assert!(!known.iter().any(|e| e.contains("admin")));
}

#[test]
fn un_metodo_non_dichiarato_su_un_path_dichiarato_non_e_known() {
    // Un'operazione non documentata dentro un endpoint documentato è
    // esattamente ciò che questo tool dovrebbe far vedere.
    let observed = observed(&[
        ("GET", "/api/users/1"),
        ("GET", "/api/users/2"),
        ("DELETE", "/api/users/3"),
    ]);
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);

    assert!(of(&findings, Classification::Known).is_empty());
    let undetermined = of(&findings, Classification::Undetermined);
    assert_eq!(undetermined.len(), 1);
    assert!(undetermined[0].evidence.iter().any(|e| e.contains("DELETE")));
}

#[test]
fn un_endpoint_dichiarato_e_non_visto_diventa_zombie_solo_con_una_finestra_sufficiente() {
    let declared = declared(&[
        ("/api/users/{userId}", &["GET"]),
        ("/api/legacy/export", &["GET"]),
    ]);

    // Finestra lunga: si può dire zombie.
    let lungo = observed_spanning(
        &[("GET", "/api/users/1"), ("GET", "/api/users/2"), ("GET", "/api/users/3")],
        90,
    );
    let findings = classify(&lungo, Some(&declared), 30);
    let zombie = of(&findings, Classification::Zombie);
    assert_eq!(subject_paths(&findings.clone()), vec!["/api/legacy/export"]);
    assert_eq!(zombie.len(), 1);
    assert_eq!(zombie[0].confidence, Confidence::High);
}

#[test]
fn con_una_finestra_troppo_corta_non_si_dice_zombie_si_dice_non_lo_so() {
    // "Non più osservato da oltre la finestra" è un'affermazione sul tempo: un
    // log che copre un'ora non permette di farla, e fingere di sì direbbe
    // zombie di tutto ciò che semplicemente non è stato chiamato stanotte.
    let declared = declared(&[
        ("/api/users/{userId}", &["GET"]),
        ("/api/legacy/export", &["GET"]),
    ]);
    let corto = observed_spanning(
        &[("GET", "/api/users/1"), ("GET", "/api/users/2"), ("GET", "/api/users/3")],
        1,
    );

    let findings = classify(&corto, Some(&declared), 30);

    assert!(of(&findings, Classification::Zombie).is_empty());
    let undetermined = of(&findings, Classification::Undetermined);
    assert_eq!(undetermined.len(), 1);
    assert!(undetermined[0]
        .evidence
        .iter()
        .any(|e| e.contains("too short to call it a zombie")));
}

#[test]
fn ogni_finding_porta_evidenza_confidenza_e_ambiguita_popolate() {
    // §6: un verdetto senza motivazione non è verificabile.
    let observed = observed(&[("GET", "/api/admin/reset"), ("GET", "/api/users/1")]);
    let declared = declared(&[("/api/users/{userId}", &["GET"]), ("/api/gone", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);

    assert!(!findings.is_empty());
    for finding in &findings {
        assert!(!finding.evidence.is_empty(), "finding senza evidenza: {finding:?}");
        assert!(finding.evidence.iter().all(|e| !e.trim().is_empty()));
        // `is_ambiguous` e confidenza sono sempre valorizzati per costruzione;
        // qui si verifica che l'ambiguità accompagni sempre l'incertezza.
        if finding.classification == Classification::Undetermined {
            assert!(finding.is_ambiguous, "un Undetermined non ambiguo è una contraddizione");
        }
    }
}

#[test]
fn un_endpoint_non_documentato_che_risponde_senza_auth_ha_severita_alta() {
    let mut builder = ObservedInventoryBuilder::new();
    builder.observe(request("GET", "/api/secret", ObservedAuth::Absent, 0));
    let observed = builder.build();
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let findings = classify(&observed, Some(&declared), 30);
    let shadow = of(&findings, Classification::Shadow);

    assert_eq!(shadow[0].severity, Severity::High);
    assert!(shadow[0].evidence.iter().any(|e| e.contains("no authentication observed")));
}

// --- senza specifica dichiarata --------------------------------------------

#[test]
fn senza_specifica_nessun_finding_e_shadow() {
    // §9, Fase 3: senza inventario dichiarato l'assenza non è verificabile. Si
    // può sospettare, non sapere.
    let observed = observed(&[("GET", "/api/debug/dump"), ("GET", "/internal/metrics")]);

    let findings = classify(&observed, None, 30);

    assert!(!findings.is_empty());
    assert!(findings
        .iter()
        .all(|f| f.classification == Classification::Undetermined));
    assert!(findings.iter().all(|f| f.is_ambiguous));
}

#[test]
fn senza_specifica_un_inventario_tranquillo_non_produce_rumore() {
    // Il fallimento opposto a quello della Fase 2: un tool che segnala tutto
    // viene spento, e a quel punto non segnala più niente.
    let observed = observed(&[
        ("GET", "/api/billing/invoices"),
        ("GET", "/api/users/1"),
        ("GET", "/health"),
    ]);

    assert!(classify(&observed, None, 30).is_empty());
}

#[test]
fn le_euristiche_sullassenza_di_auth_non_si_attivano_su_non_osservabile() {
    // Il vincolo di §9 Fase 3: se il formato non trasporta l'informazione,
    // un'euristica sull'assenza di auth sbaglierebbe sistematicamente. Meglio
    // spenta che sempre in errore.
    let mut builder = ObservedInventoryBuilder::new();
    builder.observe(request("GET", "/api/things/open", ObservedAuth::NotObservable, 0));
    builder.observe(request(
        "GET",
        "/api/things/guarded",
        ObservedAuth::Present {
            scheme: ObservedAuth::SCHEME_UNSPECIFIED.to_string(),
        },
        0,
    ));
    let observed = builder.build();

    assert!(
        classify(&observed, None, 30).is_empty(),
        "su NotObservable non si segnala l'assenza di auth"
    );
}

#[test]
fn un_endpoint_senza_auth_fra_fratelli_che_ce_lhanno_e_un_sospetto() {
    // L'euristica che §6 cita per nome. Da sola "senza auth" non dice niente:
    // diventa un segnale quando i fratelli l'autenticazione ce l'hanno.
    let mut builder = ObservedInventoryBuilder::new();
    builder.observe(request("GET", "/api/things/open", ObservedAuth::Absent, 0));
    builder.observe(request(
        "GET",
        "/api/things/guarded",
        ObservedAuth::Present {
            scheme: ObservedAuth::SCHEME_UNSPECIFIED.to_string(),
        },
        0,
    ));
    let observed = builder.build();

    let findings = classify(&observed, None, 30);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].classification, Classification::Undetermined);
    assert_eq!(findings[0].severity, Severity::High);
    assert!(findings[0]
        .evidence
        .iter()
        .any(|e| e.contains("sibling endpoint")));
}

#[test]
fn la_classificazione_e_deterministica() {
    // §P4: stesso input, stesso verdetto, stesso ordine.
    let observed = observed(&[("GET", "/api/debug/dump"), ("GET", "/api/admin/reset")]);
    let declared = declared(&[("/api/users/{userId}", &["GET"])]);

    let primo = classify(&observed, Some(&declared), 30);
    let secondo = classify(&observed, Some(&declared), 30);

    assert_eq!(primo, secondo);
}

// --- il difetto trovato dal collaudo: l'evidenza nominava un path a caso ----

/// Il caso reale, ricostruito dal rapporto di collaudo (COLLAUDO.md, «Un
/// difetto trovato leggendo i finding»).
///
/// Per `/api/v1/repos/collaudo/billing-service/issues/{id}` l'evidenza indicava
/// `/api/v1/repos/{owner}/{repo}/issues/comments` — il primo in ordine di
/// specifica — mentre il vicino ovvio è `/issues/{index}`. Il verdetto era
/// corretto e resta corretto; a essere sbagliata era la **spiegazione**, che in
/// un report è ciò su cui l'utente decide.
#[test]
fn fra_piu_match_parziali_l_evidenza_nomina_il_piu_vicino() {
    let observed = observed(&[
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/1"),
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/2"),
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/3"),
    ]);
    // L'ordine è quello che aveva ingannato: `comments` viene prima di
    // `{index}`, ed è più lontano.
    let declared = declared(&[
        ("/api/v1/repos/{owner}/{repo}/issues/comments", &["GET"][..]),
        ("/api/v1/repos/{owner}/{repo}/issues/{index}", &["GET"][..]),
    ]);

    let findings = classify(&observed, Some(&declared), 30);
    let undetermined = of(&findings, Classification::Undetermined);
    let osservati: Vec<&&Finding> = undetermined
        .iter()
        .filter(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .collect();
    assert_eq!(osservati.len(), 1);

    let evidenza = &osservati[0].evidence[0];
    assert!(
        evidenza.contains("/api/v1/repos/{owner}/{repo}/issues/{index}"),
        "l'evidenza deve nominare il più vicino, non il primo in ordine: {evidenza}"
    );
    assert!(
        !evidenza.contains("comments"),
        "il più lontano non deve comparire come 'closest': {evidenza}"
    );
}

#[test]
fn l_ordine_della_specifica_non_cambia_quale_path_viene_nominato() {
    // §P4: due specifiche con gli stessi endpoint in ordine diverso devono dare
    // la stessa evidenza. Prima non era così, ed è esattamente il difetto.
    let observed = observed(&[
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/1"),
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/2"),
        ("GET", "/api/v1/repos/collaudo/billing-service/issues/3"),
    ]);
    let avanti = declared(&[
        ("/api/v1/repos/{owner}/{repo}/issues/comments", &["GET"][..]),
        ("/api/v1/repos/{owner}/{repo}/issues/{index}", &["GET"][..]),
    ]);
    let indietro = declared(&[
        ("/api/v1/repos/{owner}/{repo}/issues/{index}", &["GET"][..]),
        ("/api/v1/repos/{owner}/{repo}/issues/comments", &["GET"][..]),
    ]);

    let evidenza = |inv: &DeclaredInventory| {
        classify(&observed, Some(inv), 30)
            .into_iter()
            .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
            .expect("un finding sull'osservato")
            .evidence[0]
            .clone()
    };
    assert_eq!(evidenza(&avanti), evidenza(&indietro));
}

#[test]
fn un_prefisso_che_regge_piu_a_lungo_vince_a_parita_di_divergenze() {
    // Due candidati con **una** sola divergenza ciascuno: quello che diverge
    // più tardi è più imparentato, e va nominato lui.
    let observed = observed(&[("GET", "/api/orders/summary")]);
    let declared = declared(&[
        ("/api/{resource}/summary", &["GET"][..]),
        ("/api/orders/{id}", &["GET"][..]),
    ]);

    let findings = classify(&observed, Some(&declared), 30);
    let finding = findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato");
    assert!(
        finding.evidence[0].contains("/api/orders/{id}"),
        "atteso il candidato che condivide il prefisso più lungo: {}",
        finding.evidence[0]
    );
}

#[test]
fn quando_piu_dichiarati_sono_vicini_uguali_lo_dice_invece_di_sceglierne_uno() {
    // Due candidati esattamente alla stessa distanza: nominarne uno e tacere
    // sull'altro rifarebbe il difetto in forma più educata. Si dice quanti sono
    // (§P9).
    let observed = observed(&[("GET", "/api/orders/summary")]);
    let declared = declared(&[
        ("/api/{resource}/summary", &["GET"][..]),
        ("/api/{kind}/summary", &["GET"][..]),
    ]);

    let findings = classify(&observed, Some(&declared), 30);
    let finding = findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato");
    assert!(
        finding
            .evidence
            .iter()
            .any(|e| e.contains("just as close")),
        "l'evidenza deve ammettere il pareggio: {:?}",
        finding.evidence
    );
}

#[test]
fn il_verdetto_non_cambia_perche_e_cambiata_la_spiegazione() {
    // La taratura tocca l'evidenza, non la classificazione: un match parziale
    // resta `Undetermined`, mai `Known` (§P9).
    let observed = observed(&[("GET", "/api/users/admin")]);
    let declared = declared(&[("/api/users/{id}", &["GET"][..])]);

    let findings = classify(&observed, Some(&declared), 30);
    let osservato = findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato");
    assert_eq!(osservato.classification, Classification::Undetermined);
    assert!(osservato.is_ambiguous);
    assert_eq!(osservato.confidence, Confidence::Medium);
}

// --- il verso della divergenza, che la prima stesura affermava a caso ------
//
// Trovato da una revisione avversaria: `PathMatch::Partial` è simmetrico, la
// frase che lo spiegava no. Diceva sempre «il dichiarato ha una variabile dove
// questo endpoint ha un valore fisso», anche quando era vero il contrario — e
// il contrario è il caso centrale del prodotto.

fn evidenza_del_solo_osservato(findings: &[Finding]) -> String {
    findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato")
        .evidence[0]
        .clone()
}

#[test]
fn quando_la_variabile_ce_lha_l_osservato_l_evidenza_lo_dice_nel_verso_giusto() {
    // Traffico su `/api/users/1|2|3` → `/api/users/{id}`, specifica che
    // dichiara solo `/api/users/me`. È l'endpoint per-id non documentato: il
    // caso per cui il prodotto esiste.
    let observed = observed(&[
        ("GET", "/api/users/1"),
        ("GET", "/api/users/2"),
        ("GET", "/api/users/3"),
    ]);
    let declared = declared(&[("/api/users/me", &["GET"][..])]);

    let evidenza = evidenza_del_solo_osservato(&classify(&observed, Some(&declared), 30));
    assert!(
        evidenza.contains("has a fixed segment where this endpoint has a variable one"),
        "il verso è invertito: {evidenza}"
    );
}

#[test]
fn quando_la_variabile_ce_lha_il_dichiarato_l_evidenza_resta_quella_di_prima() {
    let observed = observed(&[("GET", "/api/users/admin")]);
    let declared = declared(&[("/api/users/{id}", &["GET"][..])]);

    let evidenza = evidenza_del_solo_osservato(&classify(&observed, Some(&declared), 30));
    assert!(
        evidenza.contains("has a variable segment where this endpoint has a fixed one"),
        "{evidenza}"
    );
}

#[test]
fn quando_divergono_in_tutti_e_due_i_versi_l_evidenza_non_ne_sceglie_uno() {
    // `/api/{tenant}/users/admin` osservato contro `/api/acme/users/{id}`
    // dichiarato: una posizione diverge in un verso, l'altra nell'altro.
    // Raccontarne una sola sarebbe raccontarne mezza.
    let observed = observed(&[
        ("GET", "/api/1/users/admin"),
        ("GET", "/api/2/users/admin"),
        ("GET", "/api/3/users/admin"),
    ]);
    let declared = declared(&[("/api/acme/users/{id}", &["GET"][..])]);

    let evidenza = evidenza_del_solo_osservato(&classify(&observed, Some(&declared), 30));
    assert!(
        evidenza.contains("differs in both directions"),
        "{evidenza}"
    );
}

// --- il buco del parametro che attraversa le barre -------------------------
//
// Trovato dal passo umano del collaudo: 18 dei 61 finding `Shadow` di Gitea
// erano richieste a `/contents/<percorso/di/file>`, dove la specifica dichiara
// `{filepath}`. Un parametro che contiene barre esiste davvero — ogni API che
// serve file lo fa — e OpenAPI non ha modo di dichiararlo. Il tool affermava
// che quegli endpoint erano assenti dall'inventario, con confidenza alta, e
// faceva fallire una pipeline per questo.

#[test]
fn un_parametro_dichiarato_che_attraversa_le_barre_non_e_piu_shadow_conclamato() {
    let observed = observed(&[(
        "GET",
        "/api/v1/repos/collaudo/docs-portal/contents/config/app.yaml",
    )]);
    let declared = declared(&[(
        "/api/v1/repos/{owner}/{repo}/contents/{filepath}",
        &["GET"][..],
    )]);

    let findings = classify(&observed, Some(&declared), 30);
    let osservato = findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato");

    assert_eq!(osservato.classification, Classification::Undetermined);
    assert!(osservato.is_ambiguous);
    // L'evidenza dice **quanto** dovrebbe estendersi: «due segmenti» e «sette»
    // sono due cose diverse, e chi legge decide anche su questo.
    assert!(
        osservato.evidence[0].contains("spanned 2 path segments"),
        "{:?}",
        osservato.evidence
    );
    assert!(
        osservato.evidence[1].contains("OpenAPI cannot say"),
        "{:?}",
        osservato.evidence
    );
}

/// **La proprietà che rende accettabile la correzione.** Un endpoint davvero
/// non documentato, alla stessa profondità, resta `Shadow` conclamato: la
/// variabile finale non è un permesso di coprire tutto.
#[test]
fn un_endpoint_davvero_non_documentato_resta_shadow_conclamato() {
    let observed = observed(&[
        ("GET", "/api/v1/repos/collaudo/docs-portal/contents/config/app.yaml"),
        ("GET", "/api/v1/repos/collaudo/docs-portal/segreti/dump"),
    ]);
    let declared = declared(&[(
        "/api/v1/repos/{owner}/{repo}/contents/{filepath}",
        &["GET"][..],
    )]);

    let findings = classify(&observed, Some(&declared), 30);
    let shadow = of(&findings, Classification::Shadow);
    assert_eq!(shadow.len(), 1, "{:?}", findings);
    // `segreti` è un valore fisso dove il dichiarato ha `contents`: due valori
    // fissi diversi, e nessuna variabile finale può rimediare.
    assert!(shadow[0].evidence[0].contains("not present in the declared inventory"));
}

#[test]
fn un_dichiarato_che_finisce_con_un_segmento_fisso_non_si_estende() {
    // `/issues` non è una variabile: un path più lungo sotto di lui non è una
    // sua istanza, è un altro endpoint.
    let observed = observed(&[("GET", "/api/v1/repos/collaudo/frontend/issues/1/comments")]);
    let declared = declared(&[("/api/v1/repos/{owner}/{repo}/issues", &["GET"][..])]);

    let findings = classify(&observed, Some(&declared), 30);
    assert_eq!(of(&findings, Classification::Shadow).len(), 1);
}

#[test]
fn una_variabile_che_si_estende_non_copre_l_endpoint_dichiarato() {
    // Se lo coprisse, un `/contents/a/b` osservato basterebbe a far sembrare
    // vivo un `{filepath}` che nessuno chiama più: il match parziale non copre,
    // e questo non è diverso.
    let observed = observed_spanning(
        &[("GET", "/api/v1/files/collaudo/a/b")],
        90,
    );
    let declared = declared(&[("/api/v1/files/{path}", &["GET"][..])]);

    let findings = classify(&observed, Some(&declared), 30);
    let dichiarati = subject_paths(&findings);
    assert!(
        dichiarati.contains(&"/api/v1/files/{path}".to_string()),
        "il dichiarato deve restare non coperto: {dichiarati:?}"
    );
}

/// **Il prezzo della correzione, scritto perché sia visibile e rivedibile.**
///
/// Un endpoint davvero non documentato che sta *sotto* una collezione
/// dichiarata la cui ultima posizione è una variabile diventa `Undetermined`
/// invece di `Shadow`. Resta segnalato, con l'evidenza che nomina esattamente
/// l'assunzione che servirebbe — e il numero di segmenti da inghiottire rende
/// visibile quanto sia improbabile — ma **non fa più fallire una pipeline**.
///
/// È il prezzo che il blueprint paga già ovunque: `Shadow` significa «assente
/// dall'inventario dichiarato», e finché un path dichiarato potrebbe coprirlo
/// quell'assenza non è verificabile (§9, Fase 3). Se il prezzo risultasse
/// troppo alto, la leva esiste ed è nell'evidenza: limitare quanti segmenti una
/// variabile può inghiottire. È una decisione, e non l'ha presa l'agente.
#[test]
fn prezzo_noto_una_sottorisorsa_non_documentata_diventa_undetermined() {
    let observed = observed(&[("GET", "/api/users/1/comments/5")]);
    let declared = declared(&[("/api/users/{id}", &["GET"][..])]);

    let findings = classify(&observed, Some(&declared), 30);
    let osservato = findings
        .iter()
        .find(|f| matches!(f.subject, FindingSubject::ObservedEndpoint(_)))
        .expect("un finding sull'osservato");

    assert_eq!(
        osservato.classification,
        Classification::Undetermined,
        "se questo test fallisce, il prezzo è stato rivisto: aggiornare COLLAUDO.md"
    );
    // Ma resta detto quanto è improbabile: tre segmenti dentro un solo `{id}`.
    assert!(
        osservato.evidence[0].contains("spanned 3 path segments"),
        "{:?}",
        osservato.evidence
    );
}
