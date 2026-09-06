//! `shadow-spec` — legge la specifica **dichiarata** (§8).
//!
//! È il simmetrico di `shadow-collectors`, sull'altra metà del confronto:
//! quello legge il traffico osservato, questo legge ciò che il team dichiara di
//! avere. Tenerli in due crate distinti non è pignoleria — osservato e
//! dichiarato sono la distinzione su cui tutto il prodotto si regge, e
//! mescolarli nello stesso modulo è il primo passo per confonderli anche nel
//! ragionamento.
//!
//! Vincoli:
//!
//! - **§P7:** produce [`DeclaredEndpoint`](shadow_core::DeclaredEndpoint) del
//!   modello di `shadow-core`; non definisce record propri per rappresentare
//!   endpoint.
//! - **§P2:** o la specifica è comprensibile, o ci si rifiuta con un errore
//!   chiaro. Non esiste il ramo "capisco metà del file e procedo": un
//!   inventario dichiarato incompleto produrrebbe finding `Shadow` inventati,
//!   e la fiducia dell'utente crollerebbe sul primo falso allarme.
//! - **§P6:** nessuna risoluzione di riferimenti remoti. Un `$ref` verso un URL
//!   farebbe uscire il tool dalla macchina, e Shadow non apre connessioni.
//!
//! # Input non fidato, e trattato come tale
//!
//! OpenAPI 3 e Swagger 2, in **JSON e YAML**. Una specifica arriva da fuori
//! esattamente come un log, e il parser YAML è un vettore noto: le **bombe di
//! espansione** — anchor e alias che si moltiplicano a ogni livello — fanno
//! esplodere la memoria a partire da poche righe. Le difese, tutte esplicite e
//! versionate in §7:
//!
//! - dimensione massima del file;
//! - profondità massima di annidamento, misurata **senza ricorsione**;
//! - **alias YAML rifiutati** prima ancora di passare il file al parser: quando
//!   il parser li ha espansi la memoria è già finita, quindi la difesa deve
//!   stare prima. In OpenAPI il riuso si esprime con `$ref`, non con gli
//!   anchor YAML, quindi il costo pratico del rifiuto è vicino a zero.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod parse;

pub use parse::{read_spec, SpecError, SpecOutcome};
