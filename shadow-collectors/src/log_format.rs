//! Grammatica di log **dichiarata dall'utente**, nella sintassi con cui nginx
//! la scrive nel proprio `log_format` (§7, chiave `log_format`).
//!
//! # Perché esiste
//!
//! Il collaudo su corpus reale (COLLAUDO.md, esecuzione del 2026-08-30) ha
//! misurato che **una variante di `log_format` su sette** viene accettata: solo
//! il `combined` nudo. Le altre sei sono configurazioni ordinarie —
//! `$request_time`, `$upstream_response_time`, `$http_x_forwarded_for`, il
//! default di ingress-nginx, il `common` di Apache — e su un'installazione
//! reale almeno una c'è quasi sempre. Rifiutarle è il comportamento corretto
//! secondo §P2, e allo stesso tempo rende il tool inservibile dove serve.
//!
//! La via d'uscita che COLLAUDO.md indica non è allentare la grammatica: è
//! **dichiararla**. L'utente incolla la propria direttiva `log_format` e Shadow
//! la compila in una grammatica altrettanto stretta di quella preimpostata.
//!
//! # Perché questo non allenta la validazione (§7)
//!
//! §7 dice che `log_format` può avere un valore preimpostato **solo perché la
//! validazione è stretta**, e che se la validazione si allentasse il default
//! andrebbe rimosso nello stesso momento. Qui la validazione **non** si allenta:
//!
//! - senza dichiarazione, l'unica grammatica accettata resta il `combined`, e
//!   un file che non combacia esce con `1` e stdout vuoto, esattamente come
//!   prima;
//! - con una dichiarazione, la grammatica dichiarata è percorsa per intero e
//!   ogni campo è letto dalla posizione che la dichiarazione gli assegna: una
//!   riga che non combacia viene scartata, non interpretata a caso.
//!
//! Non esiste, né prima né dopo, un percorso in cui un campo viene letto dalla
//! posizione sbagliata. Il default sopravvive perché la condizione che lo
//! giustificava non è cambiata.
//!
//! # Cosa **non** è
//!
//! # Il limite che resta, detto qui perché chi dichiara lo sappia
//!
//! Un valore che contiene il **carattere delimitatore** sposta la lettura di
//! tutti i campi successivi. In un formato a barre verticali, una riga il cui
//! URI contiene `|` viene tagliata nel posto sbagliato, e se i campi spostati
//! risultano per caso ben formati la riga viene accettata.
//!
//! Non è aggirabile, ed è il motivo per cui non è stato «corretto»: un formato
//! posizionale non può rappresentare il proprio separatore dentro un valore, e
//! nginx protegge solo `"` e `\` (`escape=default`). Cercare il delimitatore
//! «giusto» più avanti significherebbe **indovinare** dove il campo finisce,
//! che è precisamente ciò che §P2 vieta. Il limite vale identico per la
//! grammatica preimpostata, dove un user agent con virgolette non protette
//! produce lo stesso effetto.
//!
//! # Non è un secondo `Collector`
//!
//! Non lo è nel senso di §3 (*«quale formato aggiungere per
//! secondo»*, decisione aperta): è la grammatica dello stesso Collector, resa
//! dichiarabile. Che il `common` e il `combined` di Apache risultino
//! esprimibili in questa sintassi è una conseguenza — sono gli stessi campi
//! nello stesso ordine, come COLLAUDO.md già annota — non la scelta di un
//! secondo formato.

use std::fmt;

use shadow_core::{ObservedRequest, SourceRef};

use crate::collector::{Collector, DiscardReason};
use crate::nginx::{
    parse_body_bytes, parse_query, parse_request_line, parse_status, parse_time_iso8601,
    parse_time_local, remote_user_auth,
};

/// Nome canonico con cui una grammatica dichiarata compare nei messaggi e nel
/// manifest. Il **valore** della dichiarazione è già registrato dal manifest
/// come valore della chiave `log_format` (§6, v1.6): qui serve solo un'etichetta
/// stabile che dica *di che genere* di formato si tratta.
pub const DECLARED_FORMAT_NAME: &str = "nginx-declared";

/// Variabile che porta la riga di richiesta completa, `"METODO PATH HTTP/x"`.
const VAR_REQUEST: &str = "request";
/// Variabili che portano il metodo e il target separatamente.
const VAR_REQUEST_METHOD: &str = "request_method";
const VAR_REQUEST_URI: &str = "request_uri";
const VAR_URI: &str = "uri";
/// Variabile dello status HTTP.
const VAR_STATUS: &str = "status";
/// Variabili del tempo, nelle due forme che nginx sa scrivere.
const VAR_TIME_LOCAL: &str = "time_local";
const VAR_TIME_ISO8601: &str = "time_iso8601";
/// Variabile dell'utente autenticato registrato dal server.
const VAR_REMOTE_USER: &str = "remote_user";
/// Variabile dei byte del corpo: se c'è, si valida come prima.
const VAR_BODY_BYTES_SENT: &str = "body_bytes_sent";

/// Un pezzo della grammatica dichiarata.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// Testo fisso, che la riga deve contenere in quella posizione.
    ///
    /// Un pezzo fatto di **soli spazi** combacia con uno o più spazi: è la
    /// stessa tolleranza della grammatica preimpostata, che separa i campi con
    /// `trim_start_matches(' ')`, e riguarda il separatore, non il contenuto di
    /// un campo.
    Literal(String),
    /// Una variabile `$nome`, il cui valore va letto fino al prossimo pezzo
    /// fisso (o fino a fine riga, se è l'ultima).
    Variable(String),
}

/// Perché una dichiarazione di `log_format` non è utilizzabile.
///
/// Sono errori d'uso, scoperti **prima** di leggere una sola riga: una
/// grammatica ambigua o incompleta non deve nemmeno arrivare al file (§P2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogFormatError {
    /// La dichiarazione è vuota.
    Empty,

    /// La dichiarazione contiene sequenze di escape con la barra rovesciata.
    ///
    /// Succede copiando la direttiva da un `nginx.conf` dove le virgolette sono
    /// protette. Interpretarle sarebbe indovinare; ignorarle produrrebbe una
    /// grammatica che non combacia mai, e l'utente non capirebbe perché. Quindi
    /// si dice esattamente cosa fare (§P2).
    EscapeSequence,

    /// Un `$` senza nome dopo, o `${}` vuoto.
    EmptyVariableName {
        /// Posizione in caratteri, per indicare dove guardare.
        at: usize,
    },

    /// Un `${nome` a cui manca la graffa di chiusura.
    ///
    /// Ha un errore suo e non si confonde con [`Self::EmptyVariableName`]: dire
    /// «un `$` senza nome» a chi il nome l'ha scritto manda a cercare il
    /// problema dove non è.
    UnclosedBrace {
        /// Posizione in caratteri del `$` che apre la variabile.
        at: usize,
        /// Il nome letto fin lì, per farlo riconoscere all'utente.
        name: String,
    },

    /// Due variabili adiacenti senza niente in mezzo: `$a$b`.
    ///
    /// È il caso in cui la grammatica non ha modo di sapere dove finisce la
    /// prima e comincia la seconda. Indovinare qui significherebbe leggere
    /// campi dalla posizione sbagliata, cioè il difetto che §P2 vieta.
    AdjacentVariables {
        /// Nome della prima delle due.
        first: String,
        /// Nome della seconda.
        second: String,
    },

    /// Mancano variabili senza le quali non si può costruire una richiesta
    /// osservata.
    MissingRequired {
        /// Cosa manca, già in forma leggibile.
        missing: Vec<String>,
    },
}

impl fmt::Display for LogFormatError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the declared log_format is empty"),
            Self::EscapeSequence => write!(
                f,
                "the declared log_format contains backslash escapes; pass it as it appears in the \
                 log line, with real quotes (write \" where nginx.conf has \\\")"
            ),
            Self::EmptyVariableName { at } => write!(
                f,
                "the declared log_format has a '$' without a variable name at character {at}"
            ),
            Self::UnclosedBrace { at, name } => write!(
                f,
                "the declared log_format opens ${{{name} at character {at} but never closes the \
                 brace; write ${{{name}}} or just ${name}"
            ),
            Self::AdjacentVariables { first, second } => write!(
                f,
                "the declared log_format puts ${first} immediately before ${second} with nothing \
                 in between, so there is no way to tell where one ends and the next begins; \
                 separate them the way the log line does"
            ),
            Self::MissingRequired { missing } => write!(
                f,
                "the declared log_format cannot describe a request: {}",
                missing.join("; ")
            ),
        }
    }
}

/// Dove, nella sequenza dei pezzi, si trova ciascun campo che serve.
#[derive(Debug, Clone, Default)]
struct Fields {
    request: Option<usize>,
    request_method: Option<usize>,
    request_uri: Option<usize>,
    status: Option<usize>,
    time_local: Option<usize>,
    time_iso8601: Option<usize>,
    remote_user: Option<usize>,
    body_bytes_sent: Option<usize>,
}

/// Una grammatica di log dichiarata dall'utente e compilata.
#[derive(Debug, Clone)]
pub struct DeclaredLogFormat {
    pieces: Vec<Piece>,
    fields: Fields,
    shape: String,
}

impl DeclaredLogFormat {
    /// Compila una dichiarazione, o dice **perché** non è utilizzabile.
    ///
    /// Tutte le verifiche avvengono qui, prima di aprire il file: una
    /// grammatica ambigua deve fermare il run all'avvio, non a metà lettura.
    pub fn parse(spec: &str) -> Result<Self, LogFormatError> {
        if spec.trim().is_empty() {
            return Err(LogFormatError::Empty);
        }
        if spec.contains('\\') {
            return Err(LogFormatError::EscapeSequence);
        }

        // Gli spazi in coda alla dichiarazione si tolgono, perché in coda alla
        // **riga** non ce ne sono: `ingest` la consegna già senza terminatore e
        // il Collector la taglia con `trim_end`. Una dichiarazione che finisse
        // con uno spazio non potrebbe combaciare con nessuna riga, mai, e
        // l'utente si vedrebbe rifiutare l'intero file senza capire perché.
        // Non è una tolleranza sui dati: è la stessa regola applicata ai due
        // lati del confronto.
        let pieces = tokenise(spec.trim_end())?;
        reject_adjacent_variables(&pieces)?;

        let mut fields = Fields::default();
        for (index, piece) in pieces.iter().enumerate() {
            let Piece::Variable(name) = piece else {
                continue;
            };
            // Una variabile ripetuta: vince la prima occorrenza, e la scelta è
            // deterministica (§P4). Un `log_format` che ripete lo stesso campo
            // non è una configurazione sensata, ma non è motivo per rifiutare.
            let slot = match name.as_str() {
                VAR_REQUEST => &mut fields.request,
                VAR_REQUEST_METHOD => &mut fields.request_method,
                VAR_REQUEST_URI | VAR_URI => &mut fields.request_uri,
                VAR_STATUS => &mut fields.status,
                VAR_TIME_LOCAL => &mut fields.time_local,
                VAR_TIME_ISO8601 => &mut fields.time_iso8601,
                VAR_REMOTE_USER => &mut fields.remote_user,
                VAR_BODY_BYTES_SENT => &mut fields.body_bytes_sent,
                // Ogni altra variabile viene letta e scartata: è esattamente
                // ciò che permette di accettare i nove campi in più del
                // default di ingress-nginx senza sapere cosa siano.
                _ => continue,
            };
            slot.get_or_insert(index);
        }

        check_required(&fields)?;
        let shape = shape_of(&pieces);
        Ok(Self {
            pieces,
            fields,
            shape,
        })
    }
}

/// Cosa manca perché la dichiarazione descriva una richiesta.
///
/// L'elenco è completo in un colpo solo: chi ha sbagliato due variabili non
/// deve scoprirlo in due esecuzioni.
fn check_required(fields: &Fields) -> Result<(), LogFormatError> {
    let mut missing = Vec::new();
    if fields.request.is_none() && !(fields.request_method.is_some() && fields.request_uri.is_some())
    {
        missing.push(
            "it needs $request, or both $request_method and $request_uri, to know which endpoint \
             was called"
                .to_string(),
        );
    }
    if fields.status.is_none() {
        missing.push("it needs $status to know how the server answered".to_string());
    }
    if fields.time_local.is_none() && fields.time_iso8601.is_none() {
        missing.push(
            "it needs $time_local or $time_iso8601 to know when the request happened".to_string(),
        );
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(LogFormatError::MissingRequired { missing })
    }
}

/// Scompone la dichiarazione in pezzi fissi e variabili.
fn tokenise(spec: &str) -> Result<Vec<Piece>, LogFormatError> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '$' {
            literal.push(chars[i]);
            i += 1;
            continue;
        }

        let start = i;
        i += 1;
        // nginx ammette sia `$nome` sia `${nome}`: la seconda serve quando al
        // nome segue subito un carattere che potrebbe farne parte.
        let braced = i < chars.len() && chars[i] == '{';
        if braced {
            i += 1;
        }
        let name_start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        let name: String = chars[name_start..i].iter().collect();
        if braced {
            if i >= chars.len() || chars[i] != '}' {
                return Err(if name.is_empty() {
                    LogFormatError::EmptyVariableName { at: start }
                } else {
                    LogFormatError::UnclosedBrace { at: start, name }
                });
            }
            i += 1;
        }
        if name.is_empty() {
            return Err(LogFormatError::EmptyVariableName { at: start });
        }

        if !literal.is_empty() {
            pieces.push(Piece::Literal(std::mem::take(&mut literal)));
        }
        pieces.push(Piece::Variable(name));
    }

    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Ok(pieces)
}

/// Rifiuta due variabili senza separatore: la grammatica sarebbe ambigua.
fn reject_adjacent_variables(pieces: &[Piece]) -> Result<(), LogFormatError> {
    for window in pieces.windows(2) {
        if let [Piece::Variable(first), Piece::Variable(second)] = window {
            return Err(LogFormatError::AdjacentVariables {
                first: first.clone(),
                second: second.clone(),
            });
        }
    }
    Ok(())
}

/// La forma attesa da una riga, ricostruita dalla dichiarazione, per i messaggi
/// d'errore. È testo rivolto all'utente (§5).
fn shape_of(pieces: &[Piece]) -> String {
    let mut shape = String::new();
    for piece in pieces {
        match piece {
            Piece::Literal(text) => shape.push_str(text),
            Piece::Variable(name) => {
                shape.push('<');
                shape.push_str(name);
                shape.push('>');
            }
        }
    }
    shape
}

impl Collector for DeclaredLogFormat {
    fn format_name(&self) -> &str {
        DECLARED_FORMAT_NAME
    }

    fn expected_shape(&self) -> &str {
        &self.shape
    }

    fn parse_line(&self, line: &str, source: SourceRef) -> Result<ObservedRequest, DiscardReason> {
        let captured = self.capture(line.trim_end())?;

        let status_code =
            parse_status(&captured[self.fields.status.expect("verificato in parse")])
                .ok_or(DiscardReason::InvalidStatusCode)?;

        let timestamp = match (self.fields.time_local, self.fields.time_iso8601) {
            (Some(index), _) => parse_time_local(&captured[index]),
            (None, Some(index)) => parse_time_iso8601(&captured[index]),
            (None, None) => unreachable!("verificato in parse"),
        }
        .ok_or(DiscardReason::InvalidTimestamp)?;

        let (method, target) = match self.fields.request {
            Some(index) => {
                let (method, target) =
                    parse_request_line(&captured[index]).ok_or(DiscardReason::InvalidRequestLine)?;
                (method.to_string(), target.to_string())
            }
            None => {
                let method = captured[self.fields.request_method.expect("verificato in parse")]
                    .clone();
                let target =
                    captured[self.fields.request_uri.expect("verificato in parse")].clone();
                // `-` è ciò che nginx scrive quando il valore non c'è: quando
                // il client chiude prima di mandare la richiesta, metodo e
                // target sono entrambi `-`. Non è una richiesta osservata, ed è
                // la stessa regola che `parse_request_line` applica al ramo
                // `$request` — dove `"-"` non ha tre parti e cade da sé. Qui
                // va detta, perché `-` è un carattere ammesso in un metodo HTTP
                // e passerebbe: diventerebbe un endpoint `/-` che non è mai
                // esistito, e con una specifica a fianco uno `Shadow` che fa
                // fallire una pipeline.
                if method == "-" || target == "-" {
                    return Err(DiscardReason::InvalidRequestLine);
                }
                if !crate::nginx::is_http_method(&method) || target.is_empty() {
                    return Err(DiscardReason::InvalidRequestLine);
                }
                (method, target)
            }
        };

        if let Some(index) = self.fields.body_bytes_sent {
            if !parse_body_bytes(&captured[index]) {
                return Err(DiscardReason::MalformedLine);
            }
        }

        let (raw_path, query) = match target.split_once('?') {
            Some((path, query)) => (path.to_string(), query.to_string()),
            None => (target, String::new()),
        };

        // Se la dichiarazione non porta `$remote_user`, dell'autenticazione il
        // log non dice **niente**: `NotObservable`, che non è "assente" (§6).
        // È il caso corretto per definizione, non una rinuncia.
        let auth = match self.fields.remote_user {
            Some(index) => remote_user_auth(&captured[index]),
            None => shadow_core::ObservedAuth::NotObservable,
        };

        Ok(ObservedRequest {
            method,
            raw_path,
            query_params: parse_query(&query),
            auth,
            status_code,
            timestamp,
            source,
        })
    }
}

impl DeclaredLogFormat {
    /// Percorre la riga secondo la grammatica, restituendo il valore di ogni
    /// pezzo (vuoto per i pezzi fissi, così che gli indici combacino).
    ///
    /// O la riga combacia per intero, o cade (§P2): non esiste il ramo che salta
    /// un pezzo che non torna.
    fn capture(&self, line: &str) -> Result<Vec<String>, DiscardReason> {
        let mut values = vec![String::new(); self.pieces.len()];
        let mut rest = line;

        for (index, piece) in self.pieces.iter().enumerate() {
            match piece {
                Piece::Literal(text) => {
                    rest = match_literal(rest, text).ok_or(DiscardReason::MalformedLine)?;
                }
                Piece::Variable(_) => {
                    // Il valore finisce dove comincia il prossimo pezzo fisso.
                    let terminator = match self.pieces.get(index + 1) {
                        Some(Piece::Literal(text)) => text.chars().next(),
                        // Due variabili adiacenti sono già state rifiutate in
                        // compilazione, quindi qui può esserci solo la fine.
                        _ => None,
                    };
                    let (value, remaining) = take_until(rest, terminator);

                    // **L'ancoraggio di coda.** Una variabile che chiude la
                    // grammatica non ha un delimitatore che la fermi, quindi
                    // senza questo controllo si mangerebbe qualunque campo la
                    // dichiarazione non nomina — e la riga verrebbe accettata,
                    // con un path che porta dentro il tempo di risposta e un
                    // `$remote_user` che diventa un'autenticazione inventata.
                    // È il «pericolo mortale» di §9 Fase 1: estrarre
                    // silenziosamente i campi dalla posizione sbagliata.
                    //
                    // La difesa è la stessa semantica della grammatica
                    // preimpostata: un campo **non racchiuso fra delimitatori**
                    // è separato dagli spazi, quindi non ne contiene. Un campo
                    // racchiuso — `[$time_local]`, `"$request"` — può
                    // contenerne quanti ne vuole, ed è per questo che la
                    // condizione guarda il delimitatore, non il nome.
                    if terminator.is_none_or(|stop| stop == ' ') && value.contains(' ') {
                        return Err(DiscardReason::MalformedLine);
                    }

                    values[index] = value.to_string();
                    rest = remaining;
                }
            }
        }

        if !rest.is_empty() {
            return Err(DiscardReason::MalformedLine);
        }
        Ok(values)
    }
}

/// Consuma un pezzo fisso dall'inizio di `input`.
///
/// Un pezzo di soli spazi combacia con **uno o più** spazi: è il separatore fra
/// campi, ed è la stessa tolleranza della grammatica preimpostata. Ogni altro
/// pezzo deve combaciare carattere per carattere.
fn match_literal<'a>(input: &'a str, text: &str) -> Option<&'a str> {
    if !text.is_empty() && text.chars().all(|c| c == ' ') {
        let trimmed = input.trim_start_matches(' ');
        if trimmed.len() == input.len() {
            return None;
        }
        return Some(trimmed);
    }
    input.strip_prefix(text)
}

/// Legge il valore di una variabile fino al carattere che apre il pezzo
/// successivo, o fino a fine riga.
fn take_until(input: &str, terminator: Option<char>) -> (&str, &str) {
    match terminator {
        Some(stop) => match input.find(stop) {
            Some(i) => (&input[..i], &input[i..]),
            None => (input, ""),
        },
        None => (input, ""),
    }
}
