//! Le verifiche del `Redactor` (§P5, Fase 4).
//!
//! Il fallimento da presidiare: il report che diventa esso stesso una falla nel
//! momento in cui qualcuno lo incolla in un ticket condiviso.

use std::path::Path;

use shadow_core::redact::{REDACTED, REDACTED_PATH};
use shadow_core::Redactor;

#[test]
fn un_jwt_in_un_path_non_esce_in_chiaro() {
    let redactor = Redactor::enabled();
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r";

    let masked = redactor.text(&format!("/api/reset/{jwt}"));

    assert_eq!(masked, format!("/api/reset/{REDACTED}"));
    assert!(!masked.contains("eyJhbGci"));
}

#[test]
fn unemail_in_un_path_non_esce_in_chiaro() {
    let redactor = Redactor::enabled();
    let masked = redactor.text("/api/users/mario.rossi@bancaxyz.it");

    assert_eq!(masked, format!("/api/users/{REDACTED}"));
    assert!(!masked.contains("bancaxyz"));
}

#[test]
fn il_valore_di_un_campo_sensibile_sparisce_ma_il_nome_resta() {
    // Sapere **che** c'era un token è informazione utile; sapere quale è una
    // falla.
    let redactor = Redactor::enabled();

    assert_eq!(redactor.text("password=hunter2"), format!("password={REDACTED}"));
    assert_eq!(redactor.text("api_key=abc"), format!("api_key={REDACTED}"));
    assert_eq!(redactor.text("page=2"), "page=2");
}

#[test]
fn un_percorso_di_filesystem_perde_la_struttura_e_tiene_il_nome() {
    // §P5 esteso in v1.2: un percorso rivela cliente e struttura interna, e
    // viaggia dritto nel file consegnato all'auditor. Quale file sia analizzato
    // resta però informazione d'audit legittima.
    let redactor = Redactor::enabled();
    let masked = redactor.path(Path::new("/home/utente/clienti/bancaXYZ/logs/access.log"));

    assert_eq!(masked, format!("{REDACTED_PATH}/access.log"));
    assert!(!masked.contains("bancaXYZ"));
    assert!(!masked.contains("clienti"));
}

#[test]
fn cio_che_non_e_un_segreto_resta_leggibile() {
    // Un report tutto mascherato è inutile quanto uno che perde segreti.
    let redactor = Redactor::enabled();

    assert_eq!(redactor.text("/api/users/{id}"), "/api/users/{id}");
    assert_eq!(redactor.text("/api/billing/invoices"), "/api/billing/invoices");
    assert_eq!(
        redactor.text("not present in the declared inventory"),
        "not present in the declared inventory"
    );
}

#[test]
fn spento_non_maschera_niente() {
    // `--show-raw-values` è la deroga esplicita di chi sa dove finirà l'output.
    let redactor = Redactor::disabled();

    assert_eq!(redactor.text("password=hunter2"), "password=hunter2");
    assert!(!redactor.is_enabled());
}

#[test]
fn nel_dubbio_maschera() {
    // §P3: un valore mascherato per eccesso costa una domanda, un segreto in
    // chiaro costa un incidente.
    let redactor = Redactor::enabled();
    let opaque = "AKIAIOSFODNN7EXAMPLE1234567890AB";

    assert_eq!(redactor.text(opaque), REDACTED);
}

// --- la difesa che il tool spegneva da sé ----------------------------------
//
// Trovati da due giri di revisione avversaria e corretti su richiesta esplicita
// del fondatore. Non decidono il **perimetro** del `Redactor`, che resta la
// domanda aperta rimandata al collaudo: chiudono i punti in cui una regola del
// tool ne disattivava un'altra. Il caso che resta aperto ha un test suo, in
// fondo, perché un limite scritto in un test non si dimentica.

/// Un escape prodotto dalla normalizzazione non deve **incollare** due parti in
/// una stringa che nessuna regola può più riconoscere.
#[test]
fn avversaria_un_escape_non_incolla_due_parti_in_un_blob_irriconoscibile() {
    let redactor = Redactor::enabled();
    // La prima metà, da sola, è un blob opaco: che dopo di lei ci sia una barra
    // codificata non la rende meno segreta.
    let mascherato = redactor.text("/api/session/aB3dEfgH7jKlmN9pQr1sT2uVwXyZ%2FmN9pQr");
    assert!(mascherato.contains("<redacted>"), "{mascherato}");
    assert!(!mascherato.contains("aB3dEfgH7jKl"), "{mascherato}");
}

/// **La regressione da non rifare.** Aggiungere `%` all'alfabeto delle stringhe
/// opache sembrava la correzione ovvia, e portava il mascheramento dallo 0% al
/// 91,2% su un corpus realistico di path GitLab, dove il progetto è un
/// parametro con la barra codificata. La soglia di COLLAUDO.md §2 è il 15%.
#[test]
fn avversaria_un_path_legittimo_con_una_barra_codificata_resta_leggibile() {
    let redactor = Redactor::enabled();
    for path in [
        "/api/v4/projects/acme-platform%2Fbilling-service-v2/repository/branches",
        "/artifactory/api/storage/docker-local%2Fteam9%2Fapp/latest",
        "/api/v1/documenti/relazione%2Fannuale%2Fdefinitiva",
    ] {
        let visto = redactor.text(path);
        assert_eq!(visto, path, "mascherato senza motivo: {visto}");
    }
}

/// Il segnale che `reencode` esiste apposta per mostrare — un client che manda
/// il template invece del valore — non deve sparire dietro `<redacted>`.
#[test]
fn un_template_mandato_per_sbaglio_resta_visibile() {
    let redactor = Redactor::enabled();
    let path = "/api/v1/reports/%7Bcustomer_id%7D-2024-summary";
    assert_eq!(redactor.text(path), path);
}

/// **Il difetto di ordine.** Il controllo `chiave=valore` girava sull'intera
/// parola: un path che contiene «session» rendeva sensibile *tutto il path*, e
/// il valore mascherato diventava ciò che stava dopo il primo `=` — cioè quasi
/// niente, mentre il segreto usciva in chiaro come se fosse il nome della
/// chiave.
#[test]
fn avversaria_una_parola_sensibile_nel_path_non_fa_uscire_il_segreto_come_nome() {
    let redactor = Redactor::enabled();
    let mascherato = redactor.text("/api/session/aB3dEfgH7jKlmN9pQr1sT2uVwXyZ==");

    assert!(
        !mascherato.contains("aB3dEfgH7jKl"),
        "il segreto è uscito come 'nome' della coppia: {mascherato}"
    );
    // La struttura del path resta leggibile: è parte del verdetto (§P5).
    assert!(mascherato.starts_with("/api/session/"), "{mascherato}");
}

/// **La punteggiatura della frase.** `text` spezza in parole solo su spazi e
/// tabulazioni, quindi un path citato dentro l'evidenza di un finding arriva
/// con la virgola ancora attaccata. Lo stesso path usciva `<redacted>` nella
/// colonna del soggetto e **in chiaro due caratteri dopo**, sulla stessa riga.
#[test]
fn avversaria_una_virgola_attaccata_non_fa_passare_un_segreto() {
    let redactor = Redactor::enabled();
    let mascherato = redactor.text(
        "path declared in the spec as /api/box/aB3dEfgH7jKlmN9pQr1sT2uVwXyZ, but method(s) DELETE were observed",
    );
    assert!(
        !mascherato.contains("aB3dEfgH7jKlmN9pQr1sT2uVwXyZ"),
        "{mascherato}"
    );
    // La frase resta leggibile: si maschera il valore, non la spiegazione.
    assert!(mascherato.contains("but method(s) DELETE were observed"), "{mascherato}");
}

/// Il **nome del file** finisce nel manifest, cioè nel file che viaggia. La
/// directory era mascherata e il nome no, anche quando il nome era un segreto o
/// un'email.
#[test]
fn avversaria_un_segreto_nel_nome_del_file_non_finisce_nel_manifest() {
    let redactor = Redactor::enabled();
    let con_email = redactor.path(std::path::Path::new("/clienti/mario.rossi@example.com.log"));
    assert!(!con_email.contains("mario.rossi"), "{con_email}");

    let con_token = redactor.path(std::path::Path::new(
        "/clienti/session-aB3dEfgH7jKlmN9pQr1sT2uVwXyZ.log",
    ));
    assert!(!con_token.contains("aB3dEfgH7jKl"), "{con_token}");

    // Un nome ordinario continua a passare: quale file sia stato analizzato è
    // informazione d'audit legittima (§P5).
    let normale = redactor.path(std::path::Path::new("/var/log/nginx/access.log"));
    assert!(normale.ends_with("/access.log"), "{normale}");
}

/// La coppia `chiave=valore` continua a funzionare dov'è utile, dentro un path
/// e fuori: sapere *che* c'era un token è informazione, sapere quale è una
/// falla.
#[test]
fn la_coppia_chiave_valore_continua_a_tenere_la_chiave_e_mascherare_il_valore() {
    let redactor = Redactor::enabled();
    assert_eq!(redactor.text("token=abc123"), "token=<redacted>");
    assert_eq!(
        redactor.text("/api/v1/token=abc123"),
        "/api/v1/token=<redacted>"
    );
}

// --- il limite che resta, scritto perché non si dimentichi ------------------

/// **Questo test dice che un segreto esce in chiaro, ed è voluto.**
///
/// Un segreto base64 corto spezzato a metà da una barra codificata diventa due
/// pezzi che nessuno dei due arriva alla soglia dei 24 caratteri. Chiuderlo
/// vuol dire abbassare la soglia o aggiungere una misura di entropia, cioè
/// **decidere il perimetro del `Redactor`** — la domanda che la v1.7 ha
/// deliberatamente rimandato al collaudo e che non spetta all'agente.
///
/// Il test esiste perché il giorno in cui il perimetro si decide, questo caso
/// sia sul tavolo invece che dimenticato: quando la decisione arriverà, questo
/// test fallirà, ed è il modo giusto di accorgersene.
#[test]
fn limite_noto_un_segreto_corto_spezzato_da_un_escape_non_viene_riconosciuto() {
    let redactor = Redactor::enabled();
    let path = "/api/session/aB3dEfgH7jKl%2FmN9pQr1sT2u";
    assert_eq!(
        redactor.text(path),
        path,
        "se questo test fallisce, il perimetro del Redactor è cambiato: aggiornare COLLAUDO.md e PROGRESS.md"
    );
}

/// Stesso limite, altra faccia: la prova richiede **almeno una cifra**, e un
/// segreto base64 che per caso non ne abbia nessuna passa in chiaro. Togliere
/// la richiesta della cifra mascherebbe le parole lunghe del vocabolario: è di
/// nuovo il perimetro, non un difetto da correggere di nascosto.
#[test]
fn limite_noto_un_segreto_senza_nessuna_cifra_non_viene_riconosciuto() {
    let redactor = Redactor::enabled();
    let path = "/api/download/aBdEfgHjKlmNpQrsTuVwXyZabcdef";
    assert_eq!(redactor.text(path), path);
}
