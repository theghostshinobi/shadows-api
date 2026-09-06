//! Il `Redactor` — maschera segreti e dati personali prima di ogni stampa e di
//! ogni scrittura (§5, §P5).
//!
//! # Perché è acceso di default
//!
//! I log contengono token nelle URL, email nelle querystring, e ogni tanto
//! password mandate per errore via GET. Un report che riporta quei valori in
//! chiaro diventa **esso stesso una falla** nel momento in cui qualcuno lo
//! incolla in un ticket condiviso — che è precisamente ciò che si fa con un
//! report. Per questo la redazione non è un'opzione da accendere: è la
//! posizione di riposo, e `--show-raw-values` è la deroga esplicita di chi sa
//! cosa sta facendo.
//!
//! # Cosa maschera
//!
//! - **valori dopo un nome di campo sensibile**: `token=`, `password=`, `key=`…
//! - **forme strutturali di segreto**: JWT, stringhe esadecimali o base64 lunghe
//! - **indirizzi email**
//! - **percorsi di filesystem**: `/home/utente/clienti/bancaXYZ/logs/access.log`
//!   rivela il cliente e la struttura interna dell'organizzazione, e viaggia
//!   dritto nel file che si consegna all'auditor (§P5, esteso in v1.2)
//!
//! # Cosa **non** fa
//!
//! Non prova a decidere se una stringa è "davvero" un segreto. Nel dubbio
//! maschera: un valore mascherato per eccesso costa una domanda all'utente, un
//! segreto lasciato in chiaro costa un incidente (§P1, §P3).

use std::path::Path;

/// Con cosa viene sostituito un valore mascherato.
pub const REDACTED: &str = "<redacted>";

/// Con cosa viene sostituita la parte di percorso che rivela la struttura.
pub const REDACTED_PATH: &str = "<path>";

/// Nomi di campo il cui **valore** è considerato sensibile (§7).
const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "token", "key", "secret", "password", "passwd", "pwd", "auth", "session", "sig", "signature",
    "credential", "apikey", "access", "refresh",
];

/// Lunghezza oltre la quale una stringa opaca è trattata come possibile segreto.
const OPAQUE_SECRET_MIN_LEN: usize = 24;

/// Il `Redactor` (§5).
///
/// Ha uno stato solo: se mascherare o no. Esiste come tipo, invece che come
/// funzione libera, perché così **non si può stampare qualcosa senza aver
/// deciso** cosa fare della redazione — la scelta va presa una volta,
/// all'avvio, e poi attraversa tutto l'output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redactor {
    enabled: bool,
}

impl Redactor {
    /// Il `Redactor` acceso: la posizione di riposo (§P5).
    pub fn enabled() -> Self {
        Self { enabled: true }
    }

    /// Il `Redactor` spento, per `--show-raw-values`.
    ///
    /// Chi lo costruisce sta scegliendo di far uscire i valori in chiaro, e
    /// l'output deve dirlo a chi lo legge: un report senza redazione va marcato
    /// come tale, o chi lo riceve non ha modo di saperlo.
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// È acceso?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Maschera un testo destinato all'output: path, evidenza, valori di
    /// configurazione.
    pub fn text(&self, value: &str) -> String {
        if !self.enabled {
            return value.to_string();
        }
        value
            .split_inclusive(|c: char| c == ' ' || c == '\t')
            .map(|chunk| {
                let trailing = chunk.len() - chunk.trim_end().len();
                let (word, space) = chunk.split_at(chunk.len() - trailing);
                format!("{}{}", self.word(word), space)
            })
            .collect()
    }

    /// Maschera un percorso di filesystem tenendone il nome del file.
    ///
    /// Quale file sia analizzato è informazione d'audit legittima; **dove** stia
    /// nel filesystem racconta di clienti e organizzazione, e non serve a
    /// leggere il report.
    pub fn path(&self, path: &Path) -> String {
        if !self.enabled {
            return path.display().to_string();
        }
        // Il **nome** del file passa dalla stessa regola dei segmenti di path:
        // `clienti/mario.rossi@example.com.log` e
        // `session-aB3dEfgH7jKlmN9pQr1sT2uVwXyZ.log` finivano in chiaro nel
        // manifest, cioè nel file che viaggia (§P5), mentre le stesse identiche
        // stringhe dentro un path osservato erano mascherate. Quale file sia
        // stato analizzato resta informazione d'audit legittima: è il nome
        // ordinario a passare, non quello che è un segreto.
        match path.file_name() {
            Some(name) if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) => {
                format!("{REDACTED_PATH}/{}", self.file_name(&name.to_string_lossy()))
            }
            Some(name) => self.file_name(&name.to_string_lossy()),
            None => REDACTED_PATH.to_string(),
        }
    }

    /// Il nome di un file, mascherato se è **esso stesso** un segreto.
    ///
    /// Si guarda anche il nome senza estensione: `session-<token>.log` non è una
    /// stringa opaca solo perché il punto di `.log` la smentisce, ed era
    /// esattamente il modo in cui un token nel nome del file arrivava intatto
    /// nel manifest.
    fn file_name(&self, name: &str) -> String {
        let stem = name.rsplit_once('.').map(|(before, _)| before).unwrap_or(name);
        if is_secret_like(name) || is_secret_like(stem) {
            return REDACTED.to_string();
        }
        name.to_string()
    }

    /// Maschera una singola parola, che può essere un path, una coppia
    /// `chiave=valore`, o un valore opaco.
    ///
    /// **L'ordine dei due controlli è la sostanza, non lo stile.** Il path si
    /// spezza per primo. Quando il controllo `chiave=valore` girava sull'intera
    /// parola, un path che conteneva una parola sensibile la rendeva il "nome"
    /// della coppia, e il valore mascherato diventava ciò che stava dopo il
    /// primo `=` — cioè quasi niente. Su
    /// `/api/session/aB3dEfgH7jKl%2FmN9pQr1sT2uVwXy==` usciva
    /// `/api/session/aB3dEfgH7jKl%2FmN9pQr1sT2uVwXy=<redacted>`: il segreto in
    /// chiaro e la maschera sulla coda vuota. Il commento del ramo sotto
    /// prometteva già "segmento per segmento"; era il ramo sopra a non
    /// lasciarglielo fare.
    fn word(&self, word: &str) -> String {
        if word.is_empty() {
            return String::new();
        }
        // Un path si maschera segmento per segmento: la struttura del path è
        // parte del verdetto, i valori dentro no.
        if word.contains('/') {
            return word
                .split('/')
                .map(|segment| self.pair_or_segment(segment))
                .collect::<Vec<String>>()
                .join("/");
        }
        self.pair_or_segment(word)
    }

    /// Una coppia `chiave=valore` con un nome sensibile, oppure un valore
    /// opaco.
    ///
    /// Della coppia si maschera il valore e si tiene la chiave, perché sapere
    /// *che* c'era un token è informazione utile e sapere quale è una falla.
    fn pair_or_segment(&self, part: &str) -> String {
        if let Some((name, value)) = part.split_once('=') {
            if !value.is_empty() && is_sensitive_name(name) {
                return format!("{name}={REDACTED}");
            }
        }
        self.segment(part)
    }

    fn segment(&self, segment: &str) -> String {
        if is_secret_like(segment) {
            REDACTED.to_string()
        } else {
            segment.to_string()
        }
    }
}

fn is_sensitive_name(name: &str) -> bool {
    let normalised: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    SENSITIVE_FIELD_NAMES
        .iter()
        .any(|sensitive| normalised.contains(sensitive))
}

/// Un indirizzo email: `qualcosa@dominio.tld`.
fn looks_like_email(value: &str) -> bool {
    match value.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
        None => false,
    }
}

/// Una forma strutturale di segreto: JWT, o stringa opaca lunga.
fn looks_like_secret(value: &str) -> bool {
    if is_jwt(value) {
        return true;
    }
    if value.len() < OPAQUE_SECRET_MIN_LEN {
        return false;
    }
    // Una stringa lunga fatta solo di caratteri da token, con almeno una cifra:
    // le parole del vocabolario non hanno cifre, gli identificatori opachi sì.
    let token_chars = value.bytes().all(is_token_byte);
    token_chars && value.bytes().any(|b| b.is_ascii_digit())
}

/// I byte che possono comparire dentro una stringa opaca senza smentirla.
///
/// **`%` non sta in questo elenco, e la ragione è misurata.** Metterci il
/// percento sembrava chiudere il buco: la normalizzazione ri-codifica in `%XX`
/// barre, graffe e caratteri di controllo dentro ogni segmento canonico
/// (`inventory::path::reencode`), quindi un `%2F` scritto dal tool stesso
/// impediva di riconoscere il segreto che lo conteneva. Ma ogni escape porta
/// **per costruzione** una cifra, e la cifra è metà della prova che questa
/// funzione richiede: con `%` nell'alfabeto, qualunque segmento lungo che
/// contenga un escape diventa "segreto". Su un corpus realistico — path GitLab,
/// dove il progetto è un parametro con la barra codificata
/// (`/api/v4/projects/gruppo%2Fprogetto/...`) — il mascheramento è passato dallo
/// **0% al 91,2%**, ben oltre la soglia del 15% di COLLAUDO.md, e 186 pattern
/// distinti collassavano su sei stringhe identiche. Un report illeggibile è
/// inutile quanto uno che perde segreti.
///
/// La contraddizione si chiude invece dove nasce: vedi [`is_secret_like`].
fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'/' | b'=')
}

/// Punteggiatura che può stare attaccata a un valore dentro una frase.
///
/// `Redactor::text` spezza in parole solo su spazi e tabulazioni, quindi un path
/// citato dentro l'evidenza di un finding arriva qui con la virgola della frase
/// ancora attaccata — e la virgola, non essendo un byte da token, smentiva la
/// stringa. Lo stesso identico path usciva `<redacted>` nella colonna del
/// soggetto e **in chiaro due caratteri dopo**, sulla stessa riga.
///
/// `=` non è in questo elenco: è il riempimento del base64, fa parte del valore.
const TRAILING_PUNCTUATION: &[char] = &[',', ';', ':', '.', '!', '?', '(', ')', '[', ']', '"', '\''];

/// Dice se un segmento va mascherato, guardando anche **dentro** gli escape
/// percentuali che la normalizzazione ha prodotto.
///
/// # Perché non basta guardare la stringa intera
///
/// `reencode` scrive `%XX` per barre, graffe, percento e caratteri di controllo.
/// Un escape in mezzo a un segmento faceva due danni opposti: se `%` non era
/// nell'alfabeto, **incollava** due parti in una stringa che nessuna regola
/// poteva più riconoscere; se `%` c'era, **regalava** la cifra che serviva a
/// dichiarare segreto qualunque path un po' lungo.
///
/// La via d'uscita è trattare l'escape per quello che è — un separatore che il
/// tool ha codificato — e giudicare **ogni parte per conto suo**, esattamente
/// come `word` fa con una barra vera. Così un segreto lungo resta riconoscibile
/// anche con un escape dentro, e `acme-platform%2Fbilling-service` resta
/// leggibile perché nessuna delle sue due metà è un blob opaco.
fn is_secret_like(segment: &str) -> bool {
    let value = segment.trim_matches(|c| TRAILING_PUNCTUATION.contains(&c));
    if looks_like_secret(value) || looks_like_email(value) {
        return true;
    }
    split_on_escapes(value).any(|part| looks_like_secret(part) || looks_like_email(part))
}

/// Spezza una stringa sugli escape percentuali `%XX` che contiene.
///
/// Restituisce le parti fra un escape e il successivo. Se non ci sono escape la
/// stringa esce intera, e chi chiama l'ha già esaminata: nessun lavoro doppio
/// che cambi l'esito.
fn split_on_escapes(value: &str) -> impl Iterator<Item = &str> {
    let bytes = value.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            parts.push(&value[start..i]);
            i += 3;
            start = i;
        } else {
            i += 1;
        }
    }
    parts.push(&value[start..]);
    parts.into_iter()
}

/// Tre parti separate da punti, come `header.payload.signature`.
fn is_jwt(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            part.len() >= 4
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}
