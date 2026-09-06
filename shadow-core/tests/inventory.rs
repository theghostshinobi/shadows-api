//! Le verifiche della Fase 2 — la fase in cui il tool può mentire senza che
//! nessuno se ne accorga (§9).
//!
//! Il fallimento da presidiare non è "non riconosce un identificatore": è il
//! **falso raggruppamento**, che è invisibile per costruzione. Quasi tutti i
//! test qui sotto esistono per dimostrare che qualcosa **non** è stato fuso.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use shadow_core::{
    AuthObservation, ObservedAuth, ObservedInventory, ObservedInventoryBuilder, ObservedRequest,
    SourceRef,
};

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("timestamp valido")
}

/// Una richiesta osservata, con i valori che al test non interessano già messi.
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

fn inventory_of(paths: &[&str]) -> ObservedInventory {
    let mut builder = ObservedInventoryBuilder::new();
    for (i, path) in paths.iter().enumerate() {
        builder.observe(request(
            "GET",
            path,
            ObservedAuth::NotObservable,
            1_700_000_000 + i as i64,
        ));
    }
    builder.build()
}

fn patterns(inventory: &ObservedInventory) -> Vec<String> {
    inventory
        .iter()
        .map(|e| e.path_pattern.clone())
        .collect()
}

// --- il caso che deve funzionare -------------------------------------------

#[test]
fn tre_identificatori_distinti_collassano_in_un_solo_pattern() {
    // §9, criterio di riuscita esplicito.
    let inventory = inventory_of(&["/api/users/1", "/api/users/2", "/api/users/3"]);

    assert_eq!(patterns(&inventory), vec!["/api/users/{id}"]);
    assert_eq!(inventory.iter().next().unwrap().observation_count, 3);
}

#[test]
fn identificatori_annidati_collassano_a_ogni_livello() {
    // Senza raggruppare per il **pattern** già deciso, il secondo livello non
    // emergerebbe mai: ogni id del primo livello sarebbe un prefisso a sé.
    let inventory = inventory_of(&[
        "/api/users/1/orders/91",
        "/api/users/2/orders/92",
        "/api/users/3/orders/93",
    ]);

    assert_eq!(patterns(&inventory), vec!["/api/users/{id}/orders/{id}"]);
}

#[test]
fn uuid_e_forme_esadecimali_opache_sono_riconosciute() {
    let inventory = inventory_of(&[
        "/api/jobs/3f2504e0-4f89-11d3-9a0c-0305e82c3301",
        "/api/jobs/9c858901-8a57-4791-81fe-4c455b099bc9",
        "/api/jobs/00000000-0000-0000-0000-000000000000",
    ]);

    assert_eq!(patterns(&inventory), vec!["/api/jobs/{id}"]);
}

// --- i casi che NON devono funzionare, che sono il punto della fase ---------

#[test]
fn avversaria_un_endpoint_admin_non_sparisce_fra_migliaia_di_identificatori() {
    // La fixture avversaria obbligatoria di §9, alla scala che il blueprint
    // descrive: se l'euristica fosse aggressiva, `/api/users/admin` finirebbe
    // dentro `/api/users/{id}` e non lo guarderebbe più nessuno.
    let mut paths: Vec<String> = (1..=5_000).map(|i| format!("/api/users/{i}")).collect();
    paths.push("/api/users/admin".to_string());
    paths.push("/api/users/me".to_string());
    paths.push("/api/users/current".to_string());
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    let inventory = inventory_of(&refs);
    let found = patterns(&inventory);

    assert!(found.contains(&"/api/users/{id}".to_string()));
    assert!(
        found.contains(&"/api/users/admin".to_string()),
        "l'endpoint amministrativo è sparito: {found:?}"
    );
    assert!(found.contains(&"/api/users/me".to_string()));
    assert!(found.contains(&"/api/users/current".to_string()));
    assert_eq!(found.len(), 4, "un pattern per gli id e uno per ogni parola");
}

#[test]
fn sotto_soglia_non_si_aggrega() {
    // Due soli valori distinti non sono evidenza forte: §P3 dice che nel dubbio
    // non si aggrega. Il costo è due righe che sono la stessa cosa, e l'utente
    // se ne accorge; il costo opposto è un endpoint che sparisce, e non se ne
    // accorge nessuno.
    let inventory = inventory_of(&["/api/users/1", "/api/users/2"]);

    assert_eq!(patterns(&inventory), vec!["/api/users/1", "/api/users/2"]);
}

#[test]
fn endpoint_realmente_diversi_restano_separati() {
    let inventory = inventory_of(&[
        "/api/users/1",
        "/api/users/2",
        "/api/users/3",
        "/api/orders/1",
        "/api/orders/2",
        "/api/orders/3",
        "/health",
    ]);

    let found = patterns(&inventory);
    assert!(found.contains(&"/api/users/{id}".to_string()));
    assert!(found.contains(&"/api/orders/{id}".to_string()));
    assert!(found.contains(&"/health".to_string()));
    assert_eq!(found.len(), 3);
}

#[test]
fn una_parola_esadecimale_corta_non_e_un_identificatore() {
    // `cafe`, `dead`, `face` sono esadecimali validi e sono parole: il catalogo
    // delle forme tipo-ID non include le lunghezze brevi proprio per questo.
    let inventory = inventory_of(&["/api/x/cafe", "/api/x/dead", "/api/x/face"]);

    assert_eq!(patterns(&inventory).len(), 3, "nessuna parola va inghiottita");
}

// --- encoding ---------------------------------------------------------------

#[test]
fn la_stessa_cosa_scritta_in_due_modi_e_un_endpoint_solo() {
    let inventory = inventory_of(&["/api/../secret", "/api/%2e%2e/secret"]);

    assert_eq!(patterns(&inventory), vec!["/api/../secret"]);
    assert_eq!(inventory.iter().next().unwrap().observation_count, 2);
}

#[test]
fn la_doppia_codifica_resta_distinta_perche_e_un_segnale() {
    // Decodificare a ripetizione cancellerebbe proprio ciò che chi legge questo
    // tool vuole vedere.
    let inventory = inventory_of(&["/api/../secret", "/api/%252e%252e/secret"]);

    assert_eq!(patterns(&inventory).len(), 2);
}

#[test]
fn un_separatore_codificato_non_fonde_due_cose_diverse() {
    // `/a%2fb` è **un** segmento, `/a/b` sono due: fonderli sarebbe un falso
    // raggruppamento, cioè l'errore che questa fase esiste per evitare.
    let inventory = inventory_of(&["/api/a%2fb", "/api/a/b"]);

    assert_eq!(patterns(&inventory).len(), 2);
    assert!(patterns(&inventory).contains(&"/api/a%2Fb".to_string()));
}

// --- identificatore stabile -------------------------------------------------

#[test]
fn lo_stesso_endpoint_riceve_lo_stesso_identificatore_in_run_diversi() {
    // §P4, ed è la condizione perché l'allowlist della Fase 4 funzioni.
    let primo = inventory_of(&["/api/users/1", "/api/users/2", "/api/users/3"]);
    let secondo = inventory_of(&["/api/users/7", "/api/users/8", "/api/users/9"]);

    let id_primo = primo.iter().next().unwrap().id.clone();
    let id_secondo = secondo.iter().next().unwrap().id.clone();

    assert_eq!(id_primo, id_secondo);
}

#[test]
fn lidentificatore_non_cambia_se_cambia_il_traffico() {
    // Se dipendesse dai metodi osservati, un endpoint su cui domani compare un
    // `POST` cambierebbe identità e uscirebbe dall'allowlist in cui l'utente
    // l'aveva messo.
    let base = inventory_of(&["/api/users/1", "/api/users/2", "/api/users/3"]);

    let mut builder = ObservedInventoryBuilder::new();
    for (i, path) in ["/api/users/4", "/api/users/5", "/api/users/6"].iter().enumerate() {
        builder.observe(request("GET", path, ObservedAuth::NotObservable, 1_700_000_000));
        builder.observe(request("POST", path, ObservedAuth::NotObservable, 1_700_000_000 + i as i64));
    }
    let con_piu_traffico = builder.build();

    assert_eq!(
        base.iter().next().unwrap().id,
        con_piu_traffico.iter().next().unwrap().id
    );
    assert_eq!(con_piu_traffico.iter().next().unwrap().methods.len(), 2);
}

// --- propagazione dell'autenticazione (§6, decisione 2) ---------------------

#[test]
fn se_nessuna_richiesta_e_osservabile_il_pattern_e_non_osservabile() {
    let inventory = inventory_of(&["/api/health"]);
    let endpoint = inventory.iter().next().unwrap();

    assert_eq!(endpoint.auth_observed, AuthObservation::NotObservable);
    assert_eq!(endpoint.auth_observable_count, 0);
    // "Non osservabile" non è "senza autenticazione".
    assert_ne!(endpoint.auth_observed, AuthObservation::No);
}

#[test]
fn il_verdetto_si_calcola_sulle_sole_osservabili_e_dice_quante_erano() {
    // Il caso che il fondatore ha voluto reso visibile: 2 richieste osservabili
    // su 5.000 sono un verdetto debole, e chi legge deve poterlo vedere.
    let mut builder = ObservedInventoryBuilder::new();
    for i in 0..4_998 {
        builder.observe(request("GET", "/api/thing", ObservedAuth::NotObservable, 1_700_000_000 + i));
    }
    builder.observe(request("GET", "/api/thing", ObservedAuth::Absent, 1_700_000_000));
    builder.observe(request("GET", "/api/thing", ObservedAuth::Absent, 1_700_000_000));
    let inventory = builder.build();
    let endpoint = inventory.iter().next().unwrap();

    assert_eq!(endpoint.auth_observed, AuthObservation::No);
    assert_eq!(endpoint.observation_count, 5_000);
    assert_eq!(endpoint.auth_observable_count, 2);
}

#[test]
fn auth_mista_quando_la_stessa_risorsa_risponde_in_entrambi_i_modi() {
    let mut builder = ObservedInventoryBuilder::new();
    builder.observe(request("GET", "/api/thing", ObservedAuth::Absent, 1_700_000_000));
    builder.observe(request(
        "GET",
        "/api/thing",
        ObservedAuth::Present {
            scheme: ObservedAuth::SCHEME_UNSPECIFIED.to_string(),
        },
        1_700_000_100,
    ));
    let inventory = builder.build();
    let endpoint = inventory.iter().next().unwrap();

    assert_eq!(endpoint.auth_observed, AuthObservation::Mixed);
    assert_eq!(endpoint.auth_observable_count, 2);
    assert_eq!(endpoint.first_seen, ts(1_700_000_000));
    assert_eq!(endpoint.last_seen, ts(1_700_000_100));
}

// --- proprietà che il collasso incrementale deve conservare -----------------

#[test]
fn il_risultato_non_dipende_dallordine_di_arrivo_delle_richieste() {
    // Con la decisione presa durante la lettura, l'ordine potrebbe contare: la
    // fusione retroattiva dei rami già creati è ciò che impedisce che conti
    // (§P4). Qui `admin` arriva prima della soglia, e i primi due
    // identificatori pure.
    let avanti = inventory_of(&[
        "/api/users/1",
        "/api/users/2",
        "/api/users/admin",
        "/api/users/3",
        "/api/users/4",
    ]);
    let indietro = inventory_of(&[
        "/api/users/4",
        "/api/users/3",
        "/api/users/admin",
        "/api/users/2",
        "/api/users/1",
    ]);

    assert_eq!(patterns(&avanti), patterns(&indietro));
    assert_eq!(
        patterns(&avanti),
        vec!["/api/users/admin", "/api/users/{id}"]
    );
    // I due identificatori visti prima della soglia non restano indietro come
    // pattern separati: vengono fusi retroattivamente.
    assert_eq!(
        avanti
            .iter()
            .find(|e| e.path_pattern == "/api/users/{id}")
            .unwrap()
            .observation_count,
        4
    );
}

#[test]
fn la_fusione_retroattiva_porta_con_se_i_sottoalberi() {
    // I rami creati prima della soglia hanno già dei figli: fondendoli, i figli
    // devono confluire, o il secondo livello non collasserebbe mai.
    let inventory = inventory_of(&[
        "/api/users/1/orders/10",
        "/api/users/2/orders/20",
        "/api/users/3/orders/30",
    ]);

    assert_eq!(patterns(&inventory), vec!["/api/users/{id}/orders/{id}"]);
    assert_eq!(inventory.iter().next().unwrap().observation_count, 3);
}

#[test]
fn un_path_letteralmente_uguale_al_segnaposto_non_si_confonde_con_esso() {
    // Un client mal configurato che manda il template invece del valore non
    // deve finire nello stesso pattern degli identificatori veri.
    let inventory = inventory_of(&[
        "/api/users/1",
        "/api/users/2",
        "/api/users/3",
        "/api/users/{id}",
    ]);

    let found = patterns(&inventory);
    assert!(found.contains(&"/api/users/{id}".to_string()));
    assert!(found.contains(&"/api/users/%7Bid%7D".to_string()));
    assert_eq!(found.len(), 2);
}

#[test]
fn un_path_con_decine_di_migliaia_di_segmenti_non_esaurisce_lo_stack() {
    // Un input che qualcuno può costruire apposta: la raccolta dei pattern è
    // iterativa proprio per questo.
    let deep = format!("/{}", vec!["a"; 20_000].join("/"));
    let inventory = inventory_of(&[&deep]);

    assert_eq!(inventory.len(), 1);
}

// --- i caratteri invisibili e di direzione ---------------------------------
//
// Trovati da una revisione avversaria. Non erano un buco nella normalizzazione:
// erano un buco in **chi legge**. Due endpoint diversi uscivano
// tipograficamente identici, e un path poteva presentarsi come qualcosa che non
// è, dentro un documento che va in mano a un auditor.

/// **Il caso che ha fatto scattare la correzione.** `/api/admin` e
/// `/api/ad<U+200B>min` sono due endpoint diversi e uscivano *identici a
/// vedersi*: un endpoint che sparisce alla vista di chi decide è il fallimento
/// che §P1 chiama il peggiore del prodotto.
#[test]
fn avversaria_uno_spazio_a_larghezza_zero_non_rende_due_endpoint_identici_a_vedersi() {
    let inventario = inventory_of(&["/api/admin", "/api/ad\u{200B}min"]);
    let visti = patterns(&inventario);

    assert_eq!(visti.len(), 2, "sono e restano due endpoint distinti");
    assert!(visti.contains(&"/api/admin".to_string()), "{visti:?}");
    assert!(
        visti.contains(&"/api/ad%E2%80%8Bmin".to_string()),
        "l'invisibile deve diventare visibile: {visti:?}"
    );
    // E nessuno dei due contiene più un carattere che non si vede.
    for pattern in &visti {
        assert!(
            !pattern.chars().any(|c| c == '\u{200B}'),
            "un carattere invisibile è sopravvissuto: {pattern:?}"
        );
    }
}

/// **Il «o peggio» del commento di `reencode`.** `U+202E` rovescia il testo che
/// segue: `/api/<U+202E>txt.exe` si legge `/api/exe.txt`. Un path che si
/// presenta come qualcosa che non è, in un report d'audit.
#[test]
fn avversaria_un_override_bidirezionale_non_puo_far_leggere_un_path_al_contrario() {
    let inventario = inventory_of(&["/api/\u{202E}txt.exe"]);
    let visti = patterns(&inventario);
    assert_eq!(visti, vec!["/api/%E2%80%AEtxt.exe".to_string()]);
}

#[test]
fn avversaria_gli_altri_invisibili_non_passano() {
    // Indicatore di ordine dei byte, spazio insecabile, trattino morbido,
    // isolamento bidirezionale, e un controllo C1 — che il controllo per byte
    // di prima si lasciava sfuggire.
    for (ostile, atteso) in [
        ("/api/\u{FEFF}admin", "/api/%EF%BB%BFadmin"),
        ("/api/\u{00A0}admin", "/api/%C2%A0admin"),
        ("/api/ad\u{00AD}min", "/api/ad%C2%ADmin"),
        ("/api/\u{2066}admin", "/api/%E2%81%A6admin"),
        ("/api/\u{009B}admin", "/api/%C2%9Badmin"),
    ] {
        let visti = patterns(&inventory_of(&[ostile]));
        assert_eq!(visti, vec![atteso.to_string()], "su {ostile:?}");
    }
}

/// **Il difetto che questa correzione esiste per non causare.** Le lettere
/// accentate e gli alfabeti non latini si vedono: ri-codificarli renderebbe
/// illeggibile un path perfettamente normale.
#[test]
fn i_caratteri_che_si_vedono_restano_leggibili() {
    let visti = patterns(&inventory_of(&[
        "/api/città/anagrafe",
        "/api/日本/users",
        "/api/naïve",
        "/api/Ελλάδα",
    ]));
    for atteso in ["/api/città/anagrafe", "/api/日本/users", "/api/naïve", "/api/Ελλάδα"] {
        assert!(visti.contains(&atteso.to_string()), "{atteso} è stato rovinato: {visti:?}");
    }
}

/// La ri-codifica **non aggrega**: non è il falso raggruppamento di §P3 con un
/// altro nome.
///
/// Le due scritture dello **stesso** path — il carattere e la sua codifica
/// percentuale — restano un endpoint solo, ed è la trasparenza della codifica
/// che la Fase 2 applica di proposito: `..` e `%2e%2e` sono la stessa cosa. La
/// **doppia** codifica invece resta distinta, perché è un path diverso.
#[test]
fn ri_codificare_non_fonde_cose_diverse() {
    // Stesso path, due scritture: un endpoint.
    let stesso = patterns(&inventory_of(&["/api/ad\u{200B}min", "/api/ad%E2%80%8Bmin"]));
    assert_eq!(stesso, vec!["/api/ad%E2%80%8Bmin".to_string()]);

    // Path diverso: il segno di percentuale c'era davvero nella richiesta.
    let doppio = patterns(&inventory_of(&["/api/ad\u{200B}min", "/api/ad%25E2%2580%258Bmin"]));
    assert_eq!(doppio.len(), 2, "la doppia codifica non deve collassare: {doppio:?}");
}
