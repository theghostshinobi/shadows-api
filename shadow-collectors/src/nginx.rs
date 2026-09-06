//! Il Collector per il formato `combined` di nginx — il primo, e in Fase 1
//! l'unico (§9, Fase 1).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use shadow_core::{ObservedAuth, ObservedRequest, SourceRef};

use crate::collector::{Collector, DiscardReason};

/// Formato del tempo nel `combined` di nginx: `10/Oct/2023:13:55:36 +0000`.
const TIME_FORMAT: &str = "%d/%b/%Y:%H:%M:%S %z";

/// Caratteri ammessi in un metodo HTTP oltre alle lettere e alle cifre
/// (RFC 9110, definizione di *token*).
const METHOD_EXTRA_CHARS: &[u8] = b"!#$%&'*+-.^_`|~";

/// Collector per il `log_format combined` di nginx, il preimpostato di ogni
/// installazione:
///
/// ```text
/// $remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent"
/// ```
///
/// # Cosa questo formato **non** dice
///
/// Il `combined` non trasporta gli header della richiesta, quindi
/// dell'autenticazione si sa quasi nulla. La conseguenza è nel modello: l'auth
/// di ogni riga è [`ObservedAuth::NotObservable`], **non**
/// [`ObservedAuth::Absent`] — che il log taccia non è una prova che
/// l'autenticazione mancasse (§6, §P2).
///
/// L'unica eccezione è `$remote_user`: quando è valorizzato, un utente
/// autenticato è stato registrato, e allora l'auth è
/// [`ObservedAuth::Present`] con schema **non specificato**. Che
/// l'autenticazione ci fosse è un fatto; che fosse `Basic` sarebbe
/// un'inferenza, e il campo si chiama `scheme`, non `probable_scheme` (§6,
/// v1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NginxCombinedCollector;

impl NginxCombinedCollector {
    /// Nome canonico del formato, cioè il valore che `log_format` (§7) assume
    /// per questo Collector.
    pub const FORMAT_NAME: &'static str = "nginx-combined";

    /// La stessa grammatica, scritta nella sintassi con cui nginx la dichiara.
    ///
    /// Serve a due cose, e la seconda è la ragione per cui vive qui e non in un
    /// commento: è la forma da suggerire all'utente che deve dichiarare il
    /// proprio formato, ed è ciò che un test confronta con questo Collector per
    /// impedire che le due grammatiche divergano nel tempo.
    pub const FORMAT_SPEC: &'static str = concat!(
        "$remote_addr - $remote_user [$time_local] ",
        r#""$request" $status $body_bytes_sent "$http_referer" "$http_user_agent""#
    );

    /// Deduce l'autenticazione osservata dal campo `$remote_user`.
    ///
    /// `-` significa "nessun utente registrato", che **non** significa "nessuna
    /// autenticazione": un token `Bearer` non lascia traccia in questo formato.
    /// Quindi `-` → [`ObservedAuth::NotObservable`].
    ///
    /// Un valore diverso da `-` significa che nginx ha registrato un utente
    /// autenticato, quindi l'autenticazione c'era: è un **fatto**, e va
    /// conservato. Quale schema fosse sarebbe invece un'**inferenza** — nella
    /// configurazione standard `$remote_user` lo popola l'HTTP Basic, ma altri
    /// moduli possono farlo — e su un campo che finisce in un report d'audit
    /// "quasi sempre giusto" è il dato inventato-che-sembra-vero vietato da
    /// §P2. Quindi schema **non specificato** (§6, v1.4).
    ///
    /// Il valore dell'utente non entra mai nel modello, come ogni credenziale.
    fn parse_auth(remote_user: &str) -> ObservedAuth {
        remote_user_auth(remote_user)
    }
}

impl Collector for NginxCombinedCollector {
    fn format_name(&self) -> &str {
        Self::FORMAT_NAME
    }

    fn expected_shape(&self) -> &str {
        "<remote_addr> - <remote_user> [<time>] \"<method> <path> HTTP/<version>\" <status> <bytes> \"<referer>\" \"<user-agent>\""
    }

    fn parse_line(&self, line: &str, source: SourceRef) -> Result<ObservedRequest, DiscardReason> {
        let rest = line.trim_end();

        // La grammatica è percorsa per intero, in ordine: ogni campo viene letto
        // dalla posizione che gli spetta o la riga cade (§P2). Non esiste un
        // ramo che "prova a indovinare" un campo mancante.
        let (_remote_addr, rest) = take_token(rest).ok_or(DiscardReason::MalformedLine)?;
        let (_ident, rest) = take_token(rest).ok_or(DiscardReason::MalformedLine)?;
        let (remote_user, rest) = take_token(rest).ok_or(DiscardReason::MalformedLine)?;
        let (time_local, rest) = take_bracketed(rest).ok_or(DiscardReason::MalformedLine)?;
        let (request, rest) = take_quoted(rest).ok_or(DiscardReason::MalformedLine)?;
        let (status, rest) = take_token(rest).ok_or(DiscardReason::MalformedLine)?;
        let (body_bytes, rest) = take_token(rest).ok_or(DiscardReason::MalformedLine)?;
        // Referer e user agent sono ciò che distingue `combined` da `common`:
        // se mancano, il file è in un altro formato e va detto, non adattato.
        let (_referer, rest) = take_quoted(rest).ok_or(DiscardReason::MalformedLine)?;
        let (_user_agent, rest) = take_quoted(rest).ok_or(DiscardReason::MalformedLine)?;
        if !rest.trim().is_empty() {
            return Err(DiscardReason::MalformedLine);
        }

        if !parse_body_bytes(body_bytes) {
            return Err(DiscardReason::MalformedLine);
        }

        let status_code = parse_status(status).ok_or(DiscardReason::InvalidStatusCode)?;
        let timestamp = parse_time_local(time_local).ok_or(DiscardReason::InvalidTimestamp)?;
        let (method, target) = parse_request_line(request).ok_or(DiscardReason::InvalidRequestLine)?;

        // Il path resta **grezzo**: nessuna decodifica percentuale, nessuna
        // normalizzazione. Quelle sono Fase 2 (§6).
        let (raw_path, query) = match target.split_once('?') {
            Some((path, query)) => (path, query),
            None => (target, ""),
        };

        Ok(ObservedRequest {
            method: method.to_string(),
            raw_path: raw_path.to_string(),
            query_params: parse_query(query),
            auth: Self::parse_auth(remote_user),
            status_code,
            timestamp,
            source,
        })
    }
}

/// Legge il prossimo campo separato da spazi, restituendo `(campo, resto)`.
fn take_token(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start_matches(' ');
    if input.is_empty() {
        return None;
    }
    match input.find(' ') {
        Some(i) => Some((&input[..i], &input[i..])),
        None => Some((input, "")),
    }
}

/// Legge il prossimo campo racchiuso fra parentesi quadre, `[…]`.
fn take_bracketed(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start_matches(' ');
    let body = input.strip_prefix('[')?;
    let end = body.find(']')?;
    Some((&body[..end], &body[end + 1..]))
}

/// Legge il prossimo campo racchiuso fra virgolette.
fn take_quoted(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start_matches(' ');
    let body = input.strip_prefix('"')?;
    let end = body.find('"')?;
    Some((&body[..end], &body[end + 1..]))
}

/// Valida e converte lo status code: esattamente tre cifre.
pub(crate) fn parse_status(field: &str) -> Option<u16> {
    if field.len() != 3 || !field.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    field.parse().ok()
}

/// Converte il tempo di nginx (`$time_local`) in UTC, così che righe con fusi
/// diversi restino confrontabili (§6).
pub(crate) fn parse_time_local(field: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_str(field, TIME_FORMAT)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// Scompone la richiesta fra virgolette in `(metodo, target)`.
///
/// Deve avere esattamente tre parti — `METODO TARGET HTTP/versione` — e la
/// terza deve dichiararsi HTTP. Una richiesta vuota (`"-"`, che nginx registra
/// quando il client chiude prima di inviarla) non è una richiesta osservata.
pub(crate) fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let mut parts = request.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !version.starts_with("HTTP/") || target.is_empty() || !is_http_method(method) {
        return None;
    }
    Some((method, target))
}

/// Un metodo HTTP è un *token*: non vuoto e composto di soli caratteri ammessi.
///
/// Non si impone che sia uno dei metodi noti né che sia maiuscolo: un metodo
/// inatteso è esattamente il genere di cosa che questo tool esiste per mostrare
/// (§P1).
pub(crate) fn is_http_method(method: &str) -> bool {
    !method.is_empty()
        && method
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || METHOD_EXTRA_CHARS.contains(&b))
}

/// Scompone la query string in chiavi e valori **grezzi** (§6).
///
/// Una chiave ripetuta conserva tutti i suoi valori; una chiave senza `=`
/// conserva un valore vuoto, perché "il parametro c'era" è un'informazione
/// diversa da "il parametro non c'era". Nessuna decodifica: i valori passeranno
/// dal `Redactor` (§P5) e la decodifica è una decisione di Fase 2.
pub(crate) fn parse_query(query: &str) -> BTreeMap<String, Vec<String>> {
    let mut params: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        params
            .entry(key.to_string())
            .or_default()
            .push(value.to_string());
    }
    params
}

/// Converte il tempo nella forma `$time_iso8601` di nginx
/// (`2023-10-10T13:55:36+00:00`).
///
/// Sta qui accanto a [`parse_time_local`] e non altrove perché le due sono la
/// **stessa decisione** — come si legge il tempo di una riga — presa su due
/// scritture diverse: separarle è il modo in cui due formati finiscono per
/// convertire i fusi in maniera diversa.
pub(crate) fn parse_time_iso8601(field: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(field)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// Dice se il campo dei byte del corpo ha una forma ammissibile: cifre, oppure
/// `-` quando nginx non ne ha scritto nessuno.
pub(crate) fn parse_body_bytes(field: &str) -> bool {
    field == "-" || (!field.is_empty() && field.bytes().all(|b| b.is_ascii_digit()))
}

/// Deduce l'autenticazione osservata da `$remote_user`, per **qualunque**
/// grammatica che porti quel campo.
///
/// È la regola di §6 in un posto solo: `-` significa che il log non dice nulla
/// (`NotObservable`, che non è "assente"), un valore significa che
/// un'autenticazione c'è stata, con schema **non specificato** perché quale
/// fosse sarebbe un'inferenza. Il valore dell'utente non entra mai nel modello.
pub(crate) fn remote_user_auth(remote_user: &str) -> ObservedAuth {
    // Vuoto **e** `-` significano la stessa cosa: il server non ha registrato
    // nessun utente. La grammatica preimpostata non può produrre un campo
    // vuoto — `take_token` non lo restituisce mai — ma una grammatica
    // dichiarata sì, e senza questo caso una colonna vuota diventerebbe
    // un'autenticazione osservata inventata dal nulla: il genere di dato che
    // sembra vero e abbassa la severità di uno `Shadow` (§P2).
    if remote_user == "-" || remote_user.is_empty() {
        ObservedAuth::NotObservable
    } else {
        ObservedAuth::Present {
            scheme: ObservedAuth::SCHEME_UNSPECIFIED.to_string(),
        }
    }
}
