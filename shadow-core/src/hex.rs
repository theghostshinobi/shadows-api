//! Codifica esadecimale minuscola — l'unica del progetto.
//!
//! Serve in tre punti che devono produrre esattamente la stessa forma: il
//! digest degli input nel `RunManifest` (§6), l'identificatore stabile
//! dell'endpoint, e il digest della specifica dichiarata. Averne una copia per
//! crate significherebbe che un giorno due di loro divergono di una lettera
//! maiuscola e nessuno se ne accorge finché un confronto non fallisce.

/// Codifica dei byte in esadecimale minuscolo.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(digit(byte >> 4));
        out.push(digit(byte & 0x0f));
    }
    out
}

fn digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}
