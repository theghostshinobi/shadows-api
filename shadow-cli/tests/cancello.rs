//! I cancelli di cablaggio, verificati **eseguendo il binario**.
//!
//! - **Fase 1** (§9): un log nginx reale entra e produce, end-to-end, endpoint
//!   osservati e un `RunManifest` con conteggi e digest SHA-256.
//! - **Fase 2** (§9): sullo **stesso input** l'output è cambiato — non più
//!   richieste distinte ma endpoint logici, con `endpoints_found` diverso. È il
//!   modo in cui si dimostra che la Fase 2 si è innestata sulla pipeline
//!   esistente invece di affiancarle un modulo isolato.
//!
//! - **Fase 3** (§9): con una specifica dichiarata i finding sono etichettati
//!   per categoria e **puntano agli `EndpointPattern` della Fase 2**; senza
//!   specifica sono sospetti euristici.
//!
//! Questi test lanciano `shadow` come lo lancerebbe l'utente: se passano, la
//! pipeline è cablata dall'inizio alla fine, non a pezzi.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/nginx-combined")
        .join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shadow"))
        .args(args)
        .output()
        .expect("il binario shadow è compilato")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn cancello_un_log_nginx_reale_attraversa_tutta_la_pipeline() {
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let report = stdout(&output);

    // (a) l'elenco degli endpoint osservati
    assert!(report.contains("observed endpoints (3)"), "report:\n{report}");
    assert!(report.contains("/api/users/{id}"));
    assert!(report.contains("/api/admin/reset"));

    // (b) il manifest, con i conteggi
    assert!(report.contains("run manifest"));
    assert!(report.contains("lines total       5"));
    assert!(report.contains("lines parsed      5"));
    assert!(report.contains("lines discarded   0"));
    assert!(report.contains("endpoints found   3"));

    // (c) e il digest dell'input, con l'algoritmo accanto (§6)
    assert!(report.contains("sha256"));
    assert!(report.contains("34feec5197120a4a25cc5108071c721f29288734764109180f0e0390f5f48813"));
}

#[test]
fn cancello_fase_2_sullo_stesso_input_luscita_e_cambiata() {
    // Il cancello della Fase 2 è proprio questo: **lo stesso file** che in Fase
    // 1 produceva 5 richieste distinte ora produce 3 endpoint logici. Non è un
    // modulo nuovo accanto alla pipeline, è la stessa pipeline che è cresciuta.
    let log = fixture("valid.log");
    let report = stdout(&run(&[log.to_str().unwrap()]));

    // Le tre richieste su id diversi sono diventate un endpoint solo, con il
    // conteggio che le somma.
    assert!(report.contains("/api/users/{id}"), "report:\n{report}");
    assert!(!report.contains("/api/users/1"));
    assert!(!report.contains("/api/users/2"));
    assert!(!report.contains("/api/users/3"));

    // Il numero è cambiato: 5 richieste distinte, 3 endpoint.
    assert!(report.contains("observed endpoints (3)"));
    assert!(report.contains("lines parsed      5"));
}

#[test]
fn cancello_fase_2_ogni_endpoint_porta_il_proprio_identificatore_stabile() {
    let log = fixture("valid.log");
    let primo = stdout(&run(&[log.to_str().unwrap()]));
    let secondo = stdout(&run(&[log.to_str().unwrap()]));

    let id_di = |report: &str, pattern: &str| -> String {
        report
            .lines()
            .find(|l| l.contains(pattern))
            .and_then(|l| l.split_whitespace().next().map(str::to_string))
            .unwrap_or_else(|| panic!("pattern {pattern} assente da:\n{report}"))
    };

    let id = id_di(&primo, "/api/users/{id}");
    assert_eq!(id.len(), 16, "identificatore leggibile e incollabile");
    // §P4: lo stesso endpoint, lo stesso identificatore, in run diversi.
    assert_eq!(id, id_di(&secondo, "/api/users/{id}"));
}

#[test]
fn avversaria_un_endpoint_admin_resta_visibile_fra_gli_identificatori() {
    // La fixture avversaria obbligatoria di §9: se l'euristica fosse
    // aggressiva, `/api/users/admin` sparirebbe dentro `/api/users/{id}` e
    // nessuno lo guarderebbe più.
    let log = fixture("admin-hidden.log");
    let report = stdout(&run(&[log.to_str().unwrap()]));

    assert!(report.contains("/api/users/{id}"), "gli id devono collassare");
    assert!(
        report.contains("/api/users/admin"),
        "l'endpoint amministrativo deve restare visibile:\n{report}"
    );
    assert!(report.contains("/api/users/me"), "e anche le altre parole");
    assert!(report.contains("observed endpoints (3)"));
}

#[test]
fn il_report_va_su_stdout_e_la_diagnostica_su_stderr() {
    // §7: stdout è ciò che l'utente ha chiesto, stderr ciò che non ha chiesto.
    let log = fixture("corrupted-midfile.log");
    let output = run(&[log.to_str().unwrap()]);

    let report = stdout(&output);
    let diagnostics = stderr(&output);

    assert!(report.contains("observed endpoints"));
    assert!(!report.contains("warning:"), "stdout deve restare pulito");

    assert!(diagnostics.contains("reading"));
    assert!(diagnostics.contains("6 line(s) read, 3 parsed, 3 discarded"));
    assert!(diagnostics.contains("warning:"));
}

#[test]
fn le_righe_malformate_sono_contate_nel_manifest_e_non_sono_fatali() {
    let log = fixture("corrupted-midfile.log");
    let output = run(&[log.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(0));
    let report = stdout(&output);

    assert!(report.contains("lines discarded   3"));
    assert!(report.contains("malformed-line"));
    assert!(report.contains("invalid-status-code"));
    assert!(report.contains("invalid-timestamp"));
}

#[test]
fn un_formato_non_riconosciuto_esce_con_1_e_non_stampa_nessun_report() {
    // §P2: o il formato è riconosciuto, o ci si rifiuta. E §7: senza analisi
    // non c'è manifest, quindi non c'è verdetto — infatti stdout è vuoto.
    let log = fixture("unrecognised-format.log");
    let output = run(&[log.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun verdetto senza analisi");
    assert!(stderr(&output).contains("does not look like the 'nginx-combined' log format"));
}

#[test]
fn un_file_illeggibile_esce_con_1() {
    let output = run(&["/questo/percorso/non/esiste.log"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("cannot read"));
}

#[test]
fn un_formato_dichiarato_e_inesistente_esce_con_1_e_dice_quali_esistono() {
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap(), "--log-format", "apache-common"]);

    assert_eq!(output.status.code(), Some(1));
    let diagnostics = stderr(&output);
    assert!(diagnostics.contains("unknown log format 'apache-common'"));
    assert!(diagnostics.contains("nginx-combined"));
}

#[test]
fn un_argomento_non_valido_e_un_errore_duso_codice_2() {
    let output = run(&["--questo-flag-non-esiste"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn due_run_sullo_stesso_input_danno_lo_stesso_report_a_parte_il_timestamp() {
    // §P4: il timestamp del run è l'unico valore che può differire.
    let log = fixture("valid.log");
    let primo = stdout(&run(&[log.to_str().unwrap()]));
    let secondo = stdout(&run(&[log.to_str().unwrap()]));

    let senza_timestamp = |report: &str| -> String {
        report
            .lines()
            .filter(|l| !l.trim_start().starts_with("run timestamp"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_eq!(senza_timestamp(&primo), senza_timestamp(&secondo));
    assert!(primo.contains("run timestamp"));
}

#[test]
fn senza_argomenti_laiuto_esce_su_stderr_e_stdout_resta_vuoto() {
    // L'aiuto stampato di iniziativa del tool non è ciò che l'utente ha chiesto
    // (§7), e nessuna analisi è avvenuta: nessun manifest, nessun verdetto.
    let output = run(&[]);

    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("Usage:"));
}

#[test]
fn laiuto_richiesto_esce_su_stdout() {
    // L'altra metà del contratto: `shadow --help | less` deve funzionare.
    let output = run(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("Usage:"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn un_file_vuoto_e_unanalisi_legittima_ma_viene_detto() {
    // Non è un errore: è un'analisi che non ha trovato niente. Ma tacerlo
    // darebbe un falso senso di sicurezza (§P1), quindi si avvisa.
    let path = std::env::temp_dir().join("shadow-fase1-vuoto.log");
    std::fs::write(&path, b"").expect("file temporaneo");

    let output = run(&[path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(0));
    assert!(stderr(&output).contains("the input file is empty"));
    // L'analisi è avvenuta, quindi il manifest c'è: il verdetto è legittimo (§7).
    assert!(stdout(&output).contains("lines total       0"));
    assert!(stdout(&output).contains("run manifest"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn avversaria_encoding_nei_path_end_to_end() {
    // §9, Fase 2: normalizzare in modo consistente perché la stessa cosa non
    // diventi due endpoint, **senza perdere il segnale**.
    let log = fixture("encoded-paths.log");
    let report = stdout(&run(&[log.to_str().unwrap()]));

    // `/api/../secret` e `/api/%2e%2e/secret` sono la stessa cosa: un endpoint,
    // due osservazioni.
    assert!(report.contains("/api/../secret"), "report:\n{report}");
    assert!(!report.contains("%2e%2e/secret"));

    // La doppia codifica resta distinta: è un segnale, non un doppione.
    assert!(report.contains("/api/%252e%252e/secret"));

    // `/api/a%2fb` è un segmento solo, `/api/a/b` sono due: non vanno fusi.
    assert!(report.contains("/api/a%2Fb"));
    assert!(report.contains("/api/a/b"));
    assert!(report.contains("observed endpoints (4)"));
}

// --- Fase 3 -----------------------------------------------------------------

fn spec(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/openapi")
        .join(name)
}

#[test]
fn cancello_fase_3_i_finding_puntano_agli_endpoint_della_fase_2() {
    // Il cancello: pipeline completa log → inventario → classificazione, con i
    // finding che riferiscono gli `EndpointPattern` prodotti dalla Fase 2.
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
    ]);
    let report = stdout(&output);

    // L'identificatore che compare nella riga dell'endpoint è lo stesso che
    // compare nel finding: sono la stessa cosa, non due elenchi paralleli.
    let id = report
        .lines()
        .find(|l| l.contains("/api/users/{id}") && l.contains("auth:"))
        .and_then(|l| l.split_whitespace().next())
        .expect("l'endpoint è nell'inventario");
    assert!(
        report.contains(&format!("Known         high     severity low    {id}")),
        "report:\n{report}"
    );

    assert!(report.contains("Shadow"));
    assert!(report.contains("because not present in the declared inventory"));
    assert!(report.contains("findings ("));
    // I conteggi per categoria finiscono nel manifest (§6).
    assert!(report.contains("Shadow"));
    assert!(report.contains("Known"));
}

#[test]
fn avversaria_un_openapi_stantio_non_produce_un_known_sbagliato() {
    // `/api/users/admin` osservato contro `/api/users/{userId}` dichiarato: un
    // confronto permissivo lo direbbe `Known` e il team smetterebbe di
    // guardarlo.
    let log = fixture("admin-hidden.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("stale.json").to_str().unwrap(),
    ]);
    let report = stdout(&output);

    // La riga del **finding**, non quella dell'inventario: si riconosce perché
    // porta una classificazione.
    let admin_finding = report
        .lines()
        .find(|l| l.contains("/api/users/admin") && l.trim_start().starts_with(|c: char| c.is_ascii_uppercase()))
        .expect("l'endpoint amministrativo ha un finding");
    assert!(
        admin_finding.contains("Undetermined"),
        "classificato come: {admin_finding}"
    );
    assert!(!admin_finding.contains("Known"));
    assert!(report.contains("would hide an endpoint the spec does not describe"));
}

#[test]
fn un_endpoint_dichiarato_e_non_visto_su_una_finestra_lunga_e_zombie_ed_esce_3() {
    let log = fixture("long-window.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
    ]);

    let report = stdout(&output);
    assert!(report.contains("Zombie"));
    assert!(report.contains("more than the staleness window"));
    // §7: un finding conclamato fa fallire di proposito una pipeline CI.
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn senza_specifica_un_log_tranquillo_non_produce_rumore_ed_esce_0() {
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap()]);

    assert!(stdout(&output).contains("findings (0)"));
    assert!(stderr(&output).contains("findings will be suspicions, not verdicts"));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn senza_specifica_i_soli_sospetti_non_fanno_fallire_la_pipeline() {
    // §7: il `3` scatta solo su conclamati. Un run senza dichiarato non ha basi
    // per bocciare una build.
    let log = fixture("suspicious-paths.log");
    let output = run(&[log.to_str().unwrap()]);
    let report = stdout(&output);

    assert!(report.contains("Undetermined"));
    assert!(!report.contains("Shadow"), "senza specifica non si dice mai Shadow");
    assert!(report.contains("suggests a non-production surface"));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn fail_on_undetermined_e_opt_in_e_cambia_solo_il_codice_di_uscita() {
    let log = fixture("suspicious-paths.log");

    let normale = run(&[log.to_str().unwrap()]);
    let con_flag = run(&[log.to_str().unwrap(), "--fail-on-undetermined"]);

    assert_eq!(normale.status.code(), Some(0));
    assert_eq!(con_flag.status.code(), Some(3));

    // I finding sono gli stessi: il flag decide se fallire, non cosa segnalare.
    let solo_finding = |r: String| -> String {
        r.lines()
            .skip_while(|l| !l.starts_with("findings ("))
            .take_while(|l| !l.starts_with("run manifest"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(solo_finding(stdout(&normale)), solo_finding(stdout(&con_flag)));

    // Ma il manifest **deve** distinguerli: è la configurazione effettiva del
    // run, e questa è cambiata (§6, v1.6).
    assert!(stdout(&normale).contains("fail_on_undetermined   false                    from default"));
    assert!(stdout(&con_flag).contains("fail_on_undetermined   true                     from command-line"));
}

#[test]
fn una_specifica_illeggibile_ferma_il_run_senza_produrre_verdetti() {
    // §P2: classificare contro mezza specifica produrrebbe `Shadow` inventati.
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        "/questa/specifica/non/esiste.json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun manifest, nessun verdetto");
    assert!(stderr(&output).contains("cannot read the declared spec"));
}

#[test]
fn il_manifest_registra_anche_la_specifica_usata() {
    let log = fixture("valid.log");
    let report = stdout(&run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
    ]));

    assert!(report.contains("current.json"));
    // Due input, due digest: il log e la specifica (§6).
    assert_eq!(report.matches("sha256").count(), 2);
}

// --- Fase 4 -----------------------------------------------------------------

fn allowlist(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/allowlist")
        .join(name)
}

#[test]
fn cancello_fase_4_un_log_con_segreti_non_produce_un_report_che_perde_segreti() {
    // Il cancello: eseguire su un log reale con parametri sensibili produce un
    // report azionabile **con i segreti mascherati** (§9, Fase 4).
    let log = fixture("secrets.log");
    let output = run(&[log.to_str().unwrap()]);
    let report = stdout(&output);

    // Nessuno dei segreti della fixture compare, in nessuna forma.
    assert!(!report.contains("eyJhbGciOiJIUzI1NiJ9"), "JWT in chiaro:\n{report}");
    assert!(!report.contains("mario.rossi@bancaxyz.it"), "email in chiaro");
    assert!(!report.contains("bancaxyz"), "dominio del cliente in chiaro");
    assert!(!report.contains("hunter2"), "password in chiaro");
    assert!(!report.contains("AKIAIOSFODNN7EXAMPLE"), "chiave in chiaro");

    // Ma il report resta azionabile: si vede che quegli endpoint esistono.
    assert!(report.contains("/api/reset/<redacted>"));
    assert!(report.contains("/api/users/<redacted>"));
    assert!(report.contains("/api/health"));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn i_percorsi_di_filesystem_sono_mascherati_nel_report_e_nel_manifest() {
    // §P5 esteso: un percorso rivela cliente e struttura interna, e il report
    // con il suo manifest è il file che si consegna all'auditor. Qui **non**
    // esistono eccezioni.
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap()]);

    assert!(stdout(&output).contains("<path>/valid.log"));
    assert!(!stdout(&output).contains("tests/fixtures"));
}

#[test]
fn un_percorso_digitato_dallutente_resta_leggibile_nei_messaggi() {
    // L'eccezione mirata di §P5 (v1.7): mostrargli il percorso che ha appena
    // scritto non gli rivela niente, e un errore che lo maschera non aiuta chi
    // ha sbagliato a scriverlo.
    let output = run(&["/questo/log/non/esiste.log"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("/questo/log/non/esiste.log"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn un_percorso_arrivato_dallambiente_resta_mascherato_nei_messaggi() {
    // L'eccezione vale solo per ciò che l'utente ha digitato **in questa
    // invocazione**: una variabile d'ambiente può essere stata scritta da
    // qualcun altro, in un file di CI che finisce in un log condiviso.
    let log = fixture("valid.log");
    let output = Command::new(env!("CARGO_BIN_EXE_shadow"))
        .arg(log.to_str().unwrap())
        .env("SHADOW_OPENAPI_SPEC", spec("current.json"))
        .output()
        .expect("il binario shadow è compilato");

    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("<path>/current.json"), "stderr: {diagnostics}");
    assert!(!diagnostics.contains("tests/fixtures/openapi"));
}

#[test]
fn show_raw_values_e_opt_in_e_lo_dice_a_chi_legge() {
    let log = fixture("secrets.log");
    let output = run(&[log.to_str().unwrap(), "--show-raw-values"]);

    assert!(stdout(&output).contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(stdout(&output).contains("redaction         OFF"));
    // Chi lancia il comando deve saperlo anche se guarda solo lo stderr.
    assert!(stderr(&output).contains("redaction is OFF"));
}

#[test]
fn il_manifest_registra_la_configurazione_effettiva_con_la_provenienza() {
    // §6, v1.6.
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--zombie-staleness-days",
        "90",
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
    ]);
    let report = stdout(&output);

    assert!(report.contains("configuration"));
    assert!(
        report.contains("zombie_staleness_days  90"),
        "report:\n{report}"
    );
    assert!(report.contains("from command-line"));
    assert!(report.contains("log_format             nginx-combined"));
    assert!(report.contains("from default"));
}

#[test]
fn lallowlist_silenzia_il_report_ma_non_i_conteggi_del_manifest() {
    // La regola che rende l'allowlist accettabile (§5, Fase 4).
    let log = fixture("valid.log");
    let senza = stdout(&run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
    ]));
    let con = stdout(&run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
        "--allowlist",
        allowlist("known-noise.allow").to_str().unwrap(),
    ]));

    assert!(senza.contains("findings (5)"));
    assert!(con.contains("findings (0)"));
    assert!(con.contains("silenced by the allowlist"));
    // I conteggi per categoria restano identici: l'evidenza non è stata
    // cancellata, solo tolta dalla vista.
    for categoria in ["Shadow          1", "Known           1", "Undetermined    3"] {
        assert!(senza.contains(categoria), "manca in senza: {categoria}");
        assert!(con.contains(categoria), "manca in con: {categoria}");
    }
}

#[test]
fn una_regola_di_allowlist_troppo_ampia_viene_segnalata() {
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("current.json").to_str().unwrap(),
        "--allowlist",
        allowlist("known-noise.allow").to_str().unwrap(),
    ]);

    let diagnostics = stderr(&output);
    assert!(diagnostics.contains("is broad"));
    assert!(diagnostics.contains("pattern /api/*"));
    assert!(diagnostics.contains("fixed segment"));
}

#[test]
fn lesport_json_e_lunico_contenuto_di_stdout() {
    // §7: `shadow ... > report.json` deve dare un file pulito.
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap(), "--output-format", "json"]);

    let document: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout è JSON valido e nient'altro");
    assert_eq!(document["schema"], "shadow-report/1");
    assert_eq!(document["redaction_enabled"], true);
    assert!(document["manifest"]["configuration"]["log_format"]["source"] == "default");
    // La diagnostica è finita dove doveva.
    assert!(stderr(&output).contains("line(s) read"));
}

#[test]
fn lesport_markdown_e_un_documento_leggibile() {
    let log = fixture("secrets.log");
    let report = stdout(&run(&[log.to_str().unwrap(), "--output-format", "markdown"]));

    assert!(report.starts_with("# Shadow report"));
    assert!(report.contains("## Observed endpoints"));
    assert!(report.contains("### Configuration"));
    // La redazione vale in ogni formato: è il Markdown che finisce nei ticket.
    assert!(!report.contains("bancaxyz"));
    assert!(!report.contains("AKIAIOSFODNN7EXAMPLE"));
}

#[test]
fn due_run_sullo_stesso_input_danno_lo_stesso_json_a_parte_il_timestamp() {
    // §P4, verificato sul formato che finisce in un archivio d'audit.
    let log = fixture("valid.log");
    let normalizza = |raw: String| -> serde_json::Value {
        let mut document: serde_json::Value = serde_json::from_str(&raw).expect("JSON valido");
        document["manifest"]["run_timestamp"] = serde_json::Value::Null;
        document
    };

    let primo = normalizza(stdout(&run(&[log.to_str().unwrap(), "--output-format", "json"])));
    let secondo = normalizza(stdout(&run(&[log.to_str().unwrap(), "--output-format", "json"])));

    assert_eq!(primo, secondo);
}

#[test]
fn una_bomba_di_espansione_yaml_ferma_il_run_senza_produrre_verdetti() {
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec("bomb.yaml").to_str().unwrap(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun manifest, nessun verdetto");
    assert!(stderr(&output).contains("expansion bombs"));
}

// --- il cablaggio della grammatica dichiarata -------------------------------

fn variante(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/log-format-variants")
        .join(name)
}

/// Il `combined`, nella sintassi con cui nginx lo dichiara.
const COMBINED: &str = "$remote_addr - $remote_user [$time_local] \
                        \"$request\" $status $body_bytes_sent \"$http_referer\" \"$http_user_agent\"";

/// Il cancello della taratura: una variante reale di `log_format` che **prima**
/// faceva rifiutare l'intero file ora attraversa la pipeline fino al report,
/// passando dalla riga di comando come la userebbe l'utente (§P8).
#[test]
fn cancello_un_log_con_request_time_attraversa_la_pipeline_se_dichiarato() {
    let log = variante("03-combined-con-request-time.log");
    let dichiarazione = format!("{COMBINED} $request_time");

    // Senza dichiarazione: rifiutato, come prima e come deve restare.
    let rifiutato = run(&[log.to_str().unwrap()]);
    assert_eq!(rifiutato.status.code(), Some(1));
    assert!(stdout(&rifiutato).is_empty(), "stdout deve restare vuoto");

    // Con la dichiarazione: analisi completa.
    let output = run(&[log.to_str().unwrap(), "--log-format", &dichiarazione]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let report = stdout(&output);
    assert!(report.contains("/api/users/{id}"), "report:\n{report}");
    assert!(report.contains("lines parsed      3"), "report:\n{report}");
}

/// La dichiarazione è una configurazione che **cambia l'esito**, quindi il
/// manifest deve registrarla con la sua provenienza (§6, v1.6). Senza, due
/// report divergenti sullo stesso servizio non direbbero se è cambiato il
/// traffico o il modo di leggerlo.
#[test]
fn il_manifest_registra_la_grammatica_dichiarata_e_da_dove_arriva() {
    let log = variante("06-apache-common.log");
    let dichiarazione = "$remote_addr - $remote_user [$time_local] \"$request\" $status $body_bytes_sent";
    let output = run(&[
        log.to_str().unwrap(),
        "--log-format",
        dichiarazione,
        "--output-format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let report = stdout(&output);
    assert!(report.contains("\"log_format\""), "report:\n{report}");
    assert!(report.contains("$remote_addr"), "report:\n{report}");
    assert!(report.contains("\"source\": \"command-line\""), "report:\n{report}");
}

/// Una dichiarazione inutilizzabile si ferma **prima** di leggere il file, con
/// il codice di errore d'esecuzione e stdout vuoto: nessun manifest, nessun
/// verdetto (§7).
#[test]
fn una_dichiarazione_ambigua_ferma_il_run_prima_di_toccare_il_log() {
    let log = fixture("valid.log");
    let output = run(&[
        log.to_str().unwrap(),
        "--log-format",
        "$remote_addr$status [$time_local] \"$request\"",
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun verdetto senza manifest");
    assert!(
        stderr(&output).contains("no way to tell where one ends"),
        "stderr: {}",
        stderr(&output)
    );
}

/// A input costante il verdetto non cambia, nemmeno passando dalla grammatica
/// dichiarata invece che da quella preimpostata (§P4).
#[test]
fn dichiarare_il_combined_da_lo_stesso_report_del_preimpostato() {
    let log = fixture("valid.log");
    let preimpostato = run(&[log.to_str().unwrap(), "--output-format", "json"]);
    let dichiarato = run(&[
        log.to_str().unwrap(),
        "--output-format",
        "json",
        "--log-format",
        COMBINED,
    ]);

    // Le sole due cose che devono differire: il timestamp del run, che §P4
    // esclude già, e la voce `log_format` del manifest — valore **e**
    // provenienza — perché è esattamente ciò che stiamo cambiando. Tutto il
    // resto, endpoint e conteggi e digest compresi, deve coincidere carattere
    // per carattere.
    let normalizza = |report: String| {
        let mut out = Vec::new();
        let mut dentro_log_format = false;
        for line in report.lines() {
            if line.contains("\"log_format\": {") {
                dentro_log_format = true;
                continue;
            }
            if dentro_log_format {
                if line.trim_start().starts_with('}') {
                    dentro_log_format = false;
                }
                continue;
            }
            if line.contains("\"run_timestamp\"") {
                continue;
            }
            out.push(line);
        }
        out.join("\n")
    };
    assert_eq!(
        normalizza(stdout(&preimpostato)),
        normalizza(stdout(&dichiarato))
    );
}

/// Una barra verticale che arriva **dai dati analizzati** non deve spezzare una
/// riga della tabella Markdown.
///
/// Trovato da una revisione avversaria: la prima stesura proteggeva solo la
/// tabella della configurazione, e lasciava scoperte quella degli endpoint e
/// quella dei finding — cioè proprio le due che portano testo deciso da chi ha
/// scritto le richieste, non da noi. Chi legge non vede che manca un pezzo:
/// vede una tabella storta e si fida lo stesso.
#[test]
fn una_barra_verticale_nel_path_non_spezza_la_tabella_markdown() {
    let log = std::env::temp_dir().join("shadow-taratura-barra.log");
    std::fs::write(
        &log,
        "10.0.0.1 - - [10/Oct/2023:13:55:31 +0000] \"GET /api/lo|gs HTTP/1.1\" 200 5 \"-\" \"UA\"\n",
    )
    .expect("la fixture si scrive");

    let output = run(&[log.to_str().unwrap(), "--output-format", "markdown"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let report = stdout(&output);
    let riga = report
        .lines()
        .find(|l| l.contains("lo") && l.contains("gs") && l.starts_with('|'))
        .expect("la riga dell'endpoint esiste");

    // La tabella degli endpoint ha quattro colonne. Le barre protette non
    // separano niente, quindi si tolgono prima di contare: se la barra dei dati
    // non fosse protetta, questa riga ne conterebbe cinque.
    assert!(riga.contains("\\|"), "la barra dev'essere protetta: {riga}");
    let colonne = riga.replace("\\|", "").split('|').count() - 2;
    assert_eq!(colonne, 4, "riga: {riga}");

    let _ = std::fs::remove_file(&log);
}

// --- i due difetti pre-esistenti, chiusi su richiesta del fondatore ---------

fn run_con_ambiente(args: &[&str], chiave: &str, valore: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shadow"))
        .args(args)
        .env(chiave, valore)
        .output()
        .expect("il binario shadow è compilato")
}

/// Il gate acceso dall'ambiente si accende davvero.
#[test]
fn una_chiave_booleana_dall_ambiente_accende_il_gate_come_il_flag() {
    let log = fixture("suspicious-paths.log");
    let acceso = run_con_ambiente(
        &[log.to_str().unwrap()],
        "SHADOW_FAIL_ON_UNDETERMINED",
        "true",
    );
    assert_eq!(acceso.status.code(), Some(3), "stderr: {}", stderr(&acceso));
    assert!(stdout(&acceso).contains("fail_on_undetermined   true                     from environment"));

    let spento = run_con_ambiente(
        &[log.to_str().unwrap()],
        "SHADOW_FAIL_ON_UNDETERMINED",
        "false",
    );
    assert_eq!(spento.status.code(), Some(0));
}

/// **Il difetto.** `yes` non è fra i valori canonici, e prima significava
/// `false` in silenzio: chi credeva di aver acceso una condizione di fallimento
/// in CI otteneva il contrario, e il manifest lo registrava come `false` senza
/// un avviso. Ora il run si ferma (§P2).
#[test]
fn avversaria_un_valore_booleano_non_riconosciuto_ferma_il_run_invece_di_valere_false() {
    let log = fixture("suspicious-paths.log");
    for valore in ["yes", "on", "TRUE", "True", "1 ", ""] {
        let output = run_con_ambiente(
            &[log.to_str().unwrap()],
            "SHADOW_FAIL_ON_UNDETERMINED",
            valore,
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "'{valore}' doveva fermare il run, stderr: {}",
            stderr(&output)
        );
        // Nessun manifest, nessun verdetto (§7): il run non ha analizzato nulla.
        assert!(stdout(&output).is_empty(), "'{valore}': stdout non è vuoto");
        assert!(
            stderr(&output).contains("SHADOW_FAIL_ON_UNDETERMINED must be one of"),
            "'{valore}': il messaggio non nomina la chiave"
        );
    }
}

/// Vale per **ogni** chiave booleana, non solo per il gate: la deroga a §P5 è
/// la più pericolosa da accendere per sbaglio, e la più pericolosa da credere
/// accesa quando non lo è.
#[test]
fn anche_la_deroga_alla_redazione_rifiuta_un_valore_non_riconosciuto() {
    let log = fixture("secrets.log");
    let output = run_con_ambiente(&[log.to_str().unwrap()], "SHADOW_SHOW_RAW_VALUES", "yes");
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("SHADOW_SHOW_RAW_VALUES must be one of"));
}

// --- Fase 5: il cancello dello storico persistente --------------------------

fn storico(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("shadow-fase5-cli-{name}.db"));
    let _ = std::fs::remove_file(&path);
    path
}

/// **Il cancello di cablaggio della Fase 5.** Due esecuzioni sullo stesso input:
/// la prima non ha mai visto niente, la seconda ha già visto tutto. Poi
/// `shadow alerts` mostra gli shadow non presi in carico.
///
/// Che sia cablato e non affiancato lo dimostra una cosa sola: gli endpoint che
/// lo storico ricorda sono gli `EndpointPattern` della **Fase 2**, con il loro
/// identificatore stabile, e i verdetti sono quelli della **Fase 3**. La Fase 5
/// non ha aggiunto analisi: ha aggiunto memoria.
#[test]
fn cancello_lo_storico_distingue_il_nuovo_dal_gia_visto() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("cancello");

    let primo = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);
    assert_eq!(primo.status.code(), Some(3), "stderr: {}", stderr(&primo));
    let diagnostica = stderr(&primo);
    assert!(
        diagnostica.contains("endpoint(s) never seen before"),
        "il primo run deve dire cosa non aveva mai visto:\n{diagnostica}"
    );
    // L'identificatore è quello stabile della Fase 2: lo stesso che compare nel
    // report.
    assert!(diagnostica.contains("/api/admin/reset"), "{diagnostica}");

    let secondo = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);
    assert!(
        !stderr(&secondo).contains("never seen before"),
        "il secondo run sullo stesso input non ha scoperto niente:\n{}",
        stderr(&secondo)
    );

    // E l'alert è una riga che si interroga, non un messaggio che parte (§P6).
    let alerts = run(&["alerts", "--history", db.to_str().unwrap()]);
    assert_eq!(alerts.status.code(), Some(0));
    let elenco = stdout(&alerts);
    assert!(elenco.contains("unacknowledged shadow endpoint"), "{elenco}");
    assert!(elenco.contains("/api/admin/reset"), "{elenco}");

    let _ = std::fs::remove_file(&db);
}

/// §P4 con il tempo di mezzo: **lo storico non tocca il verdetto.** A input
/// costante il report è identico con e senza `--history`, salvo la voce di
/// configurazione che è cambiata davvero.
#[test]
fn lo_storico_non_cambia_di_una_virgola_il_report() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("determinismo");

    let senza = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--output-format",
        "json",
    ]);
    let con = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--output-format",
        "json",
        "--history",
        db.to_str().unwrap(),
    ]);

    // Si tolgono il timestamp del run e le due chiavi di configurazione che
    // *devono* differire, perché sono ciò che stiamo cambiando.
    let normalizza = |report: String| {
        let mut out = Vec::new();
        let mut dentro = false;
        for line in report.lines() {
            if line.contains("\"history_path\": {") || line.contains("\"target\": {") {
                dentro = true;
                continue;
            }
            if dentro {
                if line.trim_start().starts_with('}') {
                    dentro = false;
                }
                continue;
            }
            if line.contains("\"run_timestamp\"") {
                continue;
            }
            out.push(line);
        }
        out.join("\n")
    };
    assert_eq!(normalizza(stdout(&senza)), normalizza(stdout(&con)));

    let _ = std::fs::remove_file(&db);
}

/// Uno storico inutilizzabile ferma il run **prima** che il report esca: nessun
/// manifest, nessun verdetto (§7). Stampare il report e poi fallire lascerebbe
/// credere che la memoria si sia aggiornata, e al run dopo tutto risulterebbe
/// nuovo.
#[test]
fn avversaria_uno_storico_illeggibile_ferma_il_run_senza_stampare_niente() {
    let log = fixture("valid.log");
    let db = storico("corrotto");
    std::fs::write(&db, b"non sono un database\n").expect("la fixture si scrive");

    let output = run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun verdetto senza manifest");
    assert!(
        stderr(&output).contains("not a readable Shadow history"),
        "stderr: {}",
        stderr(&output)
    );

    let _ = std::fs::remove_file(&db);
}

/// I bersagli non si mescolano: è ciò che «multi-tenancy» significa qui.
#[test]
fn due_bersagli_nello_stesso_file_non_si_vedono_fra_loro() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("bersagli");

    for target in ["fatturazione", "anagrafica"] {
        let output = run(&[
            log.to_str().unwrap(),
            "--openapi-spec",
            spec.to_str().unwrap(),
            "--history",
            db.to_str().unwrap(),
            "--target",
            target,
        ]);
        // Ogni bersaglio scopre le proprie novità: il secondo non eredita la
        // memoria del primo.
        assert!(
            stderr(&output).contains("never seen before on target"),
            "target {target}: {}",
            stderr(&output)
        );
    }

    let uno = run(&["alerts", "--history", db.to_str().unwrap(), "--target", "fatturazione"]);
    assert!(stdout(&uno).contains("target 'fatturazione'"), "{}", stdout(&uno));

    let _ = std::fs::remove_file(&db);
}

/// `shadow alerts` non analizza niente: non produce un manifest e non afferma
/// «nessun finding» (§7). Esce con successo nello stesso senso di `--version`.
#[test]
fn alerts_non_produce_un_manifest_perche_non_analizza() {
    let log = fixture("valid.log");
    let db = storico("nessun-manifest");
    let creato = run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);
    assert_eq!(creato.status.code(), Some(0), "stderr: {}", stderr(&creato));

    let output = run(&["alerts", "--history", db.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let elenco = stdout(&output);
    assert!(elenco.contains("no unacknowledged shadow endpoints"), "{elenco}");
    assert!(!elenco.contains("run manifest"), "{elenco}");
    assert!(!elenco.contains("findings ("), "{elenco}");

    let _ = std::fs::remove_file(&db);
}

// --- i difetti che la revisione avversaria ha trovato in questa fetta -------

/// **Il difetto grave.** Un run senza `--openapi-spec` — la modalità
/// preimpostata — azzerava in silenzio la coda degli alert prodotta da un run
/// che la specifica ce l'aveva. Nessuno li aveva presi in carico: erano
/// spariti.
#[test]
fn avversaria_un_run_senza_specifica_non_cancella_gli_alert_di_uno_con_specifica() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("alert-cancellati");

    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);
    let prima = stdout(&run(&["alerts", "--history", db.to_str().unwrap()]));
    assert!(prima.contains("2 unacknowledged"), "{prima}");

    // Lo stesso log, senza specifica: non ha niente da dire su quegli endpoint.
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    let dopo = stdout(&run(&["alerts", "--history", db.to_str().unwrap()]));
    assert!(
        dopo.contains("2 unacknowledged"),
        "un run che non dice niente non deve cancellare un alert:\n{dopo}"
    );

    let _ = std::fs::remove_file(&db);
}

/// Un percorso sbagliato non deve diventare un «tutto a posto». Prima
/// `alerts` creava il file e rispondeva «nessun endpoint shadow», uscita 0.
#[test]
fn avversaria_alerts_su_uno_storico_inesistente_non_dice_tutto_a_posto() {
    let db = storico("mai-esistito");
    let output = run(&["alerts", "--history", db.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("there is no history at"),
        "stderr: {}",
        stderr(&output)
    );
    assert!(!db.exists(), "leggere non deve creare niente");
}

/// Un bersaglio che non c'è non è un bersaglio tranquillo: prima una lettera
/// sbagliata in `--target` produceva «nessun alert» con successo.
#[test]
fn avversaria_un_bersaglio_inesistente_lo_dice_invece_di_tacere() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("bersaglio-sbagliato");
    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
        "--target",
        "produzione",
    ]);

    let output = run(&[
        "alerts",
        "--history",
        db.to_str().unwrap(),
        "--target",
        "produzine",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let detto = stdout(&output);
    assert!(detto.contains("no target named 'produzine'"), "{detto}");
    // E dice quali ci sono, invece di lasciare indovinare.
    assert!(detto.contains("produzione"), "{detto}");

    let _ = std::fs::remove_file(&db);
}

/// Prendere in carico un identificatore che non esiste non è un successo.
#[test]
fn avversaria_un_acknowledge_a_vuoto_non_esce_con_successo() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("ack-a-vuoto");
    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);

    let output = run(&[
        "alerts",
        "--history",
        db.to_str().unwrap(),
        "--acknowledge",
        "000000deadbeef00",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("no endpoint with id"), "{}", stdout(&output));

    let _ = std::fs::remove_file(&db);
}

/// §P5: il percorso dello storico non deve comparire in chiaro nel messaggio
/// d'errore quando arriva dall'ambiente. Prima ci compariva **due volte**: una
/// mascherata e una grezza, dentro il testo di SQLite.
#[test]
fn avversaria_un_percorso_di_storico_dall_ambiente_non_esce_in_chiaro() {
    let log = fixture("valid.log");
    let inesistente = std::env::temp_dir().join("shadow-clienti-bancaXYZ/storico.db");
    let output = run_con_ambiente(
        &[log.to_str().unwrap()],
        "SHADOW_HISTORY_PATH",
        inesistente.to_str().unwrap(),
    );

    assert_eq!(output.status.code(), Some(1));
    let messaggio = stderr(&output);
    assert!(
        !messaggio.contains("shadow-clienti-bancaXYZ"),
        "la struttura di directory non deve uscire:\n{messaggio}"
    );
    assert!(messaggio.contains("<path>/storico.db"), "{messaggio}");
}

/// §P5 vale anche in `alerts`: un percorso arrivato dall'ambiente non si mostra
/// in chiaro, nemmeno quando lo storico non esiste.
#[test]
fn anche_in_alerts_un_percorso_dall_ambiente_resta_mascherato() {
    let inesistente = std::env::temp_dir().join("shadow-clienti-riservati/storico.db");
    let output = run_con_ambiente(
        &["alerts"],
        "SHADOW_HISTORY_PATH",
        inesistente.to_str().unwrap(),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !stderr(&output).contains("shadow-clienti-riservati"),
        "stderr: {}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("<path>/storico.db"), "{}", stderr(&output));
}

/// La precedenza di §7 vale anche per `shadow alerts`: prima il sottocomando
/// leggeva direttamente da clap e ignorava l'ambiente.
#[test]
fn anche_alerts_rispetta_la_precedenza_flag_ambiente_default() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("precedenza");
    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
        "--target",
        "produzione",
    ]);

    // Il bersaglio arriva dall'ambiente, non dal flag.
    let output = run_con_ambiente(
        &["alerts", "--history", db.to_str().unwrap()],
        "SHADOW_TARGET",
        "produzione",
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("target 'produzione'"), "{}", stdout(&output));

    let _ = std::fs::remove_file(&db);
}

/// `history_path` è un percorso, non un piccolo linguaggio: uno storico «in
/// memoria» direbbe di ricordare senza ricordare niente, e ogni run troverebbe
/// tutto nuovo.
///
/// Togliere il flag `SQLITE_OPEN_URI` non basta: SQLite compilato con
/// `SQLITE_USE_URI` interpreta comunque un nome che comincia per `file:`.
#[test]
fn avversaria_un_uri_sqlite_non_viene_interpretato_come_storico_in_memoria() {
    let log = fixture("valid.log");
    for uri in ["file::memory:?cache=shared", "file:/tmp/x.db?mode=memory"] {
        let output = run(&[log.to_str().unwrap(), "--history", uri]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "'{uri}' doveva essere rifiutato, stderr: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "nessun verdetto senza manifest");
        assert!(
            stderr(&output).contains("SQLite reads as a URI"),
            "stderr: {}",
            stderr(&output)
        );
    }
}

/// Senza log e senza sottocomando è un **errore d'uso**: `2` (§7), non `1`.
#[test]
fn senza_log_e_senza_sottocomando_e_un_errore_d_uso() {
    let output = run(&["--output-format", "json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("a log file is required"), "{}", stderr(&output));
}

// --- le tre conversazioni aperte dal passo umano del collaudo ---------------

fn scrivi_log(nome: &str, paths: &[&str]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("shadow-collaudo-{nome}.log"));
    let mut contenuto = String::new();
    for (i, p) in paths.iter().enumerate() {
        contenuto.push_str(&format!(
            "10.0.0.1 - - [10/Oct/2023:13:55:{:02} +0000] \"GET {p} HTTP/1.1\" 200 512 \"-\" \"UA\"\n",
            i % 60
        ));
    }
    std::fs::write(&path, contenuto).expect("la fixture si scrive");
    path
}

fn scrivi_spec(nome: &str, paths: &[&str]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("shadow-collaudo-{nome}.json"));
    let voci: Vec<String> = paths
        .iter()
        .map(|p| format!("\"{p}\":{{\"get\":{{\"responses\":{{\"200\":{{\"description\":\"ok\"}}}}}}}}"))
        .collect();
    std::fs::write(
        &path,
        format!(
            "{{\"openapi\":\"3.0.0\",\"info\":{{\"title\":\"t\",\"version\":\"1\"}},\"paths\":{{{}}}}}",
            voci.join(",")
        ),
    )
    .expect("la fixture si scrive");
    path
}

/// **Sette finding su dieci dicevano la stessa cosa**, una volta per ogni
/// endpoint dichiarato non colpito. Ora la ragione si scrive una volta e sotto
/// ci sono tutti i soggetti: **nessuno sparisce**.
#[test]
fn i_finding_che_dicono_la_stessa_cosa_la_dicono_una_volta_sola() {
    let log = scrivi_log("gruppo", &["/api/v1/user"]);
    let spec = scrivi_spec(
        "gruppo",
        &[
            "/api/v1/user",
            "/api/v1/a",
            "/api/v1/b",
            "/api/v1/c",
            "/api/v1/d",
            "/api/v1/e",
        ],
    );

    let output = run(&[log.to_str().unwrap(), "--openapi-spec", spec.to_str().unwrap()]);
    let report = stdout(&output);

    // Cinque dichiarati mai osservati: la ragione compare **una volta**.
    let volte = report
        .matches("because declared in the spec but never observed in this log")
        .count();
    assert_eq!(volte, 1, "la ragione si ripete {volte} volte:\n{report}");
    assert!(report.contains("5 endpoint(s), all for the same reason"), "{report}");

    // E ci sono tutti e cinque, uno per riga.
    for atteso in ["/api/v1/a", "/api/v1/b", "/api/v1/c", "/api/v1/d", "/api/v1/e"] {
        assert!(report.contains(atteso), "manca {atteso}:\n{report}");
    }
    // Il conteggio complessivo non è cambiato: raggruppare non è cancellare.
    assert!(report.contains("findings (6)"), "{report}");

    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&spec);
}

/// Ragioni diverse restano finding diversi: il raggruppamento non fonde storie
/// che non sono la stessa.
#[test]
fn ragioni_diverse_non_finiscono_nello_stesso_gruppo() {
    let log = scrivi_log("ragioni", &["/api/v1/user", "/api/v1/segreto"]);
    let spec = scrivi_spec("ragioni", &["/api/v1/user"]);

    let output = run(&[log.to_str().unwrap(), "--openapi-spec", spec.to_str().unwrap()]);
    let report = stdout(&output);
    assert!(report.contains("path and methods both declared"), "{report}");
    assert!(report.contains("not present in the declared inventory"), "{report}");
    assert!(!report.contains("all for the same reason"), "{report}");

    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&spec);
}

/// **La UI web dentro il log.** Senza portata ogni pagina è un endpoint non
/// documentato; con `--path-prefix` no — e le richieste fuori portata sono
/// **contate**, non nascoste.
#[test]
fn la_portata_esclude_il_traffico_non_api_senza_nasconderlo() {
    let log = scrivi_log(
        "portata",
        &[
            "/api/v1/user/repos",
            "/collaudo/frontend/issues",
            "/collaudo/frontend/src/branch/main/README.md",
            "/collaudo/log-parser/issues",
        ],
    );
    let spec = scrivi_spec("portata", &["/api/v1/user/repos"]);

    // Senza portata: tre pagine web diventano Shadow conclamati, e la CI fallisce.
    let senza = run(&[log.to_str().unwrap(), "--openapi-spec", spec.to_str().unwrap()]);
    assert_eq!(senza.status.code(), Some(3));
    let shadow_conclamati = stdout(&senza)
        .lines()
        .filter(|l| l.trim_start().starts_with("Shadow ") && l.contains("severity"))
        .count();
    assert_eq!(shadow_conclamati, 3, "{}", stdout(&senza));

    // Con la portata: nessun falso positivo, e il conteggio di ciò che è
    // rimasto fuori è **detto**, in chiaro, sia su stderr sia nel report.
    let con = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--path-prefix",
        "/api/v1",
    ]);
    assert_eq!(con.status.code(), Some(0), "stderr: {}", stderr(&con));
    assert!(stdout(&con).contains("observed endpoints (1)"), "{}", stdout(&con));
    assert!(
        stdout(&con).contains("3 request(s) outside the analysed scope"),
        "{}",
        stdout(&con)
    );
    assert!(
        stderr(&con).contains("fell outside it and were not examined"),
        "{}",
        stderr(&con)
    );

    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&spec);
}

/// Il conteggio fuori portata esce anche in JSON: chi automatizza deve poter
/// sapere quanto traffico non è stato guardato.
#[test]
fn il_conteggio_fuori_portata_e_nel_json() {
    let log = scrivi_log("portata-json", &["/api/v1/x", "/pagina"]);
    let output = run(&[
        log.to_str().unwrap(),
        "--output-format",
        "json",
        "--path-prefix",
        "/api",
    ]);
    assert!(
        stdout(&output).contains("\"requests_outside_scope\": 1"),
        "{}",
        stdout(&output)
    );
    let _ = std::fs::remove_file(&log);
}

/// Senza `--path-prefix` non cambia niente: la portata è opt-in, e il
/// comportamento preimpostato resta analizzare tutto.
#[test]
fn senza_portata_il_comportamento_e_quello_di_sempre() {
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap(), "--output-format", "json"]);
    assert!(
        stdout(&output).contains("\"requests_outside_scope\": 0"),
        "{}",
        stdout(&output)
    );
    assert!(stdout(&output).contains("\"path_prefix\""), "la chiave è nel manifest");
}

/// Un prefisso che non può combaciare con niente ridurrebbe l'analisi a zero
/// senza dirlo: si rifiuta (§P2).
#[test]
fn un_prefisso_che_non_comincia_con_barra_e_rifiutato() {
    let log = fixture("valid.log");
    let output = run(&[log.to_str().unwrap(), "--path-prefix", "api"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun verdetto senza manifest");
    assert!(
        stderr(&output).contains("path_prefix must start with '/'"),
        "{}",
        stderr(&output)
    );
}

// --- Fase 5: il cancello del demone e del report d'audit -------------------

/// **Il cancello del demone.** La cosa che deve valere sopra ogni altra: a
/// input costante il demone e il comando singolo danno **lo stesso inventario e
/// gli stessi verdetti**. Se divergessero, «§P4 vale anche col tempo di mezzo»
/// sarebbe una frase e non una proprietà.
#[test]
fn cancello_il_demone_e_il_comando_singolo_non_divergono() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let singolo = storico("demone-singolo");
    let demone = storico("demone-continuo");

    let a = run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        singolo.to_str().unwrap(),
    ]);
    assert_eq!(a.status.code(), Some(3), "stderr: {}", stderr(&a));

    let b = run(&[
        "daemon",
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        demone.to_str().unwrap(),
        "--cycles",
        "1",
        "--interval",
        "0",
    ]);
    assert_eq!(b.status.code(), Some(3), "stderr: {}", stderr(&b));

    // Si confrontano gli inventari come li vedrebbe un auditor: è la forma in
    // cui una divergenza si vedrebbe davvero.
    let esporta = |db: &PathBuf| {
        stdout(&run(&[
            "compliance",
            "--history",
            db.to_str().unwrap(),
            "--format",
            "csv",
        ]))
    };
    assert_eq!(esporta(&singolo), esporta(&demone));

    let _ = std::fs::remove_file(&singolo);
    let _ = std::fs::remove_file(&demone);
}

/// Il demone **analizza**, quindi produce manifest: uno per giro, nello
/// storico. «Nessun manifest, nessun verdetto» vale anche qui (§7).
#[test]
fn ogni_giro_del_demone_lascia_un_manifest_nello_storico() {
    let log = fixture("valid.log");
    let db = storico("demone-manifest");
    let output = run(&[
        "daemon",
        log.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
        "--cycles",
        "3",
        "--interval",
        "0",
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("cycle 3:"), "{}", stderr(&output));

    // Tre giri, tre run registrati, e il secondo e il terzo non hanno trovato
    // niente di nuovo: il file non è cresciuto.
    assert_eq!(stderr(&output).matches("never seen before").count(), 1);

    let _ = std::fs::remove_file(&db);
}

/// Il demone non apre nessuna porta e non stampa il report su stdout: il report
/// si chiede con `alerts` e `compliance`.
#[test]
fn il_demone_non_scrive_niente_su_stdout() {
    let log = fixture("valid.log");
    let db = storico("demone-stdout");
    let output = run(&[
        "daemon",
        log.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
        "--cycles",
        "1",
        "--interval",
        "0",
    ]);
    assert!(stdout(&output).is_empty(), "stdout: {}", stdout(&output));
    let _ = std::fs::remove_file(&db);
}

/// **Il cancello del report d'audit.** I quattro valori dell'autenticazione non
/// collassano mai in un binario, e «non osservabile» ha una riga sua anche
/// quando vale zero (§9).
#[test]
fn cancello_il_report_di_compliance_non_collassa_i_quattro_valori() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("audit");
    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);

    let csv = stdout(&run(&["compliance", "--history", db.to_str().unwrap()]));
    assert!(csv.contains("authentication,observable_requests"), "{csv}");
    assert!(csv.contains("not observable"), "{csv}");
    // La base di calcolo sta accanto al verdetto, sempre.
    assert!(csv.lines().count() >= 4, "{csv}");

    let md = stdout(&run(&[
        "compliance",
        "--history",
        db.to_str().unwrap(),
        "--format",
        "markdown",
    ]));
    assert!(md.contains("not a security certification"), "{md}");
    // Tutti e cinque gli stati compaiono, **zeri compresi**: uno stato che
    // sparisce quando è vuoto fa credere che non esista.
    for stato in ["| yes |", "| no |", "| mixed |", "| not observable |", "| not recorded |"] {
        assert!(md.contains(stato), "manca {stato}:\n{md}");
    }

    let json = stdout(&run(&[
        "compliance",
        "--history",
        db.to_str().unwrap(),
        "--format",
        "json",
    ]));
    assert!(json.contains("\"schema\": \"shadow-compliance/1\""), "{json}");
    assert!(json.contains("\"observable_requests\""), "{json}");
    assert!(json.contains("\"not observable\""), "{json}");

    let _ = std::fs::remove_file(&db);
}

/// Un bersaglio inesistente non produce un inventario vuoto: consegnare a un
/// auditor un documento vuoto per un refuso sarebbe la risposta peggiore.
#[test]
fn un_report_di_compliance_su_un_bersaglio_inesistente_non_esce_vuoto() {
    let log = fixture("valid.log");
    let db = storico("audit-bersaglio");
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    let output = run(&[
        "compliance",
        "--history",
        db.to_str().unwrap(),
        "--target",
        "inesistente",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "nessun documento per un refuso");
    assert!(stderr(&output).contains("nothing was exported"), "{}", stderr(&output));

    let _ = std::fs::remove_file(&db);
}

// --- Fase 5: le quattro superfici di supervisione --------------------------

/// **Il cancello della supervisione.** Terminale, pagina, server e applicazione
/// macOS mostrano **la stessa vista**: se divergessero, chi guarda il cruscotto
/// e chi guarda il terminale non potrebbero discutere della stessa cosa.
#[test]
fn cancello_le_viste_di_supervisione_dicono_la_stessa_cosa() {
    let log = fixture("valid.log");
    let spec = spec("stale.json");
    let db = storico("viste");
    run(&[
        log.to_str().unwrap(),
        "--openapi-spec",
        spec.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
    ]);

    let terminale = stdout(&run(&["status", "--history", db.to_str().unwrap()]));
    let json = stdout(&run(&[
        "status",
        "--history",
        db.to_str().unwrap(),
        "--format",
        "json",
    ]));
    let pagina = stdout(&run(&["dashboard", "--history", db.to_str().unwrap()]));

    // Lo stesso endpoint shadow compare in tutte e tre.
    for vista in [&terminale, &json, &pagina] {
        assert!(vista.contains("/api/admin/reset"), "{vista}");
    }
    // E lo stesso conteggio di alert aperti.
    assert!(terminale.contains("open alerts       2"), "{terminale}");
    assert!(json.contains("\"open_alerts\""), "{json}");
    assert!(pagina.contains("Open alerts"), "{pagina}");

    let _ = std::fs::remove_file(&db);
}

/// La pagina si scrive **atomicamente**: chi la sta leggendo vede quella di
/// prima o quella nuova, mai una a metà.
#[test]
fn la_pagina_si_scrive_su_file_e_non_lascia_scarti() {
    let log = fixture("valid.log");
    let db = storico("pagina-file");
    let pagina = std::env::temp_dir().join("shadow-cancello-pagina.html");
    let _ = std::fs::remove_file(&pagina);
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    let output = run(&[
        "dashboard",
        "--history",
        db.to_str().unwrap(),
        "--out",
        pagina.to_str().unwrap(),
        "--refresh",
        "15",
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let contenuto = std::fs::read_to_string(&pagina).expect("la pagina esiste");
    assert!(contenuto.starts_with("<!doctype html>"), "{contenuto}");
    assert!(contenuto.contains("content=\"15\""));
    // Il file temporaneo non resta in giro.
    assert!(!pagina.with_extension("html.tmp").exists());

    let _ = std::fs::remove_file(&pagina);
    let _ = std::fs::remove_file(&db);
}

/// Il demone riscrive la pagina a ogni giro: è ciò che dà la liveness **senza
/// un socket**, perché un browser lasciato aperto rilegge un file da disco.
#[test]
fn il_demone_tiene_aggiornata_la_pagina() {
    let log = fixture("valid.log");
    let db = storico("demone-pagina");
    let pagina = std::env::temp_dir().join("shadow-cancello-demone.html");
    let _ = std::fs::remove_file(&pagina);

    let output = run(&[
        "daemon",
        log.to_str().unwrap(),
        "--history",
        db.to_str().unwrap(),
        "--cycles",
        "1",
        "--interval",
        "0",
        "--dashboard",
        pagina.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let contenuto = std::fs::read_to_string(&pagina).expect("la pagina esiste");
    assert!(contenuto.contains("http-equiv=\"refresh\""), "il browser deve poter seguire");

    let _ = std::fs::remove_file(&pagina);
    let _ = std::fs::remove_file(&db);
}

/// `shadow serve` è l'unico comando che apre un socket, e lo **dice** prima di
/// aprirlo.
///
/// Il test si collega davvero: un server che si può solo avviare non si può
/// mettere sotto verifica, ed è il motivo per cui `--requests` esiste.
#[test]
fn il_server_dice_dove_ascolta_ed_e_in_sola_lettura() {
    use std::io::{Read as _, Write as _};

    let log = fixture("valid.log");
    let db = storico("serve");
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_shadow"))
        .args([
            "serve",
            "--history",
            db.to_str().unwrap(),
            "--bind",
            "127.0.0.1:8793",
            "--requests",
            "1",
        ])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("il binario parte");

    // Si riprova finché non è in ascolto: l'avvio non è istantaneo, e un test
    // che dorme un tempo fisso è un test che ogni tanto fallisce da solo.
    let mut stream = None;
    for _ in 0..50 {
        if let Ok(connected) = std::net::TcpStream::connect("127.0.0.1:8793") {
            stream = Some(connected);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    let mut stream = stream.expect("il server si è messo in ascolto");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("scrive");
    let mut risposta = String::new();
    let _ = stream.read_to_string(&mut risposta);

    assert!(risposta.starts_with("HTTP/1.1 200 OK"), "{risposta}");
    assert!(risposta.contains("/api/admin/reset"), "{risposta}");

    let esito = child.wait_with_output().expect("il server si ferma da solo");
    let diagnostica = String::from_utf8_lossy(&esito.stderr);
    assert!(
        diagnostica.contains("read-only, on http://127.0.0.1:8793/"),
        "deve dire dove ascolta: {diagnostica}"
    );
    // Su localhost non deve esserci l'avviso: è il caso normale.
    assert!(!diagnostica.contains("reachable from outside"), "{diagnostica}");

    let _ = std::fs::remove_file(&db);
}

/// Le viste di sola lettura non producono manifest e non affermano «nessun
/// finding» (§7), come `alerts` e `compliance`.
#[test]
fn le_viste_non_producono_manifest_perche_non_analizzano() {
    let log = fixture("valid.log");
    let db = storico("viste-manifest");
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    let status = stdout(&run(&["status", "--history", db.to_str().unwrap()]));
    assert!(!status.contains("run manifest"), "{status}");
    assert!(status.contains("not a security certification"), "{status}");

    let _ = std::fs::remove_file(&db);
}

/// Un bersaglio inesistente non produce una vista vuota, come per il report
/// d'audit: una schermata tranquilla per un refuso è la risposta peggiore.
#[test]
fn una_vista_su_un_bersaglio_inesistente_non_esce_tranquilla() {
    let log = fixture("valid.log");
    let db = storico("viste-bersaglio");
    run(&[log.to_str().unwrap(), "--history", db.to_str().unwrap()]);

    for comando in ["status", "dashboard"] {
        let output = run(&[comando, "--history", db.to_str().unwrap(), "--target", "inesistente"]);
        assert_eq!(output.status.code(), Some(1), "{comando}");
        assert!(stdout(&output).is_empty(), "{comando}");
        assert!(stderr(&output).contains("no target named"), "{comando}");
    }

    let _ = std::fs::remove_file(&db);
}
