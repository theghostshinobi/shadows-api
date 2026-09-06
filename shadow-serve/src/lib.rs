//! `shadow-serve` — **la sola cosa di questo progetto che apre un socket**.
//!
//! # Perché sta da sola
//!
//! §P6 dice che Shadow non apre connessioni di rete non richieste, e la
//! decisione 36 del fondatore aveva scelto l'alert senza egress proprio perché
//! la frase *«questo binario non apre socket»* restasse verificabile
//! dall'esterno. Il fondatore ha poi chiesto anche il server, e quella frase
//! cambia. Perché cambi nel modo meno peggiore, tutta la rete del progetto sta
//! **in un file solo**, e la frase diventa:
//!
//! > l'unica rete di Shadow è qui dentro, e si legge tutta in una volta.
//!
//! Le tre difese che ne discendono, in ordine di importanza:
//!
//! 1. **Non si apre niente se non lo si chiede.** Nessun'altra parte del
//!    progetto usa questo crate: solo il sottocomando `shadow serve`, che è una
//!    richiesta esplicita dell'utente.
//! 2. **Si ascolta solo dove l'utente dice**, e il preimpostato è `127.0.0.1`.
//!    Legarsi a un indirizzo raggiungibile dalla rete è una scelta che va
//!    scritta, e il server la dice a voce alta quando succede.
//! 3. **È in sola lettura.** Non c'è nessun percorso che scriva nello storico:
//!    prendere in carico un alert resta una cosa che si fa dal terminale. Un
//!    server che scrive è una superficie di scrittura remota su uno strumento
//!    offline, ed è una decisione diversa da quella che è stata presa.
//!
//! # Nessuna dipendenza di rete
//!
//! Il server è scritto sulla libreria standard. Per servire una pagina non
//! serve un framework, e su uno strumento di sicurezza ogni dipendenza è una
//! catena di fornitura in più da difendere.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use shadow_history::{HistoryStore, Target};
use shadow_view::Status;

/// Quanto si aspetta una richiesta prima di chiudere la connessione.
///
/// Una connessione che resta aperta senza dire niente è il modo più economico
/// di occupare un server. Cinque secondi bastano a un browser e non bastano a
/// tenere impegnato niente.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Quanti byte al massimo si leggono da una richiesta.
///
/// Serviamo una pagina e non accettiamo corpi: tutto ciò che eccede questa
/// soglia non è una richiesta che ci riguarda.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// Perché il server non parte, o non prosegue.
#[derive(Debug)]
pub enum ServeError {
    /// L'indirizzo non è utilizzabile.
    Address {
        /// Ciò che l'utente ha scritto.
        given: String,
        /// Il motivo riportato dal sistema.
        source: std::io::Error,
    },
    /// Non si riesce a mettersi in ascolto.
    Listen {
        /// Indirizzo su cui si è provato.
        address: String,
        /// Il motivo riportato dal sistema.
        source: std::io::Error,
    },
}

impl fmt::Display for ServeError {
    /// Messaggi in inglese: sono testo rivolto all'utente (§5).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address { given, source } => write!(
                f,
                "'{given}' is not an address Shadow can listen on: {source}.\n  \
                 Give it as <host>:<port>, for example 127.0.0.1:8787"
            ),
            Self::Listen { address, source } => write!(
                f,
                "cannot listen on {address}: {source}.\n  \
                 Another process may already be using that port"
            ),
        }
    }
}

/// Un server in sola lettura che mostra la vista di supervisione.
pub struct Server {
    listener: TcpListener,
    address: String,
}

impl Server {
    /// Si mette in ascolto, o dice perché non può.
    ///
    /// **Non risolve nomi che portino fuori dalla macchina senza dirlo:**
    /// l'indirizzo effettivo su cui si è legato viene restituito da
    /// [`Server::address`], e chi chiama deve mostrarlo. Legarsi a `0.0.0.0`
    /// perché così c'era scritto, e non accorgersene, è il genere di cosa che
    /// su uno strumento di sicurezza non deve poter succedere in silenzio.
    pub fn bind(address: &str) -> Result<Self, ServeError> {
        let resolved = address
            .to_socket_addrs()
            .map_err(|source| ServeError::Address {
                given: address.to_string(),
                source,
            })?
            .next()
            .ok_or_else(|| ServeError::Address {
                given: address.to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "the address resolved to nothing",
                ),
            })?;

        let listener = TcpListener::bind(resolved).map_err(|source| ServeError::Listen {
            address: resolved.to_string(),
            source,
        })?;
        Ok(Self {
            address: listener
                .local_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|_| resolved.to_string()),
            listener,
        })
    }

    /// L'indirizzo su cui si sta **davvero** ascoltando.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Dice se si sta ascoltando su un indirizzo raggiungibile da fuori.
    ///
    /// Serve a chi chiama per avvisare: legarsi oltre la macchina locale è una
    /// scelta legittima e va detta, non dedotta.
    pub fn is_local_only(&self) -> bool {
        self.listener
            .local_addr()
            .map(|addr| addr.ip().is_loopback())
            .unwrap_or(false)
    }

    /// Serve finché non lo si ferma, o per `limit` richieste se lo si dice.
    ///
    /// Il limite esiste per poter **verificare** il server senza lasciarlo
    /// acceso: un server che si può solo avviare non si può mettere sotto test.
    pub fn serve(
        &self,
        store: &HistoryStore,
        target: &Target,
        refresh: Option<u32>,
        limit: Option<u64>,
    ) -> std::io::Result<u64> {
        let mut served = 0_u64;
        for stream in self.listener.incoming() {
            let mut stream = stream?;
            // Una connessione che sbaglia non ferma il server: chiude e si va
            // avanti. Un cruscotto che cade perché qualcuno ha parlato male
            // sulla porta non è un cruscotto.
            let _ = self.handle(&mut stream, store, target, refresh);
            served += 1;
            if limit.is_some_and(|limit| served >= limit) {
                break;
            }
        }
        Ok(served)
    }

    fn handle(
        &self,
        stream: &mut TcpStream,
        store: &HistoryStore,
        target: &Target,
        refresh: Option<u32>,
    ) -> std::io::Result<()> {
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(READ_TIMEOUT))?;

        let request = read_request_line(stream)?;
        let (method, path) = parse_request_line(&request);

        // In sola lettura: qualunque cosa non sia una lettura viene rifiutata
        // prima di guardare il percorso.
        if method != "GET" && method != "HEAD" {
            return respond(
                stream,
                405,
                "text/plain; charset=utf-8",
                b"Shadow serves a read-only view. Only GET is accepted.\n",
                None,
            );
        }

        match path {
            "/" | "/index.html" => {
                let status = match Status::read(store, target) {
                    Ok(status) => status,
                    Err(e) => {
                        return respond(
                            stream,
                            500,
                            "text/plain; charset=utf-8",
                            format!("cannot read the history: {e}\n").as_bytes(),
                            None,
                        )
                    }
                };
                let page = shadow_view::page(&status, refresh);
                respond(stream, 200, "text/html; charset=utf-8", page.as_bytes(), None)
            }
            "/status.json" => {
                let status = match Status::read(store, target) {
                    Ok(status) => status,
                    Err(e) => {
                        return respond(
                            stream,
                            500,
                            "text/plain; charset=utf-8",
                            format!("cannot read the history: {e}\n").as_bytes(),
                            None,
                        )
                    }
                };
                let mut body = Vec::new();
                shadow_view::json(&mut body, &status);
                respond(stream, 200, "application/json; charset=utf-8", &body, None)
            }
            _ => respond(
                stream,
                404,
                "text/plain; charset=utf-8",
                b"Shadow serves / and /status.json\n",
                None,
            ),
        }
    }
}

/// Legge la prima riga della richiesta, e non più di [`MAX_REQUEST_BYTES`].
fn read_request_line(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut reader = BufReader::new(stream.try_clone()?).take(MAX_REQUEST_BYTES as u64);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

/// Scompone `GET /percorso HTTP/1.1`.
///
/// Il percorso viene troncato alla query: serviamo due percorsi fissi, e
/// nessuna parte di ciò che il client scrive raggiunge mai il filesystem o la
/// pagina. È la difesa più semplice contro l'attraversamento dei percorsi:
/// **non c'è nessun percorso da attraversare**.
fn parse_request_line(line: &str) -> (&str, &str) {
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split(['?', '#']).next().unwrap_or("/");
    (method, path)
}

/// Scrive una risposta.
///
/// Le intestazioni non sono decorazione: `X-Content-Type-Options` e una
/// `Content-Security-Policy` che vieta tutto ciò che non è la pagina stessa
/// sono la seconda rete sotto la neutralizzazione dell'HTML. Se un giorno un
/// carattere sfuggisse a `escape`, questa riga è ciò che impedisce al browser
/// di eseguirlo.
fn respond(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &[u8],
    extra: Option<&str>,
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    let mut head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n",
        body.len()
    );
    if let Some(extra) = extra {
        head.push_str(extra);
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

use std::io::Read as _;
