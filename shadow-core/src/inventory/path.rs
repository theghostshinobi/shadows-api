//! Forma canonica di un path e riconoscimento dei segmenti "tipo-ID" (§7, Fase 2).

use crate::ruleset::HEX_ID_LENGTHS;

/// Scompone un path grezzo nella sua forma **canonica**: una sequenza di
/// segmenti su cui due path che sono la stessa cosa combaciano, e due path che
/// sono cose diverse no.
///
/// # Come si normalizza l'encoding, e perché così
///
/// Il rischio ha due facce opposte, e la regola deve reggerle entrambe (§9,
/// Fase 2):
///
/// - se `/api/%2e%2e/x` e `/api/../x` restassero due endpoint distinti,
///   l'inventario si gonfierebbe di doppioni della stessa cosa;
/// - se `/a%2fb` e `/a/b` diventassero lo stesso endpoint, due cose diverse
///   verrebbero fuse — ed è il falso raggruppamento che questa fase esiste per
///   evitare.
///
/// La regola: si divide **prima** sui `/` reali, poi si decodifica ogni
/// segmento **una volta sola**, poi si ri-codificano i caratteri che
/// altrimenti renderebbero ambigua la forma canonica (`%`, `/`, i caratteri di
/// controllo). Ne segue che:
///
/// - `/api/%2e%2e/x` e `/api/../x` → stessa forma canonica: sono la stessa cosa;
/// - `/a%2fb` resta distinto da `/a/b`: uno ha un segmento, l'altro due;
/// - `/api/%252e` (doppia codifica) **non** diventa `/api/.`: decodificare a
///   ripetizione cancellerebbe il segnale, e una doppia codifica è
///   esattamente il genere di cosa che chi legge questo tool vuole vedere.
///
/// Una decodifica che produce byte non UTF-8 lascia il segmento **com'era**:
/// non si indovina (§P2).
///
/// Il `/` iniziale non produce un segmento vuoto; un `/` finale sì, così che
/// `/api/users/` resti distinto da `/api/users` — nel dubbio non si aggrega
/// (§P3).
pub fn canonical_segments(raw_path: &str) -> Vec<String> {
    let trimmed = raw_path.strip_prefix('/').unwrap_or(raw_path);
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed
        .split('/')
        .map(|segment| reencode(&decode_once(segment)))
        .collect()
}

/// Ricompone una sequenza di segmenti canonici nella stringa di un pattern.
pub fn join_segments(segments: &[String]) -> String {
    let mut out = String::with_capacity(segments.iter().map(|s| s.len() + 1).sum());
    for segment in segments {
        out.push('/');
        out.push_str(segment);
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// Dice se un segmento ha la forma di un identificatore (§7).
///
/// È metà della "evidenza forte" che §P3 richiede prima di considerare
/// variabile una posizione; l'altra metà è la cardinalità. Il catalogo è corto
/// di proposito: ogni forma in più è un modo in più di scambiare una parola per
/// un identificatore e far sparire un endpoint dentro un gruppo.
///
/// `admin`, `me`, `current`, `latest`, `search` non hanno nessuna di queste
/// forme, e questo è il punto: non diventeranno mai `{id}`, neanche circondati
/// da un milione di identificatori numerici.
pub fn is_id_like(segment: &str) -> bool {
    is_numeric(segment) || is_uuid(segment) || is_opaque_hex(segment)
}

fn is_numeric(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit())
}

fn is_uuid(segment: &str) -> bool {
    if segment.len() != 36 {
        return false;
    }
    segment.bytes().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

fn is_opaque_hex(segment: &str) -> bool {
    HEX_ID_LENGTHS.contains(&segment.len()) && segment.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Decodifica le sequenze `%XX` **una volta sola**.
///
/// Una sequenza malformata (`%zz`, `%` finale) resta letterale: è un dato
/// osservato, non un errore da correggere.
fn decode_once(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Byte non UTF-8 dopo la decodifica: si tiene il segmento originale invece
    // di inventarsi caratteri di sostituzione (§P2).
    String::from_utf8(out).unwrap_or_else(|_| segment.to_string())
}

/// Ri-codifica i caratteri che renderebbero ambigua la forma canonica.
///
/// Sono quattro famiglie: `/` (altrimenti un segmento si spaccerebbe per due),
/// `%` (altrimenti la doppia codifica collasserebbe su quella singola), le
/// graffe (altrimenti un path che contiene letteralmente `{id}` — un client mal
/// configurato che manda il template invece del valore — si confonderebbe con
/// il segnaposto dei segmenti variabili, che è un falso raggruppamento) e i
/// caratteri di controllo (illeggibili in un report, o peggio).
fn reencode(decoded: &str) -> String {
    let mut out = String::with_capacity(decoded.len());
    // Si itera per **caratteri**, non per byte: i caratteri da ri-codificare
    // sono tutti ASCII, e trattare per byte spezzerebbe l'UTF-8 multibyte.
    for ch in decoded.chars() {
        if crate::ruleset::must_be_reencoded(ch) {
            push_escaped_char(&mut out, ch);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Ri-codifica un carattere, **tutti i suoi byte**.
///
/// Non basta trattare il primo: `U+200B` in UTF-8 sono tre byte, e troncarlo a
/// uno solo avrebbe prodotto una sequenza diversa da quella di partenza — cioè
/// avrebbe cambiato il dato invece di renderlo visibile.
fn push_escaped_char(out: &mut String, ch: char) {
    let mut buffer = [0_u8; 4];
    for byte in ch.encode_utf8(&mut buffer).as_bytes() {
        push_escape(out, *byte);
    }
}

fn push_escape(out: &mut String, byte: u8) {
    out.push('%');
    out.push(hex_digit(byte >> 4));
    out.push(hex_digit(byte & 0x0f));
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
