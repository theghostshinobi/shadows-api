//! Lo schema del database e le sue migrazioni.
//!
//! # Come si evolve
//!
//! La versione dello schema vive in `PRAGMA user_version`, e le migrazioni sono
//! un elenco ordinato: aprire un database applica tutte quelle che gli mancano,
//! dentro una transazione. Un database più **nuovo** del binario viene rifiutato
//! con un errore chiaro invece che aperto a caso: leggere un formato che non si
//! conosce è il modo in cui si corrompe un archivio d'audit (§P2).
//!
//! La regola per chi aggiungerà migrazioni: **si aggiunge in coda, non si
//! modifica una migrazione già rilasciata.** Un archivio già scritto non si
//! riscrive, esattamente come i motivi di scarto e lo schema del report (§7).

use rusqlite::{Connection, Transaction};

/// Versione dello schema che questo binario sa leggere e scrivere.
pub const SCHEMA_VERSION: i64 = 3;

/// Le migrazioni, in ordine. L'indice `i` porta dallo schema `i` allo `i + 1`.
const MIGRATIONS: &[fn(&Transaction<'_>) -> rusqlite::Result<()>] =
    &[migration_1, migration_2, migration_3];

/// Porta il database alla [`SCHEMA_VERSION`], o dice quale versione ha trovato.
///
/// # Perché la transazione è `Immediate`, e perché la versione si rilegge dentro
///
/// Due primi run concorrenti sullo stesso file nuovo sono un caso ordinario, non
/// avversario: due job di CI che partono insieme. Con una transazione
/// `Deferred` — il default di rusqlite — il perdente non ha uno snapshot di
/// lettura da far confliggere, quindi dopo il commit del vincitore rieseguiva la
/// migrazione su un database che aveva già le tabelle, e moriva con
/// `table runs already exists`. Misurato: **venti fallimenti su venti**
/// iterazioni con due soli processi.
///
/// La difesa è quella che `record_run` usava già: si prende subito il lucchetto
/// di scrittura, e **dentro** la transazione si rilegge `user_version`, così chi
/// arriva secondo vede il lavoro del primo e non lo rifà.
pub fn migrate(connection: &mut Connection) -> rusqlite::Result<i64> {
    loop {
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Riletta **qui**, non prima: fra il controllo e la scrittura ci può
        // essere un altro processo.
        let found: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        // Un valore che non riconosciamo — più nuovo, o negativo perché
        // qualcuno ci ha scritto a mano — si restituisce **così com'è**. Prima
        // un `found` negativo diventava `usize::MAX` nel cast, il ciclo era
        // vuoto, e la funzione dichiarava lo schema corrente su un database
        // senza tabelle: il run se ne accorgeva solo a fine analisi, con un
        // messaggio che parlava di permessi.
        if found > SCHEMA_VERSION || found < 0 {
            transaction.rollback()?;
            return Ok(found);
        }
        if found == SCHEMA_VERSION {
            transaction.rollback()?;
            return Ok(found);
        }

        let step = found as usize;
        MIGRATIONS[step](&transaction)?;
        transaction.pragma_update(None, "user_version", (step + 1) as i64)?;
        transaction.commit()?;
    }
}

/// Schema iniziale.
///
/// `target` compare in ogni tabella perché la multi-tenancy di §9 qui significa
/// **separazione dei bersagli d'analisi**: un demone sorveglia più servizi, e
/// gli inventari di due servizi non si mescolano mai. Non è multi-utente, e non
/// pretende di esserlo: chi può leggere il file lo decide il sistema operativo.
fn migration_1(t: &Transaction<'_>) -> rusqlite::Result<()> {
    t.execute_batch(
        "
        CREATE TABLE runs (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            target           TEXT NOT NULL,
            run_timestamp    TEXT NOT NULL,
            shadow_version   TEXT NOT NULL,
            ruleset_version  TEXT NOT NULL,
            manifest_json    TEXT NOT NULL
        );

        CREATE INDEX runs_by_target ON runs (target, id);

        CREATE TABLE run_findings (
            run_id          INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
            endpoint_id     TEXT,
            declared_path   TEXT,
            classification  TEXT NOT NULL,
            confidence      TEXT NOT NULL,
            severity        TEXT NOT NULL,
            evidence        TEXT NOT NULL
        );

        CREATE INDEX run_findings_by_run ON run_findings (run_id);

        CREATE TABLE endpoints (
            target              TEXT NOT NULL,
            endpoint_id         TEXT NOT NULL,
            path_pattern        TEXT NOT NULL,
            first_seen_run      INTEGER NOT NULL REFERENCES runs (id),
            last_seen_run       INTEGER NOT NULL REFERENCES runs (id),
            last_classification TEXT,
            acknowledged        INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (target, endpoint_id)
        );
        ",
    )
}

/// Segna quali endpoint hanno **fatto scattare un alert**.
///
/// Prima l'alert si deduceva dall'ultima classificazione, e questo lo rendeva
/// cancellabile da un run successivo che semplicemente non aveva niente da dire
/// su quell'endpoint — cioè dalla modalità d'uso preimpostata di Shadow, quella
/// senza inventario dichiarato. Un alert non ancora preso in carico spariva in
/// silenzio: il falso negativo silenzioso che §P1 chiama il fallimento peggiore
/// del prodotto, sul meccanismo che esiste per non far perdere un endpoint.
///
/// Con una colonna sua, l'alert è un **fatto avvenuto**: nasce quando un
/// endpoint è stato visto `Shadow`, e lo toglie solo chi lo prende in carico.
fn migration_2(t: &Transaction<'_>) -> rusqlite::Result<()> {
    t.execute_batch(
        "
        ALTER TABLE endpoints ADD COLUMN alerted INTEGER NOT NULL DEFAULT 0;
        UPDATE endpoints SET alerted = 1 WHERE last_classification = 'Shadow';
        ",
    )
}

/// Aggiunge allo stato per endpoint ciò che serve a un **report d'audit**:
/// metodi, conteggio delle osservazioni, e soprattutto lo **stato
/// dell'autenticazione con la sua base di calcolo**.
///
/// §9 chiede che il report di compliance porti i **quattro** valori di §6,
/// *«non osservabile» compreso, mai collassati in un binario presenza/assenza*.
/// Perché quello resti possibile, lo storico deve conservare quattro valori e
/// non due — e deve conservare anche **su quante richieste osservabili** il
/// verdetto si regge, o «senza auth» e «senza auth, su 3 richieste su 5000»
/// finirebbero per assomigliarsi.
///
/// `auth_observed` è `NULL` per le righe scritte prima di questa migrazione:
/// non è «non osservabile», è **non registrato**, ed è una terza cosa ancora.
/// Il report lo dice invece di indovinare.
fn migration_3(t: &Transaction<'_>) -> rusqlite::Result<()> {
    t.execute_batch(
        "
        ALTER TABLE endpoints ADD COLUMN methods TEXT;
        ALTER TABLE endpoints ADD COLUMN observation_count INTEGER;
        ALTER TABLE endpoints ADD COLUMN auth_observed TEXT;
        ALTER TABLE endpoints ADD COLUMN auth_observable_count INTEGER;
        ",
    )
}
