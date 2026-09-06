//! Le fixture della Fase 5 (§10), dorate e avversarie.
//!
//! Le avversarie obbligatorie di questa fase le ha elencate il fondatore:
//! orologio che va all'indietro, database corrotto o bloccato da un altro
//! processo, due esecuzioni concorrenti sullo stesso storico, retention che
//! cancella evidenza ancora citata da un finding. Ci sono tutte qui, tranne il
//! log ruotato durante la lettura, che è un'avversaria dell'**ingestione** e
//! sta in `shadow-collectors`.

use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use shadow_core::Classification;
use shadow_history::{FindingRecord, HistoryStore, ObservedEndpointRecord, Target};

fn temp(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("shadow-fase5-{name}.db"));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    path
}

fn store(name: &str) -> (HistoryStore, PathBuf) {
    let path = temp(name);
    let store = HistoryStore::open(&path, "<path>/storia.db".to_string()).expect("si apre");
    (store, path)
}

fn endpoint(id: &str, pattern: &str, classification: Option<Classification>) -> ObservedEndpointRecord {
    ObservedEndpointRecord {
        id: id.to_string(),
        redacted_pattern: pattern.to_string(),
        methods: vec!["GET".to_string()],
        observation_count: 1,
        // Il valore che il report d'audit non deve mai poter collassare in un
        // binario: qui è il caso preimpostato del `combined` di nginx, che
        // l'autenticazione non la trasporta affatto (§6).
        auth_observed: "not observable".to_string(),
        auth_observable_count: 0,
        classification,
    }
}

fn finding(endpoint_id: &str, classification: &str) -> FindingRecord {
    FindingRecord {
        endpoint_id: Some(endpoint_id.to_string()),
        declared_path: None,
        classification: classification.to_string(),
        confidence: "high".to_string(),
        severity: "medium".to_string(),
        evidence: "not present in the declared inventory".to_string(),
    }
}

fn record(
    store: &mut HistoryStore,
    target: &Target,
    timestamp: &str,
    endpoints: &[ObservedEndpointRecord],
) -> shadow_history::RecordedRun {
    let findings: Vec<FindingRecord> = endpoints
        .iter()
        .filter_map(|e| e.classification.map(|c| finding(&e.id, c.as_str())))
        .collect();
    store
        .record_run(target, timestamp, "0.0.0", "0.5.0", "{}", endpoints, &findings)
        .expect("si registra")
}

// --- fixture dorate ---------------------------------------------------------

#[test]
fn il_primo_run_vede_tutto_nuovo_il_secondo_niente() {
    let (mut store, _path) = store("dorata");
    let target = Target::default();
    let endpoints = vec![
        endpoint("aaa", "/api/users/{id}", Some(Classification::Known)),
        endpoint("bbb", "/api/admin/reset", Some(Classification::Shadow)),
    ];

    let primo = record(&mut store, &target, "2026-09-05T10:00:00+00:00", &endpoints);
    assert_eq!(primo.new_endpoints.len(), 2);

    let secondo = record(&mut store, &target, "2026-09-05T11:00:00+00:00", &endpoints);
    assert!(secondo.new_endpoints.is_empty(), "{:?}", secondo.new_endpoints);
    assert_eq!(store.run_count(&target).expect("conta"), 2);
}

#[test]
fn un_endpoint_che_compare_dopo_e_l_unico_nuovo() {
    let (mut store, _path) = store("nuovo");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/users/{id}", Some(Classification::Known))],
    );

    let secondo = record(
        &mut store,
        &target,
        "2026-09-05T11:00:00+00:00",
        &[
            endpoint("aaa", "/api/users/{id}", Some(Classification::Known)),
            endpoint("ccc", "/api/debug/dump", Some(Classification::Shadow)),
        ],
    );
    assert_eq!(secondo.new_endpoints.len(), 1);
    assert_eq!(secondo.new_endpoints[0].id, "ccc");

    // È shadow e nessuno l'ha preso in carico: è un alert.
    let alerts = store.pending_alerts(&target).expect("legge");
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].redacted_pattern, "/api/debug/dump");
}

#[test]
fn un_endpoint_senza_verdetto_viene_comunque_ricordato() {
    // Un run senza inventario dichiarato che non trova niente di sospetto non
    // dice niente su un endpoint (§9, Fase 3). Se lo storico lo dimenticasse,
    // quell'endpoint tornerebbe "nuovo" a ogni esecuzione, per sempre.
    let (mut store, _path) = store("senza-verdetto");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/users/{id}", None)],
    );
    let secondo = record(
        &mut store,
        &target,
        "2026-09-05T11:00:00+00:00",
        &[endpoint("aaa", "/api/users/{id}", None)],
    );
    assert!(secondo.new_endpoints.is_empty());
}

#[test]
fn prendere_in_carico_toglie_dall_elenco_senza_cancellare() {
    let (mut store, _path) = store("carico");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("bbb", "/api/admin/reset", Some(Classification::Shadow))],
    );
    assert_eq!(store.pending_alerts(&target).expect("legge").len(), 1);

    assert!(store.acknowledge(&target, "bbb").expect("segna"));
    assert!(store.pending_alerts(&target).expect("legge").is_empty());
    // L'evidenza resta: silenziare non è cancellare (§5, Fase 4).
    assert_eq!(store.run_count(&target).expect("conta"), 1);
    assert!(!store.acknowledge(&target, "non-esiste").expect("segna"));
}

#[test]
fn i_bersagli_non_si_mescolano_mai() {
    // È ciò che «multi-tenancy» significa qui: separazione dei bersagli.
    let (mut store, _path) = store("bersagli");
    let uno = Target::new("fatturazione").expect("nome valido");
    let due = Target::new("anagrafica").expect("nome valido");

    record(
        &mut store,
        &uno,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/invoices", Some(Classification::Shadow))],
    );
    // Lo stesso identificatore su un altro bersaglio è **nuovo**, perché è un
    // altro servizio: fondere i due inventari sarebbe il falso raggruppamento
    // di §P3 un piano più su.
    let altro = record(
        &mut store,
        &due,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/people", Some(Classification::Shadow))],
    );
    assert_eq!(altro.new_endpoints.len(), 1);
    assert_eq!(store.pending_alerts(&uno).expect("legge").len(), 1);
    assert_eq!(store.pending_alerts(&due).expect("legge").len(), 1);
    assert_eq!(store.run_count(&uno).expect("conta"), 1);
    assert_eq!(store.targets().expect("elenca").len(), 2);
}

#[test]
fn un_nome_di_bersaglio_inutilizzabile_e_rifiutato() {
    assert!(Target::new("").is_err());
    assert!(Target::new(" spazio ").is_err());
    assert!(Target::new("fatturazione").is_ok());
}

// --- fixture avversarie obbligatorie (§10) ----------------------------------

#[test]
fn avversaria_l_orologio_che_va_all_indietro_viene_detto_non_aggiustato() {
    let (mut store, _path) = store("orologio");
    let target = Target::default();
    let e = vec![endpoint("aaa", "/api/users/{id}", Some(Classification::Known))];

    let primo = record(&mut store, &target, "2026-09-05T12:00:00+00:00", &e);
    assert!(!primo.clock_went_backwards);

    // Il secondo run dice di essere successo **prima** del primo.
    let secondo = record(&mut store, &target, "2026-09-05T09:00:00+00:00", &e);
    assert!(
        secondo.clock_went_backwards,
        "un orologio all'indietro deve essere segnalato, non corretto"
    );
    // E l'ordine non ne risente: lo decide l'identificatore progressivo, non il
    // tempo. Se lo decidesse il tempo, un endpoint già visto tornerebbe nuovo.
    assert!(secondo.run_id > primo.run_id);
    assert!(secondo.new_endpoints.is_empty());
}

#[test]
fn avversaria_due_esecuzioni_concorrenti_si_serializzano() {
    let path = temp("concorrenti");
    // Il file deve esistere con lo schema prima che i due thread lo aprano.
    drop(HistoryStore::open(&path, "<path>/x.db".to_string()).expect("si apre"));

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for n in 0..2 {
        let path = path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            let mut store = HistoryStore::open(&path, "<path>/x.db".to_string()).expect("si apre");
            let target = Target::default();
            barrier.wait();
            record(
                &mut store,
                &target,
                "2026-09-05T10:00:00+00:00",
                &[endpoint(
                    &format!("id-{n}"),
                    &format!("/api/{n}"),
                    Some(Classification::Shadow),
                )],
            );
        }));
    }
    for handle in handles {
        handle.join().expect("nessun thread è caduto");
    }

    // Nessuno dei due ha visto metà del lavoro dell'altro: due run interi, due
    // endpoint interi.
    let store = HistoryStore::open(&path, "<path>/x.db".to_string()).expect("si apre");
    let target = Target::default();
    assert_eq!(store.run_count(&target).expect("conta"), 2);
    assert_eq!(store.pending_alerts(&target).expect("legge").len(), 2);
}

#[test]
fn avversaria_un_database_corrotto_viene_rifiutato_con_un_messaggio_utile() {
    let path = temp("corrotto");
    std::fs::write(&path, b"questo non e' un database, e' un file di testo\n").expect("scrive");

    let Err(errore) = HistoryStore::open(&path, "<path>/storia.db".to_string()) else {
        panic!("un file che non è un database non si apre");
    };
    let messaggio = errore.to_string();
    assert!(messaggio.contains("<path>/storia.db"), "{messaggio}");
    assert!(
        messaggio.contains("not a readable Shadow history"),
        "il messaggio deve dire cosa fare: {messaggio}"
    );
    // §P5: nel messaggio compare il percorso **già redatto** che gli è stato
    // passato, mai uno costruito qui.
    assert!(!messaggio.contains(path.display().to_string().as_str()), "{messaggio}");
}

#[test]
fn avversaria_uno_storico_di_una_versione_piu_nuova_e_rifiutato() {
    let path = temp("futuro");
    {
        let connection = rusqlite::Connection::open(&path).expect("si apre");
        connection
            .pragma_update(None, "user_version", 99_i64)
            .expect("scrive la versione");
    }
    let Err(errore) = HistoryStore::open(&path, "<path>/storia.db".to_string()) else {
        panic!("uno schema più nuovo non si apre");
    };
    let messaggio = errore.to_string();
    assert!(messaggio.contains("schema version 99"), "{messaggio}");
    assert!(
        messaggio.contains("would corrupt an audit archive"),
        "{messaggio}"
    );
}

#[test]
fn avversaria_la_retention_non_puo_cancellare_evidenza_ancora_citata() {
    // La politica di retention è una decisione ancora aperta e non è stata
    // scritta. Ma la **proprietà** che qualunque retention dovrà rispettare sì:
    // un run a cui un endpoint dello stato corrente si appoggia ancora non si
    // può cancellare. Il vincolo di chiave esterna la rende impossibile invece
    // di affidarla a chi scriverà la retention.
    let (mut store, path) = store("retention");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/admin/reset", Some(Classification::Shadow))],
    );
    drop(store);

    let connection = rusqlite::Connection::open(&path).expect("si apre");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("accende i vincoli");
    let esito = connection.execute("DELETE FROM runs WHERE id = 1", []);
    assert!(
        esito.is_err(),
        "cancellare un run ancora citato da un endpoint deve fallire, non riuscire in silenzio"
    );

    // E lo storico resta leggibile e completo.
    let store = HistoryStore::open(&path, "<path>/storia.db".to_string()).expect("si apre");
    assert_eq!(store.pending_alerts(&target).expect("legge").len(), 1);
}

#[test]
fn avversaria_riaprire_uno_storico_gia_migrato_non_lo_tocca() {
    // Una migrazione applicata due volte è il modo in cui un archivio d'audit
    // si perde. Le migrazioni sono idempotenti perché guardano `user_version`.
    let (mut store, path) = store("migrazione");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/x", Some(Classification::Shadow))],
    );
    drop(store);

    for _ in 0..3 {
        let store = HistoryStore::open(&path, "<path>/storia.db".to_string()).expect("si riapre");
        assert_eq!(store.run_count(&target).expect("conta"), 1);
    }
}

#[test]
fn nello_storico_finisce_solo_ciò_che_gli_e_stato_dato_gia_redatto() {
    // §P5: uno storico su disco è una superficie nuova. Il tipo chiede valori
    // già redatti e questo test verifica che non ne inventi altri: ciò che si
    // rilegge è esattamente ciò che si è scritto.
    let (mut store, _path) = store("redazione");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/session/<redacted>", Some(Classification::Shadow))],
    );
    let alerts = store.pending_alerts(&target).expect("legge");
    assert_eq!(alerts[0].redacted_pattern, "/api/session/<redacted>");
}

// --- i difetti che la revisione avversaria ha trovato in questa fetta -------

/// **Il difetto grave.** L'alert è un *fatto avvenuto*: un run che su
/// quell'endpoint non ha niente da dire non può cancellarlo.
///
/// Prima l'alert si deduceva dall'ultima classificazione, e un run senza
/// inventario dichiarato — la modalità preimpostata di Shadow — scriveva NULL
/// e azzerava in silenzio la coda di chi la specifica ce l'aveva.
#[test]
fn avversaria_un_run_senza_verdetto_non_cancella_un_alert_pendente() {
    let (mut store, _path) = store("alert-non-cancellabile");
    let target = Target::default();

    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("bbb", "/api/admin/reset", Some(Classification::Shadow))],
    );
    assert_eq!(store.pending_alerts(&target).expect("legge").len(), 1);

    // Stesso endpoint, nessun verdetto: il run non aveva la specifica.
    record(
        &mut store,
        &target,
        "2026-09-05T11:00:00+00:00",
        &[endpoint("bbb", "/api/admin/reset", None)],
    );
    let dopo = store.pending_alerts(&target).expect("legge");
    assert_eq!(dopo.len(), 1, "l'alert non preso in carico deve restare");
    // E il verdetto precedente non è stato cancellato.
    assert_eq!(dopo[0].last_classification.as_deref(), Some("Shadow"));

    // Solo prenderlo in carico lo toglie.
    assert!(store.acknowledge(&target, "bbb").expect("segna"));
    assert!(store.pending_alerts(&target).expect("legge").is_empty());
}

/// Un endpoint che viene documentato — da `Shadow` a `Known` — resta in coda
/// finché qualcuno non lo prende in carico.
///
/// È la posizione conservativa: l'alert dice «questo endpoint è comparso senza
/// essere documentato», e resta un fatto avvenuto anche se poi qualcuno ha
/// aggiornato la specifica. Toglierlo da solo significherebbe che nessuno si
/// accorge di ciò che è successo (§P1, §P3).
#[test]
fn un_endpoint_documentato_dopo_resta_in_coda_finche_non_lo_si_prende_in_carico() {
    let (mut store, _path) = store("documentato-dopo");
    let target = Target::default();
    record(
        &mut store,
        &target,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("bbb", "/api/admin/reset", Some(Classification::Shadow))],
    );
    record(
        &mut store,
        &target,
        "2026-09-05T11:00:00+00:00",
        &[endpoint("bbb", "/api/admin/reset", Some(Classification::Known))],
    );

    let pending = store.pending_alerts(&target).expect("legge");
    assert_eq!(pending.len(), 1);
    // Ma il verdetto corrente è aggiornato: chi legge vede tutt'e due le cose.
    assert_eq!(pending[0].last_classification.as_deref(), Some("Known"));
}

/// Un `user_version` negativo mandava il cast in wraparound: il ciclo delle
/// migrazioni restava vuoto, `migrate` dichiarava lo schema corrente, e lo
/// storico si apriva **senza tabelle**. Il run se ne accorgeva solo a fine
/// analisi, con un messaggio che parlava di permessi.
#[test]
fn avversaria_una_versione_di_schema_negativa_viene_rifiutata_all_apertura() {
    let path = temp("versione-negativa");
    {
        let connection = rusqlite::Connection::open(&path).expect("si apre");
        connection
            .pragma_update(None, "user_version", -1_i64)
            .expect("scrive la versione");
    }
    let Err(errore) = HistoryStore::open(&path, "<path>/storia.db".to_string()) else {
        panic!("uno schema che non riconosciamo non si apre");
    };
    assert!(errore.to_string().contains("schema version -1"), "{errore}");
}

/// Due **primi** run concorrenti sullo stesso file nuovo: la migrazione non
/// deve partire due volte.
///
/// Con la transazione differita di prima il perdente rieseguiva la migrazione
/// su un database che aveva già le tabelle e moriva con `table runs already
/// exists`. Misurato: venti fallimenti su venti iterazioni con due processi.
#[test]
fn avversaria_due_prime_aperture_concorrenti_non_migrano_due_volte() {
    let path = temp("migrazione-concorrente");
    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            HistoryStore::open(&path, "<path>/x.db".to_string()).map(|_| ())
        }));
    }
    for handle in handles {
        let esito = handle.join().expect("nessun thread è caduto");
        assert!(esito.is_ok(), "una prima apertura concorrente è fallita: {esito:?}");
    }
}

/// Uno storico da **leggere** deve già esistere: crearlo vuoto significherebbe
/// rispondere «niente da segnalare» a un errore di battitura.
#[test]
fn avversaria_aprire_in_lettura_uno_storico_inesistente_non_lo_crea() {
    let path = temp("mai-esistito");
    let Err(errore) = HistoryStore::open_existing(&path, "<path>/storia.db".to_string()) else {
        panic!("non deve aprirsi");
    };
    assert!(errore.to_string().contains("there is no history at"), "{errore}");
    assert!(!path.exists(), "leggere non deve creare niente");
}

/// Un percorso che comincia per `file:` è un URI per SQLite, e aprirebbe un
/// database che vive solo in memoria: il tool direbbe di ricordare senza
/// ricordare niente.
#[test]
fn avversaria_un_uri_non_diventa_uno_storico_volatile() {
    let Err(errore) = HistoryStore::open(
        std::path::Path::new("file::memory:?cache=shared"),
        "<path>/x".to_string(),
    ) else {
        panic!("un URI non è un percorso");
    };
    assert!(errore.to_string().contains("SQLite reads as a URI"), "{errore}");
}

/// Un bersaglio che non esiste si sa dire.
#[test]
fn un_bersaglio_mai_visto_si_riconosce() {
    let (mut store, _path) = store("bersaglio-noto");
    let presente = Target::new("produzione").expect("nome valido");
    record(
        &mut store,
        &presente,
        "2026-09-05T10:00:00+00:00",
        &[endpoint("aaa", "/api/x", Some(Classification::Shadow))],
    );
    assert!(store.has_target(&presente).expect("controlla"));
    assert!(!store
        .has_target(&Target::new("produzine").expect("nome valido"))
        .expect("controlla"));
}
