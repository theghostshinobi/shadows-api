//! Le verifiche del ritmo (§10, Fase 5).
//!
//! Il ritmo è separato dal lavoro proprio per poter essere verificato senza un
//! log, senza un orologio vero e senza aspettare.

use std::time::{Duration, Instant};

use shadow_daemon::{DaemonError, Schedule};

#[test]
fn il_primo_giro_parte_subito() {
    // Chi avvia un demone vuole sapere com'è messo **adesso**, non fra un
    // minuto: se il primo giro dormisse, l'avvio sembrerebbe non funzionare.
    let mut schedule = Schedule::new(Duration::from_secs(30), Some(1)).expect("valido");
    let inizio = Instant::now();
    assert!(schedule.next_cycle());
    assert!(
        inizio.elapsed() < Duration::from_secs(1),
        "il primo giro ha dormito"
    );
    assert_eq!(schedule.completed(), 1);
    assert!(!schedule.next_cycle(), "il limite deve fermare");
}

#[test]
fn un_intervallo_zero_senza_limite_e_rifiutato() {
    // Sarebbe un ciclo stretto che rilegge il file senza sosta: si rifiuta
    // invece di consumare una macchina in silenzio (§P2).
    assert_eq!(
        Schedule::new(Duration::ZERO, None).unwrap_err(),
        DaemonError::BusyLoop
    );
    // Con un limite è invece legittimo: è come si eseguono le verifiche, e come
    // si fa un giro solo da `cron`.
    assert!(Schedule::new(Duration::ZERO, Some(3)).is_ok());
}

#[test]
fn senza_limite_si_continua() {
    let mut schedule = Schedule::new(Duration::ZERO, Some(4)).expect("valido");
    let mut giri = 0;
    while schedule.next_cycle() {
        giri += 1;
    }
    assert_eq!(giri, 4);
    assert_eq!(schedule.completed(), 4);

    let illimitato = Schedule::new(Duration::from_secs(60), None).expect("valido");
    assert_eq!(illimitato.limit_description(), "unlimited");
}

#[test]
fn il_messaggio_del_ciclo_stretto_dice_cosa_fare() {
    let messaggio = DaemonError::BusyLoop.to_string();
    assert!(messaggio.contains("Give an interval, or a number of cycles"), "{messaggio}");
}
