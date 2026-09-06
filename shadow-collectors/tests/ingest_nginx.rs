//! Fixture dorate e avversarie della Fase 1 (§10).
//!
//! Le fixture avversarie non sono un extra: sono l'unico meccanismo che cattura
//! i falsi negativi silenziosi, e §10 le rende obbligatorie. Qui ci sono tutte
//! quelle che §9 elenca per la Fase 1: riga corrotta a metà file, file grande,
//! formato non riconosciuto, encoding sporco.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use shadow_collectors::{ingest_file, DiscardReason, IngestError, NginxCombinedCollector};
use shadow_core::ruleset::MAX_LINE_BYTES;
use shadow_core::{DigestAlgorithm, InputRole, ObservedAuth, ObservedRequest};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/nginx-combined")
        .join(name)
}

/// Esegue l'ingestione raccogliendo le richieste, per poterle ispezionare.
fn ingest(path: &Path) -> Result<(Vec<ObservedRequest>, shadow_collectors::IngestOutcome), IngestError>
{
    let mut requests = Vec::new();
    let outcome = ingest_file(
        path,
        InputRole::Log,
        &NginxCombinedCollector,
        &mut |r| requests.push(r),
    )?;
    Ok((requests, outcome))
}

fn reasons(outcome: &shadow_collectors::IngestOutcome) -> &BTreeMap<String, u64> {
    &outcome.counts.discarded_by_reason
}

// --- fixture dorata ---------------------------------------------------------

#[test]
fn un_log_nginx_valido_produce_le_richieste_osservate() {
    let (requests, outcome) = ingest(&fixture("valid.log")).expect("il formato è riconosciuto");

    assert_eq!(outcome.counts.total_lines, 5);
    assert_eq!(outcome.counts.parsed_lines, 5);
    assert_eq!(outcome.counts.discarded_lines, 0);
    assert!(reasons(&outcome).is_empty());

    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].raw_path, "/api/users/1");
    assert_eq!(requests[0].status_code, 200);

    // Il fuso viene portato a UTC, così righe di fusi diversi restano
    // confrontabili: `14:02:01 +0200` è `12:02:01Z`.
    assert_eq!(requests[3].timestamp.to_rfc3339(), "2023-10-10T12:02:01+00:00");
}

#[test]
fn una_chiave_di_query_ripetuta_conserva_tutti_i_suoi_valori() {
    // §6, allargamento confermato in v1.1: nessun valore va perso in silenzio.
    let (requests, _) = ingest(&fixture("valid.log")).expect("il formato è riconosciuto");
    let last = requests.last().expect("cinque richieste");

    assert_eq!(
        last.query_params.get("page"),
        Some(&vec!["2".to_string(), "3".to_string()])
    );
    // Un parametro senza `=` c'era comunque: "presente senza valore" non è
    // "assente".
    assert_eq!(last.query_params.get("debug"), Some(&vec![String::new()]));
}

#[test]
fn il_path_resta_grezzo_e_la_query_ne_esce_separata() {
    // Il Collector non normalizza: consegna il path com'era. Fondere
    // `/api/users/1` e `/api/users/2` in `/api/users/{id}` è compito della
    // normalizzazione, in `shadow-core`, non di chi legge il file.
    let (requests, _) = ingest(&fixture("valid.log")).expect("il formato è riconosciuto");
    let paths: Vec<&str> = requests.iter().map(|r| r.raw_path.as_str()).collect();

    assert!(paths.contains(&"/api/users/1"));
    assert!(paths.contains(&"/api/users/2"));
    assert!(!paths.iter().any(|p| p.contains('?')));
    assert!(!paths.iter().any(|p| p.contains("{id}")));
}

#[test]
fn su_un_formato_che_non_porta_gli_header_lauth_e_non_osservabile() {
    // La decisione chiusa in v1.1, verificata sul primo formato reale: il
    // `combined` di nginx non trasporta gli header, quindi "non osservabile" —
    // mai "assente" (§6, §P2).
    let (requests, _) = ingest(&fixture("valid.log")).expect("il formato è riconosciuto");

    assert_eq!(requests[0].auth, ObservedAuth::NotObservable);
    assert_ne!(requests[0].auth, ObservedAuth::Absent);

    // L'unica cosa che il formato dice davvero: `$remote_user` valorizzato
    // significa che un utente autenticato è stato registrato. Che schema fosse
    // il log non lo dice, quindi lo schema è **non specificato** (§6, v1.4):
    // il fatto si conserva, l'inferenza non si spaccia per osservazione.
    assert_eq!(
        requests[2].auth,
        ObservedAuth::Present {
            scheme: ObservedAuth::SCHEME_UNSPECIFIED.to_string()
        }
    );
    assert_ne!(
        requests[2].auth,
        ObservedAuth::Present {
            scheme: "Basic".to_string()
        }
    );
}

#[test]
fn il_digest_e_quello_del_file_calcolato_da_uno_strumento_indipendente() {
    // Il valore atteso viene da `shasum -a 256`: se il nostro digest in
    // streaming coincide, il calcolo in passata unica è corretto (§6).
    let (_, outcome) = ingest(&fixture("valid.log")).expect("il formato è riconosciuto");

    assert_eq!(outcome.digest.algorithm, DigestAlgorithm::Sha256);
    assert_eq!(
        outcome.digest.digest,
        "34feec5197120a4a25cc5108071c721f29288734764109180f0e0390f5f48813"
    );
}

// --- fixture avversarie obbligatorie (§9 Fase 1, §10) -----------------------

#[test]
fn avversaria_una_riga_corrotta_a_meta_file_non_perde_le_altre() {
    let (requests, outcome) =
        ingest(&fixture("corrupted-midfile.log")).expect("tre righe su sei sono valide");

    assert_eq!(outcome.counts.total_lines, 6);
    assert_eq!(outcome.counts.parsed_lines, 3);
    assert_eq!(outcome.counts.discarded_lines, 3);
    assert_eq!(requests.len(), 3);

    // Scartate **con il perché**: §6 lo mette nel contratto del manifest.
    assert_eq!(reasons(&outcome).get(DiscardReason::MalformedLine.as_str()), Some(&1));
    assert_eq!(reasons(&outcome).get(DiscardReason::InvalidStatusCode.as_str()), Some(&1));
    assert_eq!(reasons(&outcome).get(DiscardReason::InvalidTimestamp.as_str()), Some(&1));

    // La riga dopo quella rotta è stata analizzata: l'analisi prosegue.
    assert_eq!(requests[2].raw_path, "/api/orders");
}

#[test]
fn avversaria_i_byte_non_validi_sono_una_riga_scartata_non_un_crash() {
    let (requests, outcome) =
        ingest(&fixture("dirty-encoding.log")).expect("due righe su tre sono valide");

    assert_eq!(outcome.counts.total_lines, 3);
    assert_eq!(outcome.counts.parsed_lines, 2);
    assert_eq!(reasons(&outcome).get(DiscardReason::InvalidUtf8.as_str()), Some(&1));
    assert_eq!(requests[1].raw_path, "/api/after-the-bad-bytes");

    // Il digest è del **file**, non delle righe sopravvissute: anche i byte
    // scartati sono passati per l'hash.
    assert_eq!(
        outcome.digest.digest,
        "5691e9fa816199daab7d1d639067e7a38651d99cc42b077fcf4779c3532b5d0c"
    );
}

#[test]
fn avversaria_un_formato_non_riconosciuto_si_rifiuta_invece_di_indovinare() {
    // Il pericolo mortale di §9: leggere i campi dalla posizione sbagliata e
    // produrre numeri inventati che sembrano veri. La difesa è rifiutarsi.
    let error = ingest(&fixture("unrecognised-format.log")).expect_err("formato diverso");

    match &error {
        IngestError::FormatNotRecognized {
            format_name,
            lines_examined,
            first_offending_line,
            ..
        } => {
            assert_eq!(*format_name, "nginx-combined");
            assert_eq!(*lines_examined, 3);
            assert_eq!(*first_offending_line, Some(1));
        }
        other => panic!("atteso FormatNotRecognized, ottenuto {other:?}"),
    }

    let message = error.to_string();
    assert!(message.contains("does not look like the 'nginx-combined' log format"));
    assert!(message.contains("--log-format"));
    // Il messaggio non mostra il contenuto della riga: una riga di log può
    // contenere token ed email, e il `Redactor` è di Fase 4 (§P5).
    assert!(!message.contains("request_id=abc"));
}

#[test]
fn avversaria_una_riga_senza_fine_non_diventa_unallocazione_grande_come_il_file() {
    // §P10 aggirato da un input malformato: un file senza ritorni a capo.
    let path = temp_path("riga-infinita.log");
    let mut file = fs::File::create(&path).expect("file temporaneo");
    let mut giant = vec![b'x'; MAX_LINE_BYTES * 3];
    giant.push(b'\n');
    file.write_all(&giant).expect("scrittura");
    file.write_all(b"10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/after HTTP/1.1\" 200 2 \"-\" \"curl/8.1.2\"\n")
        .expect("scrittura");
    drop(file);

    let (requests, outcome) = ingest(&path).expect("la riga valida salva il riconoscimento");

    assert_eq!(outcome.counts.total_lines, 2);
    assert_eq!(reasons(&outcome).get(DiscardReason::LineTooLong.as_str()), Some(&1));
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].raw_path, "/api/after");

    fs::remove_file(&path).ok();
}

#[test]
fn avversaria_file_grande_la_memoria_non_cresce_con_la_dimensione() {
    // La prova che §9 prescrive: un file molto grande generato ripetendo righe.
    // Ciò che deve restare piatto è lo stato tenuto in memoria — qui misurato
    // come numero di richieste distinte — mentre il file cresce di dieci volte.
    let small = repeated_log("grande-1x.log", 2_000);
    let large = repeated_log("grande-10x.log", 20_000);

    let distinct_small = distinct_requests(&small);
    let distinct_large = distinct_requests(&large);

    assert_eq!(distinct_small.0, distinct_large.0, "lo stato non deve crescere col file");
    assert_eq!(distinct_small.0, 5);
    assert_eq!(distinct_large.1, distinct_small.1 * 10, "le righe sì, però");

    fs::remove_file(&small).ok();
    fs::remove_file(&large).ok();
}

// --- utilità di test --------------------------------------------------------

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("shadow-fase1-{name}"))
}

/// Genera un log grande ripetendo la fixture valida `times` volte.
fn repeated_log(name: &str, times: usize) -> PathBuf {
    let source = fs::read_to_string(fixture("valid.log")).expect("fixture leggibile");
    let path = temp_path(name);
    let mut file = fs::File::create(&path).expect("file temporaneo");
    for _ in 0..times {
        file.write_all(source.as_bytes()).expect("scrittura");
    }
    path
}

/// Restituisce `(richieste distinte, righe parsate)`.
fn distinct_requests(path: &Path) -> (usize, u64) {
    let mut distinct: BTreeMap<(String, String), u64> = BTreeMap::new();
    let outcome = ingest_file(path, InputRole::Log, &NginxCombinedCollector, &mut |r| {
        *distinct.entry((r.method, r.raw_path)).or_insert(0) += 1;
    })
    .expect("formato riconosciuto");
    (distinct.len(), outcome.counts.parsed_lines)
}

// --- avversaria della Fase 5: il log ruota mentre lo si legge ---------------
//
// Un demone legge log che ruotano sotto di lui: `logrotate` rinomina il file e
// ne crea uno nuovo, o lo tronca sul posto. §10 la rende obbligatoria per la
// Fase 5, e la sua sede è qui, dove vive la lettura.

/// Il file viene **rinominato** a metà lettura, come fa `logrotate` prima di
/// segnalare al server di riaprire.
///
/// Il lettore tiene aperto il descrittore, quindi continua sul contenuto che
/// stava leggendo: il run finisce sul file che aveva davanti, e il digest è di
/// quel contenuto. È la proprietà che serve a un manifest — dice cosa ha
/// analizzato — e sarebbe rotta da un lettore che riaprisse il percorso a metà
/// strada.
#[test]
fn avversaria_un_log_ruotato_a_meta_lettura_non_perde_ne_inventa_righe() {
    let path = temp_path("ruotato.log");
    let ruotato = temp_path("ruotato.log.1");
    let riga = "10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/users/1 HTTP/1.1\" 200 512 \"-\" \"UA\"\n";
    let contenuto = riga.repeat(500);
    fs::write(&path, &contenuto).expect("la fixture si scrive");
    let _ = fs::remove_file(&ruotato);

    let mut lette = 0_u64;
    let mut ruotato_una_volta = false;
    let outcome = ingest_file(&path, InputRole::Log, &NginxCombinedCollector, &mut |_| {
        lette += 1;
        // A metà del file, il log ruota sotto il lettore.
        if lette == 250 && !ruotato_una_volta {
            fs::rename(&path, &ruotato).expect("la rotazione riesce");
            fs::write(&path, riga).expect("il nuovo file nasce");
            ruotato_una_volta = true;
        }
    })
    .expect("la rotazione non fa cadere il run");

    // Nessuna riga persa e nessuna inventata: il run ha visto il file che aveva
    // aperto, tutto intero.
    assert_eq!(outcome.counts.total_lines, 500);
    assert_eq!(outcome.counts.parsed_lines, 500);
    assert_eq!(lette, 500);

    // E il digest è quello del contenuto letto, non del file nuovo: un manifest
    // che dicesse il contrario mentirebbe su cosa ha analizzato (§6).
    let atteso = {
        use sha2::Digest;
        sha2::Sha256::digest(contenuto.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(outcome.digest.digest, atteso);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&ruotato);
}

/// Il file viene **troncato sul posto** a metà lettura, come fa `logrotate`
/// con `copytruncate`.
///
/// Qui il contenuto sparisce davvero sotto il lettore, e non c'è modo di
/// leggerlo: la proprietà che si può pretendere è che il run **non cada** e che
/// i conteggi restino coerenti fra loro. Un tool che andasse in panico qui
/// sarebbe inservibile come demone.
#[test]
fn avversaria_un_log_troncato_a_meta_lettura_non_fa_cadere_il_run() {
    let path = temp_path("troncato.log");
    let riga = "10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/users/1 HTTP/1.1\" 200 512 \"-\" \"UA\"\n";
    fs::write(&path, riga.repeat(5_000)).expect("la fixture si scrive");

    let mut lette = 0_u64;
    let outcome = ingest_file(&path, InputRole::Log, &NginxCombinedCollector, &mut |_| {
        lette += 1;
        if lette == 100 {
            fs::write(&path, "").expect("il troncamento riesce");
        }
    })
    .expect("il troncamento non fa cadere il run");

    assert_eq!(
        outcome.counts.total_lines,
        outcome.counts.parsed_lines + outcome.counts.discarded_lines,
        "i conteggi devono restare coerenti anche su un file che si accorcia"
    );
    assert!(outcome.counts.parsed_lines >= 100);

    let _ = fs::remove_file(&path);
}

// --- lettura incrementale: la base del demone (§9, Fase 5) -----------------

/// Il digest cumulativo di due letture incrementali è **identico** a quello di
/// una lettura sola dello stesso file.
///
/// Non è pignoleria: il manifest di un ciclo dice cosa quel ciclo ha
/// analizzato, e i verdetti di quel ciclo coprono tutto ciò che si è visto
/// dall'inizio. Se il digest coprisse solo il pezzo nuovo, il manifest
/// dichiarerebbe mille byte mentre i finding ne coprono un milione (§6).
#[test]
fn il_digest_di_due_letture_incrementali_e_quello_del_file_intero() {
    let path = temp_path("incrementale.log");
    let riga = "10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/users/1 HTTP/1.1\" 200 512 \"-\" \"UA\"\n";
    fs::write(&path, riga.repeat(10)).expect("la fixture si scrive");

    let mut state = shadow_collectors::TailState::new();
    let mut richieste = 0;
    let primo = shadow_collectors::ingest_tail(
        &path,
        &mut state,
        &NginxCombinedCollector,
        &mut |_| richieste += 1,
    )
    .expect("legge");
    assert_eq!(primo.new_requests, 10);
    assert!(!primo.restarted);

    // Arrivano altre righe.
    let mut file = fs::OpenOptions::new().append(true).open(&path).expect("apre");
    file.write_all(riga.repeat(5).as_bytes()).expect("scrive");
    drop(file);

    let secondo = shadow_collectors::ingest_tail(
        &path,
        &mut state,
        &NginxCombinedCollector,
        &mut |_| richieste += 1,
    )
    .expect("legge");
    assert_eq!(secondo.new_requests, 5, "solo ciò che è cresciuto");
    assert_eq!(richieste, 15);
    assert_eq!(state.counts().total_lines, 15, "i conteggi sono cumulativi");

    // E il digest è quello del file intero.
    let (_, intero) = ingest(&path).expect("il formato è riconosciuto");
    assert_eq!(secondo.digest.digest, intero.digest.digest);

    let _ = fs::remove_file(&path);
}

/// Un log troncato sotto il demone viene **detto**, non ripreso in silenzio: se
/// si ricominciasse senza dirlo, tutto ciò che era già stato visto risulterebbe
/// nuovo, e l'alert perderebbe senso.
#[test]
fn avversaria_una_rotazione_fra_due_letture_viene_dichiarata() {
    let path = temp_path("rotazione-incrementale.log");
    let riga = "10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/users/1 HTTP/1.1\" 200 512 \"-\" \"UA\"\n";
    fs::write(&path, riga.repeat(10)).expect("la fixture si scrive");

    let mut state = shadow_collectors::TailState::new();
    let primo =
        shadow_collectors::ingest_tail(&path, &mut state, &NginxCombinedCollector, &mut |_| {})
            .expect("legge");
    assert!(!primo.restarted);
    assert_eq!(state.offset(), riga.len() as u64 * 10);

    // `logrotate --copytruncate`: il file si accorcia sotto il lettore.
    fs::write(&path, riga.repeat(2)).expect("il troncamento riesce");

    let mut richieste = 0;
    let secondo = shadow_collectors::ingest_tail(
        &path,
        &mut state,
        &NginxCombinedCollector,
        &mut |_| richieste += 1,
    )
    .expect("legge");
    assert!(secondo.restarted, "la rotazione deve essere dichiarata");
    assert_eq!(richieste, 2, "si riparte dall'inizio del file nuovo");
    assert_eq!(state.counts().total_lines, 2, "e i conteggi ripartono con lui");

    let _ = fs::remove_file(&path);
}

/// Una lettura incrementale su un file che non è cresciuto non produce niente e
/// non rompe niente.
#[test]
fn una_lettura_senza_niente_di_nuovo_non_fa_danni() {
    let path = temp_path("fermo.log");
    let riga = "10.0.0.1 - - [10/Oct/2023:13:55:36 +0000] \"GET /api/users/1 HTTP/1.1\" 200 512 \"-\" \"UA\"\n";
    fs::write(&path, riga).expect("la fixture si scrive");

    let mut state = shadow_collectors::TailState::new();
    let primo =
        shadow_collectors::ingest_tail(&path, &mut state, &NginxCombinedCollector, &mut |_| {})
            .expect("legge");
    let secondo =
        shadow_collectors::ingest_tail(&path, &mut state, &NginxCombinedCollector, &mut |_| {})
            .expect("legge");

    assert_eq!(secondo.new_requests, 0);
    assert!(!secondo.restarted);
    assert_eq!(primo.digest.digest, secondo.digest.digest);

    let _ = fs::remove_file(&path);
}
