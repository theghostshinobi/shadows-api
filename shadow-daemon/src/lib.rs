//! `shadow-daemon` — l'esecuzione continua di Shadow (§9, Fase 5).
//!
//! # Cosa c'è qui dentro, e cosa no
//!
//! Qui c'è **il tempo**: ogni quanto si guarda, quante volte, quando ci si
//! ferma. Non c'è niente di ciò che un giro *fa* — leggere, normalizzare,
//! classificare, redigere, ricordare — perché quello è la pipeline delle Fasi
//! 1–4 e l'orchestrazione è di `shadow-cli` (§8). Un demone che si portasse
//! dentro una copia della pipeline sarebbe il modo più rapido di far divergere
//! i verdetti di `shadow` da quelli di `shadow daemon`.
//!
//! # §P4 con il tempo di mezzo
//!
//! Un demone introduce tre cose che un comando singolo non ha: un **orologio**,
//! un **ordine di arrivo** e uno **stato accumulato**. Ognuna è un modo di far
//! dipendere un verdetto da qualcosa che non è l'input.
//!
//! La difesa è che qui non si decide niente: questo modulo dice *quando*
//! chiamare la pipeline, e la pipeline riceve ogni volta l'inventario
//! **accumulato dall'inizio**, non il pezzo arrivato nell'ultimo minuto. Al
//! ciclo N il verdetto è quello che un comando singolo darebbe sul file letto
//! fino a lì — e c'è una fixture che confronta le due cose carattere per
//! carattere.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;
use std::time::Duration;

/// Quante volte al massimo si può chiedere di guardare, se non si dice
/// «per sempre». Serve solo a rendere l'intenzione esplicita nei messaggi.
pub const UNLIMITED: &str = "unlimited";

/// Perché un demone non può partire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonError {
    /// L'intervallo è zero e i cicli sono illimitati: sarebbe un ciclo stretto
    /// che rilegge il file senza sosta.
    BusyLoop,
}

impl fmt::Display for DaemonError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BusyLoop => write!(
                f,
                "an interval of 0 with no cycle limit would spin without ever sleeping. \
                 Give an interval, or a number of cycles"
            ),
        }
    }
}

/// Il ritmo con cui il demone guarda il log.
///
/// Non fa niente di suo: dice **se** è il momento di un altro giro, e dorme fra
/// un giro e l'altro. Tenerlo separato dal lavoro è ciò che permette di
/// verificarne il comportamento senza un log, un orologio vero o un'attesa.
#[derive(Debug, Clone)]
pub struct Schedule {
    interval: Duration,
    limit: Option<u64>,
    done: u64,
}

impl Schedule {
    /// Un ritmo con un intervallo e, se lo si vuole, un numero di giri.
    ///
    /// `limit` a `None` significa «finché non lo fermi». Un intervallo di zero
    /// **con** un limite è legittimo — è come si eseguono le verifiche, e come
    /// si fa un giro solo da `cron` — mentre senza limite sarebbe un ciclo
    /// stretto, e si rifiuta invece di consumare una macchina in silenzio.
    pub fn new(interval: Duration, limit: Option<u64>) -> Result<Self, DaemonError> {
        if interval.is_zero() && limit.is_none() {
            return Err(DaemonError::BusyLoop);
        }
        Ok(Self {
            interval,
            limit,
            done: 0,
        })
    }

    /// Dice se fare un altro giro, **dormendo** prima di tutti tranne il primo.
    ///
    /// Il primo giro parte subito: chi avvia un demone vuole sapere com'è messo
    /// adesso, non fra un minuto.
    pub fn next_cycle(&mut self) -> bool {
        if self.limit.is_some_and(|limit| self.done >= limit) {
            return false;
        }
        if self.done > 0 && !self.interval.is_zero() {
            std::thread::sleep(self.interval);
        }
        self.done += 1;
        true
    }

    /// Ogni quanto si guarda.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Quanti giri sono stati fatti.
    pub fn completed(&self) -> u64 {
        self.done
    }

    /// Come si descrive il limite all'utente.
    pub fn limit_description(&self) -> String {
        match self.limit {
            Some(limit) => limit.to_string(),
            None => UNLIMITED.to_string(),
        }
    }
}
