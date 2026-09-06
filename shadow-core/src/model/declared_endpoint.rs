//! `DeclaredEndpoint` — l'endpoint atteso, estratto dal dichiarato (§6).

use std::collections::BTreeSet;

/// `DeclaredEndpoint` — un endpoint come lo dichiara l'OpenAPI/Swagger fornito
/// dall'utente (§6).
///
/// L'insieme dei `DeclaredEndpoint` è il `DeclaredInventory` (§5): il termine
/// di paragone contro cui l'`ObservedInventory` viene confrontato in Fase 3.
/// Il parsing che li produce è Fase 3; qui c'è solo la forma del record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredEndpoint {
    /// Pattern del path così come dichiarato (es. `/api/users/{id}`).
    pub path_pattern: String,

    /// Metodi HTTP dichiarati per questo path. Ordinato per determinismo (§P4).
    pub methods: BTreeSet<String>,

    /// Schemi di autenticazione dichiarati per questo endpoint.
    ///
    /// Insieme (anziché valore singolo) perché una specifica può dichiararne
    /// più d'uno sullo stesso endpoint: tenerli tutti evita di scartarne uno in
    /// silenzio (§P2). Insieme **vuoto** = nessuno schema dichiarato, che è
    /// un'informazione, non un dato mancante.
    pub declared_auth_schemes: BTreeSet<String>,
}
