//! Lettura di un file OpenAPI/Swagger in JSON.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use shadow_core::hex;
use shadow_core::ruleset::{MAX_SPEC_BYTES, MAX_SPEC_DEPTH};
use shadow_core::{DeclaredEndpoint, DeclaredInventory, DigestAlgorithm, InputDigest, InputRole};

/// Metodi HTTP riconosciuti come operazioni dentro un path item.
const OPERATION_KEYS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Esito della lettura di una specifica.
#[derive(Debug, Clone)]
pub struct SpecOutcome {
    /// Gli endpoint dichiarati (§5).
    pub inventory: DeclaredInventory,

    /// Digest SHA-256 del file, per il `RunManifest` (§6): un verdetto prodotto
    /// confrontando una specifica deve dire **quale** specifica.
    pub digest: InputDigest,
}

/// Perché una specifica non è stata letta.
#[derive(Debug)]
pub enum SpecError {
    /// Il file non è leggibile.
    Io {
        /// Percorso che si è tentato di leggere.
        path: PathBuf,
        /// Errore riportato dal sistema operativo.
        source: io::Error,
    },

    /// Il file non è né JSON né YAML leggibile.
    NotReadable {
        /// File esaminato.
        path: PathBuf,
        /// Dettaglio del parser.
        detail: String,
    },

    /// Il file supera uno dei limiti di §7.
    ///
    /// Una specifica è input non fidato quanto un log: qui si fallisce
    /// rumorosamente invece di lasciar decidere alla memoria disponibile.
    TooLarge {
        /// File esaminato.
        path: PathBuf,
        /// Cosa è stato superato.
        limit: &'static str,
        /// Il valore massimo ammesso.
        maximum: usize,
    },

    /// Il documento YAML usa alias, che sono il mattone delle bombe di
    /// espansione.
    YamlAliases {
        /// File esaminato.
        path: PathBuf,
        /// Riga del primo alias trovato.
        line_number: u64,
    },

    /// Il file è JSON valido ma non è una specifica: manca l'oggetto `paths`.
    NotASpec {
        /// File esaminato.
        path: PathBuf,
    },
}

impl fmt::Display for SpecError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5). Non mostrano mai
    /// il contenuto del file, che può contenere URL interni e nomi di clienti.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "cannot read the declared spec {}: {}", path.display(), source)
            }
            Self::NotReadable { path, detail } => write!(
                f,
                "{} is neither valid JSON nor valid YAML ({detail})",
                path.display()
            ),
            Self::TooLarge {
                path,
                limit,
                maximum,
            } => write!(
                f,
                "{} exceeds the maximum {limit} for a declared spec ({maximum}): refusing to read it.\
                 \n  a spec this large is either a mistake or a file built to make a parser fall over",
                path.display()
            ),
            Self::YamlAliases { path, line_number } => write!(
                f,
                "{} uses a YAML alias at line {line_number}, which this tool refuses to expand.\
                 \n  aliases are how YAML expansion bombs work: a few lines can expand into gigabytes.\
                 \n  in OpenAPI, reuse is expressed with $ref, not with YAML anchors",
                path.display()
            ),
            Self::NotASpec { path } => write!(
                f,
                "{} is valid JSON but has no 'paths' object, so it is not an OpenAPI or Swagger document",
                path.display()
            ),
        }
    }
}

/// Legge una specifica dichiarata e ne ricava il `DeclaredInventory`.
///
/// # Errori
///
/// Fallisce se il file non è leggibile, non è JSON, o non è una specifica.
/// Non esiste un esito parziale: metà inventario dichiarato produrrebbe finding
/// `Shadow` inventati (§P2).
pub fn read_spec(path: &Path) -> Result<SpecOutcome, SpecError> {
    let bytes = fs::read(path).map_err(|source| SpecError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.len() > MAX_SPEC_BYTES {
        return Err(SpecError::TooLarge {
            path: path.to_path_buf(),
            limit: "size in bytes",
            maximum: MAX_SPEC_BYTES,
        });
    }

    let document = parse_document(path, &bytes)?;

    let depth = depth_of(&document);
    if depth > MAX_SPEC_DEPTH {
        return Err(SpecError::TooLarge {
            path: path.to_path_buf(),
            limit: "nesting depth",
            maximum: MAX_SPEC_DEPTH,
        });
    }

    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| SpecError::NotASpec {
            path: path.to_path_buf(),
        })?;

    let bases = base_paths(&document);
    let global_security = security_schemes(document.get("security"));

    let mut endpoints = Vec::new();
    for (declared_path, item) in paths {
        let Some(item) = item.as_object() else { continue };

        let mut methods = BTreeSet::new();
        let mut schemes = global_security.clone();
        for (key, operation) in item {
            if !OPERATION_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                continue;
            }
            methods.insert(key.to_ascii_uppercase());
            schemes.extend(security_schemes(operation.get("security")));
        }
        if methods.is_empty() {
            continue;
        }

        for base in &bases {
            endpoints.push(DeclaredEndpoint {
                path_pattern: join(base, declared_path),
                methods: methods.clone(),
                declared_auth_schemes: schemes.clone(),
            });
        }
    }

    Ok(SpecOutcome {
        inventory: DeclaredInventory::from_endpoints(endpoints),
        digest: InputDigest {
            path: path.to_path_buf(),
            role: InputRole::OpenApiSpec,
            algorithm: DigestAlgorithm::Sha256,
            digest: hex::encode(&Sha256::digest(&bytes)),
        },
    })
}

/// I prefissi sotto cui la specifica dichiara di servire i propri path.
///
/// Swagger 2 ha un solo `basePath`; OpenAPI 3 ha una lista di `servers`, e ogni
/// server può avere un prefisso diverso. Si tengono **tutti**: sono tutti
/// dichiarati, e scartarne uno significherebbe non riconoscere nel traffico un
/// endpoint che la specifica descrive — cioè segnalarlo come `Shadow` quando non
/// lo è.
fn base_paths(document: &Value) -> Vec<String> {
    let mut bases: BTreeSet<String> = BTreeSet::new();

    if let Some(base) = document.get("basePath").and_then(Value::as_str) {
        bases.insert(normalise_base(base));
    }

    if let Some(servers) = document.get("servers").and_then(Value::as_array) {
        for server in servers {
            if let Some(url) = server.get("url").and_then(Value::as_str) {
                bases.insert(normalise_base(&path_component(url)));
            }
        }
    }

    if bases.is_empty() {
        bases.insert(String::new());
    }
    bases.into_iter().collect()
}

/// La parte di path di un URL di server, senza schema né host.
fn path_component(url: &str) -> String {
    let after_scheme = match url.find("://") {
        Some(index) => &url[index + 3..],
        None => return url.to_string(),
    };
    match after_scheme.find('/') {
        Some(index) => after_scheme[index..].to_string(),
        None => String::new(),
    }
}

fn normalise_base(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed == "/" || trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn join(base: &str, path: &str) -> String {
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// I nomi degli schemi di sicurezza citati in un blocco `security`.
///
/// Non si risolvono i riferimenti a `components.securitySchemes`: qui serve
/// sapere **se e quali** schemi la specifica dichiara su un endpoint, non come
/// funzionano.
fn security_schemes(security: Option<&Value>) -> BTreeSet<String> {
    let mut schemes = BTreeSet::new();
    let Some(requirements) = security.and_then(Value::as_array) else {
        return schemes;
    };
    for requirement in requirements {
        let Some(object) = requirement.as_object() else { continue };
        for name in object.keys() {
            schemes.insert(name.clone());
        }
    }
    schemes
}

/// Interpreta il documento: JSON se lo è, altrimenti YAML.
///
/// L'ordine conta poco — un JSON valido è anche YAML valido — ma provare prima
/// JSON tiene i messaggi d'errore più precisi per il caso più semplice.
fn parse_document(path: &Path, bytes: &[u8]) -> Result<Value, SpecError> {
    if let Ok(document) = serde_json::from_slice::<Value>(bytes) {
        return Ok(document);
    }

    // **Prima** di dare il file al parser YAML: gli alias sono il mattone con
    // cui si costruiscono le bombe di espansione, e quando il parser li ha
    // espansi la memoria è già finita. Si rifiuta guardando il testo, non il
    // risultato (§7).
    if let Some(line_number) = first_yaml_alias(bytes) {
        return Err(SpecError::YamlAliases {
            path: path.to_path_buf(),
            line_number,
        });
    }

    serde_norway::from_slice::<Value>(bytes).map_err(|error| SpecError::NotReadable {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })
}

/// Cerca il primo alias YAML (`*nome`) fuori dalle stringhe fra virgolette.
///
/// La scansione è volutamente grossolana e prudente: nel dubbio segnala, perché
/// il costo di un rifiuto è una domanda dell'utente e il costo di una bomba
/// espansa è il processo che muore (§P3).
fn first_yaml_alias(bytes: &[u8]) -> Option<u64> {
    let text = String::from_utf8_lossy(bytes);
    for (index, line) in text.lines().enumerate() {
        let mut in_single = false;
        let mut in_double = false;
        let chars: Vec<char> = line.chars().collect();
        for (position, ch) in chars.iter().enumerate() {
            match ch {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                '#' if !in_single && !in_double => break,
                '*' if !in_single && !in_double => {
                    // Un alias apre un valore: a inizio riga, dopo uno spazio,
                    // o dentro una lista o una mappa in forma compatta —
                    // `[*a,*a]` è la forma con cui le bombe si scrivono.
                    let opens_value = position == 0
                        || matches!(chars[position - 1], ' ' | '\t' | '[' | '{' | ',' | '-');
                    let names_something = chars
                        .get(position + 1)
                        .is_some_and(|next| next.is_alphanumeric() || *next == '_' || *next == '-');
                    if opens_value && names_something {
                        return Some(index as u64 + 1);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Profondità di annidamento del documento, calcolata **senza ricorsione**: un
/// documento profondo non deve poter esaurire lo stack mentre lo si misura.
fn depth_of(document: &Value) -> usize {
    let mut deepest = 0;
    let mut stack = vec![(document, 1_usize)];
    while let Some((value, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        match value {
            Value::Object(map) => stack.extend(map.values().map(|v| (v, depth + 1))),
            Value::Array(items) => stack.extend(items.iter().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    deepest
}
