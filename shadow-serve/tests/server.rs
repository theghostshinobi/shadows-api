//! Le verifiche del server (§10, Fase 5).
//!
//! Il server è la sola cosa di questo progetto che apre un socket, quindi è la
//! sola parte in cui un difetto è raggiungibile da qualcun altro. Queste
//! verifiche esistono per quello.

use std::io::{Read, Write};
use std::net::TcpStream;

use shadow_history::{
    Classification, FindingRecord, HistoryStore, ObservedEndpointRecord, Target,
};
use shadow_serve::Server;

fn storico(name: &str) -> (HistoryStore, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("shadow-serve-{name}.db"));
    let _ = std::fs::remove_file(&path);
    let mut store = HistoryStore::open(&path, "<path>/x.db".to_string()).expect("si apre");
    let target = Target::default();
    store
        .record_run(
            &target,
            "2026-09-06T10:00:00+00:00",
            "0.0.0",
            "0.7.0",
            r#"{"counts":{"endpoints_found":1,"lines_total":3},"shadow_version":"0.0.0","ruleset_version":"0.7.0"}"#,
            &[ObservedEndpointRecord {
                id: "aaa".to_string(),
                redacted_pattern: "/api/admin/reset".to_string(),
                methods: vec!["POST".to_string()],
                observation_count: 1,
                auth_observed: "not observable".to_string(),
                auth_observable_count: 0,
                classification: Some(Classification::Shadow),
            }],
            &[FindingRecord {
                endpoint_id: Some("aaa".to_string()),
                declared_path: None,
                classification: "Shadow".to_string(),
                confidence: "high".to_string(),
                severity: "medium".to_string(),
                evidence: "not present in the declared inventory".to_string(),
            }],
        )
        .expect("registra");
    (store, path)
}

/// Manda una richiesta grezza e restituisce la risposta intera.
fn chiedi(address: &str, raw: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("si connette");
    stream.write_all(raw.as_bytes()).expect("scrive");
    stream.flush().expect("svuota");
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

/// Avvia un server che serve `n` richieste e poi si ferma, su una porta scelta
/// dal sistema.
fn con_server(name: &str, richieste: u64, prova: impl FnOnce(&str) + Send + 'static) {
    let (store, path) = storico(name);
    let server = Server::bind("127.0.0.1:0").expect("si lega");
    let address = server.address().to_string();
    let handle = std::thread::spawn(move || prova(&address));
    server
        .serve(&store, &Target::default(), None, Some(richieste))
        .expect("serve");
    handle.join().expect("la prova non è caduta");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn il_preimpostato_ascolta_solo_questa_macchina() {
    let server = Server::bind("127.0.0.1:0").expect("si lega");
    assert!(
        server.is_local_only(),
        "l'indirizzo preimpostato non deve essere raggiungibile da fuori"
    );
    assert!(server.address().starts_with("127.0.0.1:"));
}

#[test]
fn un_indirizzo_che_non_esiste_lo_dice_invece_di_partire() {
    let Err(errore) = Server::bind("questo-non-e-un-indirizzo") else {
        panic!("un indirizzo che non esiste non deve far partire un server");
    };
    let messaggio = errore.to_string();
    assert!(messaggio.contains("<host>:<port>"), "{messaggio}");
}

#[test]
fn la_pagina_esce_con_le_intestazioni_che_la_difendono() {
    con_server("pagina", 1, |address| {
        let risposta = chiedi(address, "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(risposta.starts_with("HTTP/1.1 200 OK"), "{risposta}");
        assert!(risposta.contains("Content-Type: text/html"), "{risposta}");
        // La seconda rete sotto la neutralizzazione dell'HTML: se un carattere
        // sfuggisse, questa riga impedisce al browser di eseguirlo.
        assert!(risposta.contains("default-src 'none'"), "{risposta}");
        assert!(risposta.contains("X-Content-Type-Options: nosniff"), "{risposta}");
        assert!(risposta.contains("/api/admin/reset"), "{risposta}");
    });
}

/// **Il server è in sola lettura.** Non c'è nessun percorso che scriva nello
/// storico: prendere in carico un alert resta una cosa che si fa dal terminale.
#[test]
fn avversaria_qualunque_cosa_non_sia_una_lettura_viene_rifiutata() {
    con_server("sola-lettura", 3, |address| {
        for metodo in ["POST", "PUT", "DELETE"] {
            let risposta = chiedi(
                address,
                &format!("{metodo} / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n"),
            );
            assert!(
                risposta.starts_with("HTTP/1.1 405"),
                "{metodo} non è stato rifiutato: {risposta}"
            );
            assert!(risposta.contains("read-only"), "{risposta}");
        }
    });
}

/// Non c'è nessun percorso da attraversare: si servono due percorsi fissi, e
/// niente di ciò che il client scrive raggiunge il filesystem.
#[test]
fn avversaria_un_attraversamento_di_percorsi_non_arriva_da_nessuna_parte() {
    con_server("percorsi", 4, |address| {
        for percorso in [
            "/../../etc/passwd",
            "/%2e%2e%2f%2e%2e%2fetc%2fpasswd",
            "/status.json/../../../etc/hosts",
            "/index.html%00.png",
        ] {
            let risposta = chiedi(address, &format!("GET {percorso} HTTP/1.1\r\nHost: x\r\n\r\n"));
            assert!(
                risposta.starts_with("HTTP/1.1 404"),
                "{percorso} non ha dato 404: {risposta}"
            );
            assert!(!risposta.contains("root:"), "ha servito un file di sistema");
        }
    });
}

#[test]
fn la_query_non_cambia_quale_pagina_si_serve() {
    con_server("query", 1, |address| {
        let risposta = chiedi(address, "GET /?x=1#y HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(risposta.starts_with("HTTP/1.1 200 OK"), "{risposta}");
    });
}

#[test]
fn lo_stato_in_json_e_servito_e_dichiarato_come_tale() {
    con_server("json", 1, |address| {
        let risposta = chiedi(address, "GET /status.json HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(risposta.contains("Content-Type: application/json"), "{risposta}");
        assert!(risposta.contains("\"open_alerts\""), "{risposta}");
    });
}

/// Una richiesta malformata non fa cadere il server: chiude quella connessione
/// e va avanti. Un cruscotto che muore perché qualcuno ha parlato male sulla
/// porta non è un cruscotto.
#[test]
fn avversaria_una_richiesta_malformata_non_ferma_il_server() {
    con_server("malformata", 2, |address| {
        let _ = chiedi(address, "questo non e' HTTP\r\n\r\n");
        // Il server è ancora lì.
        let risposta = chiedi(address, "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(risposta.starts_with("HTTP/1.1 200 OK"), "{risposta}");
    });
}
