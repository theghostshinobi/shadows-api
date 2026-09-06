//! Le fixture della grammatica **dichiarata** (§10).
//!
//! La fixture dorata di questo pezzo non me la sono inventata: è la tabella che
//! il collaudo ha misurato su corpus reale (COLLAUDO.md, §4). Sette varianti di
//! `log_format`, una sola accettata. Qui si verifica che restino sette e che
//! ora siano leggibili **dichiarandole**, senza che quella preimpostata si
//! allarghi di un millimetro.

use std::fs;
use std::path::{Path, PathBuf};

use shadow_collectors::{
    collector_for_format, ingest_file, Collector, DeclaredLogFormat, DiscardReason, FormatError,
    IngestError, LogFormatError, NginxCombinedCollector,
};
use shadow_core::{InputRole, ObservedAuth, ObservedRequest, SourceRef};

/// Il `combined`, nella sintassi con cui nginx lo dichiara.
const COMBINED: &str = NginxCombinedCollector::FORMAT_SPEC;

fn variante(nome: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/log-format-variants")
        .join(nome)
}

fn ingest(path: &Path, collector: &dyn Collector) -> Result<Vec<ObservedRequest>, IngestError> {
    let mut requests = Vec::new();
    ingest_file(path, InputRole::Log, collector, &mut |r| requests.push(r))?;
    Ok(requests)
}

/// Le sette varianti con la dichiarazione che le descrive, nell'ordine della
/// tabella di COLLAUDO.md.
fn varianti() -> Vec<(&'static str, String)> {
    vec![
        ("01-nginx-combined.log", COMBINED.to_string()),
        (
            "02-main-con-xforwardedfor.log",
            format!("{COMBINED} \"$http_x_forwarded_for\""),
        ),
        (
            "03-combined-con-request-time.log",
            format!("{COMBINED} $request_time"),
        ),
        (
            "04-combined-con-request-e-upstream-time.log",
            format!("{COMBINED} $request_time $upstream_response_time"),
        ),
        (
            "05-ingress-nginx-default.log",
            format!(
                "{COMBINED} $request_length $request_time [$proxy_upstream_name] \
                 [$proxy_alternative_upstream_name] $upstream_addr $upstream_response_length \
                 $upstream_response_time $upstream_status $req_id"
            ),
        ),
        (
            "06-apache-common.log",
            "$remote_addr - $remote_user [$time_local] \"$request\" $status $body_bytes_sent"
                .to_string(),
        ),
        (
            "07-request-time-in-testa.log",
            format!("$request_time {COMBINED}"),
        ),
    ]
}

// --- la fixture dorata: la tabella del collaudo, prima e dopo ---------------

#[test]
fn tutte_e_sette_le_varianti_reali_sono_leggibili_dichiarandole() {
    for (nome, dichiarazione) in varianti() {
        let collector = collector_for_format(&dichiarazione)
            .unwrap_or_else(|e| panic!("{nome}: la dichiarazione non compila: {e}"));
        let requests = ingest(&variante(nome), collector.as_ref())
            .unwrap_or_else(|e| panic!("{nome}: {e}"));

        // Ogni variante contiene le stesse tre richieste: cambia la riga
        // attorno, non il traffico. Se una variante desse un risultato diverso
        // dalle altre, vorrebbe dire che un campo è stato letto dalla posizione
        // sbagliata — cioè il difetto che §P2 vieta.
        assert_eq!(requests.len(), 3, "{nome}");
        assert_eq!(requests[0].method, "GET", "{nome}");
        assert_eq!(requests[0].raw_path, "/api/users/1", "{nome}");
        assert_eq!(requests[0].status_code, 200, "{nome}");
        assert_eq!(
            requests[0].timestamp.to_rfc3339(),
            "2023-10-10T13:55:31+00:00",
            "{nome}"
        );
    }
}

#[test]
fn senza_dichiarazione_solo_una_variante_su_sette_passa_come_prima() {
    let mut accettate = Vec::new();
    for (nome, _) in varianti() {
        if ingest(&variante(nome), &NginxCombinedCollector).is_ok() {
            accettate.push(nome);
        }
    }
    // È il punto di §7 su cui questa taratura non deve muoversi: il default può
    // esistere **solo perché** la validazione preimpostata è stretta. Se questo
    // test diventasse verde con sette, il default andrebbe rimosso nello stesso
    // momento.
    assert_eq!(accettate, vec!["01-nginx-combined.log"]);
}

#[test]
fn le_fixture_sono_identiche_al_corpus_del_collaudo() {
    for (nome, _) in varianti() {
        let copia = fs::read(variante(nome)).expect("la fixture esiste");
        let originale = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../collaudo/corpus/varianti-log-format")
                .join(nome),
        )
        .expect("il corpus del collaudo esiste");
        assert_eq!(copia, originale, "{nome}: la copia ha smesso di essere tale");
    }
}

// --- la grammatica dichiarata non è più larga di quella preimpostata --------

#[test]
fn la_grammatica_dichiarata_da_lo_stesso_risultato_di_quella_preimpostata() {
    let dichiarata = DeclaredLogFormat::parse(COMBINED).expect("il combined si dichiara");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/nginx-combined");

    for nome in ["valid.log", "secrets.log", "admin-hidden.log", "encoded-paths.log"] {
        let path = fixtures.join(nome);
        let preimpostata = ingest(&path, &NginxCombinedCollector).expect(nome);
        let dichiarata = ingest(&path, &dichiarata).expect(nome);
        assert_eq!(
            preimpostata.len(),
            dichiarata.len(),
            "{nome}: le due grammatiche hanno letto un numero diverso di righe"
        );
        for (a, b) in preimpostata.iter().zip(&dichiarata) {
            assert_eq!(a.method, b.method, "{nome}");
            assert_eq!(a.raw_path, b.raw_path, "{nome}");
            assert_eq!(a.query_params, b.query_params, "{nome}");
            assert_eq!(a.auth, b.auth, "{nome}");
            assert_eq!(a.status_code, b.status_code, "{nome}");
            assert_eq!(a.timestamp, b.timestamp, "{nome}");
        }
    }
}

#[test]
fn una_dichiarazione_con_un_campo_in_piu_rifiuta_il_log_senza_quel_campo() {
    // La difesa di §P2 vale anche per la grammatica dichiarata: se l'utente
    // dichiara `$request_time` e il file non ce l'ha, il file **non** è quello.
    let collector = collector_for_format(&format!("{COMBINED} $request_time")).expect("compila");
    let errore = ingest(&variante("01-nginx-combined.log"), collector.as_ref())
        .expect_err("manca un campo dichiarato");
    assert!(matches!(errore, IngestError::FormatNotRecognized { .. }));
}

#[test]
fn una_dichiarazione_con_un_campo_in_meno_rifiuta_il_log_che_ne_ha_di_piu() {
    let collector = collector_for_format(COMBINED).expect("compila");
    let errore = ingest(&variante("03-combined-con-request-time.log"), collector.as_ref())
        .expect_err("la riga ha un campo in più");
    assert!(matches!(errore, IngestError::FormatNotRecognized { .. }));
}

// --- le dichiarazioni che non devono nemmeno arrivare al file ---------------

#[test]
fn due_variabili_adiacenti_sono_rifiutate_in_compilazione() {
    // È il caso in cui non esiste modo di sapere dove finisce la prima:
    // accettarlo significherebbe leggere campi dalla posizione sbagliata.
    let errore = DeclaredLogFormat::parse("$remote_addr$status [$time_local] \"$request\"")
        .expect_err("grammatica ambigua");
    assert_eq!(
        errore,
        LogFormatError::AdjacentVariables {
            first: "remote_addr".to_string(),
            second: "status".to_string(),
        }
    );
}

#[test]
fn una_dichiarazione_che_non_descrive_una_richiesta_e_rifiutata() {
    let errore = DeclaredLogFormat::parse("$remote_addr $body_bytes_sent").expect_err("incompleta");
    let LogFormatError::MissingRequired { missing } = errore else {
        panic!("atteso MissingRequired");
    };
    // Tutte e tre le mancanze insieme: chi ha sbagliato tre variabili non deve
    // scoprirlo in tre esecuzioni.
    assert_eq!(missing.len(), 3);
}

#[test]
fn gli_escape_copiati_da_nginx_conf_sono_rifiutati_con_istruzioni() {
    let errore = DeclaredLogFormat::parse("$remote_addr [$time_local] \\\"$request\\\" $status")
        .expect_err("escape");
    assert_eq!(errore, LogFormatError::EscapeSequence);
    // Il messaggio deve dire cosa fare, non solo cosa non va.
    assert!(errore.to_string().contains("as it appears in the log line"));
}

#[test]
fn una_dichiarazione_vuota_e_rifiutata() {
    assert_eq!(
        DeclaredLogFormat::parse("   ").unwrap_err(),
        LogFormatError::Empty
    );
}

#[test]
fn un_dollaro_senza_nome_e_rifiutato() {
    assert!(matches!(
        DeclaredLogFormat::parse("$ [$time_local] \"$request\" $status"),
        Err(LogFormatError::EmptyVariableName { .. })
    ));
}

#[test]
fn un_nome_di_formato_inesistente_elenca_quelli_che_esistono_e_come_dichiarare() {
    let Err(errore) = collector_for_format("apache-combined") else {
        panic!("un nome che non esiste non può produrre un Collector");
    };
    assert_eq!(
        errore,
        FormatError::UnknownName("apache-combined".to_string())
    );
    let messaggio = errore.to_string();
    assert!(messaggio.contains("nginx-combined"));
    assert!(messaggio.contains("--log-format"));
}

// --- l'autenticazione, che è il punto in cui si mente più facilmente --------

#[test]
fn senza_remote_user_l_auth_e_non_osservabile_non_assente() {
    // Una dichiarazione che non porta `$remote_user` non dice **niente**
    // sull'autenticazione. `NotObservable` non è `Absent`: confonderli
    // produrrebbe un dato inventato che sembra vero (§6, §P2).
    let collector =
        DeclaredLogFormat::parse("$remote_addr [$time_local] \"$request\" $status").expect("compila");
    let request = collector
        .parse_line(
            "10.0.0.1 [10/Oct/2023:13:55:31 +0000] \"GET /api/users/1 HTTP/1.1\" 200",
            SourceRef {
                file: PathBuf::from("x.log"),
                line_number: 1,
            },
        )
        .expect("la riga combacia");
    assert_eq!(request.auth, ObservedAuth::NotObservable);
}

#[test]
fn con_remote_user_valorizzato_l_auth_e_presente_con_schema_non_specificato() {
    let collector = DeclaredLogFormat::parse(COMBINED).expect("compila");
    let request = collector
        .parse_line(
            "10.0.0.1 - alice [10/Oct/2023:13:55:31 +0000] \"GET /api/users/1 HTTP/1.1\" 200 5 \"-\" \"UA\"",
            SourceRef {
                file: PathBuf::from("x.log"),
                line_number: 1,
            },
        )
        .expect("la riga combacia");
    match request.auth {
        ObservedAuth::Present { ref scheme } => {
            assert_eq!(scheme, ObservedAuth::SCHEME_UNSPECIFIED);
        }
        altro => panic!("atteso Present, trovato {altro:?}"),
    }
    // E il nome dell'utente non entra da nessuna parte nel modello.
    assert!(!format!("{request:?}").contains("alice"));
}

#[test]
fn il_tempo_iso8601_e_letto_e_portato_a_utc() {
    let collector =
        DeclaredLogFormat::parse("$time_iso8601 $remote_addr \"$request\" $status").expect("compila");
    let request = collector
        .parse_line(
            "2023-10-10T15:55:31+02:00 10.0.0.1 \"GET /api/users/1 HTTP/1.1\" 200",
            SourceRef {
                file: PathBuf::from("x.log"),
                line_number: 1,
            },
        )
        .expect("la riga combacia");
    assert_eq!(request.timestamp.to_rfc3339(), "2023-10-10T13:55:31+00:00");
}

#[test]
fn una_riga_che_non_combacia_con_la_dichiarazione_e_scartata_col_perche() {
    let collector = DeclaredLogFormat::parse(COMBINED).expect("compila");
    let esito = collector.parse_line(
        "questa non e' una riga di log",
        SourceRef {
            file: PathBuf::from("x.log"),
            line_number: 1,
        },
    );
    assert_eq!(esito.unwrap_err(), DiscardReason::MalformedLine);
}

#[test]
fn uno_status_non_numerico_nella_dichiarazione_ha_il_proprio_motivo_di_scarto() {
    // I motivi di scarto sono catalogo congelato (§7): una grammatica nuova non
    // può inventarne, e deve usare quelli giusti.
    let collector = DeclaredLogFormat::parse(COMBINED).expect("compila");
    let esito = collector.parse_line(
        "10.0.0.1 - - [10/Oct/2023:13:55:31 +0000] \"GET /a HTTP/1.1\" duecento 5 \"-\" \"UA\"",
        SourceRef {
            file: PathBuf::from("x.log"),
            line_number: 1,
        },
    );
    assert_eq!(esito.unwrap_err(), DiscardReason::InvalidStatusCode);
}

#[test]
fn uno_spazio_in_coda_alla_dichiarazione_non_rende_il_file_illeggibile() {
    // La riga arriva al Collector già senza terminatore e tagliata in coda: una
    // dichiarazione che finisse con uno spazio non combacerebbe con nessuna
    // riga, e l'utente si vedrebbe rifiutare l'intero file senza capire perché.
    let con_spazio = format!("{COMBINED}   ");
    let collector = collector_for_format(&con_spazio).expect("compila");
    let requests = ingest(&variante("01-nginx-combined.log"), collector.as_ref())
        .expect("lo spazio in coda non cambia la grammatica");
    assert_eq!(requests.len(), 3);
}

// --- l'ancoraggio di coda: il pericolo mortale di §9 Fase 1 -----------------
//
// Trovato da una revisione avversaria dopo la prima stesura della taratura, non
// da queste fixture: la prima versione lasciava che una variabile finale si
// mangiasse qualunque campo la dichiarazione non nominasse, e la riga veniva
// **accettata**. Questi test esistono perché non torni.

fn parse_line(spec: &str, line: &str) -> Result<ObservedRequest, DiscardReason> {
    DeclaredLogFormat::parse(spec)
        .expect("la dichiarazione compila")
        .parse_line(
            line,
            SourceRef {
                file: PathBuf::from("x.log"),
                line_number: 1,
            },
        )
}

#[test]
fn avversaria_una_variabile_finale_non_si_mangia_i_campi_non_dichiarati() {
    // L'utente dichiara quattro campi, la riga ne ha sei. Senza ancoraggio di
    // coda `$request_uri` diventava "/api/users 0.042 10.244.1.7:8080": un
    // endpoint che non esiste, con dentro il tempo di risposta.
    let esito = parse_line(
        "$time_iso8601 $status $request_method $request_uri",
        "2023-10-10T13:55:31+00:00 200 GET /api/users 0.042 10.244.1.7:8080",
    );
    assert_eq!(esito.unwrap_err(), DiscardReason::MalformedLine);
}

#[test]
fn avversaria_un_remote_user_finale_non_diventa_un_auth_inventata() {
    // Stesso difetto, conseguenza peggiore: `$remote_user` si mangiava
    // "- 0.042 0.038", che essendo diverso da "-" diventava
    // `ObservedAuth::Present` — un'autenticazione osservata dal nulla, che
    // abbassa la severità di uno `Shadow` da alta a media.
    let esito = parse_line(
        "[$time_local] \"$request\" $status $remote_user",
        "[10/Oct/2023:13:55:31 +0000] \"GET /a HTTP/1.1\" 200 - 0.042 0.038",
    );
    assert_eq!(esito.unwrap_err(), DiscardReason::MalformedLine);
}

#[test]
fn un_campo_racchiuso_fra_delimitatori_puo_contenere_spazi() {
    // L'ancoraggio guarda il **delimitatore**, non il nome: `[$time_local]` e
    // `"$request"` contengono spazi per costruzione e devono continuare a
    // passare, o la taratura avrebbe rotto il formato per cui esiste.
    let request = parse_line(
        NginxCombinedCollector::FORMAT_SPEC,
        "10.0.0.1 - - [10/Oct/2023:13:55:31 +0000] \"GET /a HTTP/1.1\" 200 5 \"-\" \"Mozilla/5.0 (X11; Linux)\"",
    )
    .expect("la riga combacia");
    assert_eq!(request.method, "GET");
    assert_eq!(request.timestamp.to_rfc3339(), "2023-10-10T13:55:31+00:00");
}

#[test]
fn avversaria_un_remote_user_vuoto_non_e_un_auth_osservata() {
    // Una colonna vuota è "nessun utente registrato", esattamente come `-`.
    // La grammatica preimpostata non può produrre un campo vuoto; quella
    // dichiarata sì, ed è da lì che passava un'auth inventata.
    let request = parse_line(
        "$remote_addr|$remote_user|[$time_local]|\"$request\"|$status",
        "10.0.0.1||[10/Oct/2023:13:55:31 +0000]|\"GET /a HTTP/1.1\"|200",
    )
    .expect("la riga combacia");
    assert_eq!(request.auth, ObservedAuth::NotObservable);
}

#[test]
fn avversaria_una_richiesta_assente_non_diventa_un_endpoint() {
    // Quando il client chiude prima di mandare la richiesta, nginx scrive `-`
    // in metodo e target. Il ramo `$request` lo rifiutava già (`-` non ha tre
    // parti); il ramo a campi separati no, perché `-` è un carattere ammesso in
    // un metodo HTTP: nasceva un endpoint `/-` mai esistito, e con una
    // specifica a fianco uno `Shadow` che fa fallire una pipeline.
    let esito = parse_line(
        "$remote_addr [$time_local] $request_method $request_uri $status",
        "10.0.0.1 [10/Oct/2023:13:55:31 +0000] - - 400",
    );
    assert_eq!(esito.unwrap_err(), DiscardReason::InvalidRequestLine);
}

#[test]
fn una_graffa_non_chiusa_lo_dice_invece_di_parlare_di_nome_mancante() {
    // Dire «un `$` senza nome» a chi il nome l'ha scritto manda a cercare il
    // problema dove non è.
    let errore = DeclaredLogFormat::parse("${remote_addr - $status [$time_local] \"$request\"")
        .expect_err("graffa non chiusa");
    assert_eq!(
        errore,
        LogFormatError::UnclosedBrace {
            at: 0,
            name: "remote_addr".to_string()
        }
    );
    assert!(errore.to_string().contains("${remote_addr}"));
}
