//! `DeclaredInventory` — l'insieme degli endpoint dichiarati (§5, Fase 3).

use std::collections::BTreeMap;

use crate::model::DeclaredEndpoint;

/// `DeclaredInventory` — l'insieme degli endpoint dichiarati dalla specifica
/// fornita dall'utente (§5).
///
/// È il termine di paragone contro cui l'[`ObservedInventory`](super::ObservedInventory)
/// viene confrontato. A produrlo è `shadow-spec`, che legge il file: qui c'è
/// solo la forma, perché `shadow-core` non fa I/O (§8).
#[derive(Debug, Clone, Default)]
pub struct DeclaredInventory {
    endpoints: BTreeMap<String, DeclaredEndpoint>,
}

impl DeclaredInventory {
    /// Costruisce l'inventario da un insieme di endpoint dichiarati.
    ///
    /// Endpoint con lo stesso pattern di path si fondono unendo metodi e schemi
    /// di autenticazione: una specifica può dichiarare lo stesso path sotto più
    /// server, e perderne uno significherebbe non riconoscerlo poi nel traffico.
    pub fn from_endpoints(endpoints: impl IntoIterator<Item = DeclaredEndpoint>) -> Self {
        let mut merged: BTreeMap<String, DeclaredEndpoint> = BTreeMap::new();
        for endpoint in endpoints {
            match merged.get_mut(&endpoint.path_pattern) {
                Some(existing) => {
                    existing.methods.extend(endpoint.methods);
                    existing
                        .declared_auth_schemes
                        .extend(endpoint.declared_auth_schemes);
                }
                None => {
                    merged.insert(endpoint.path_pattern.clone(), endpoint);
                }
            }
        }
        Self { endpoints: merged }
    }

    /// Quanti endpoint dichiara.
    pub fn len(&self) -> usize {
        self.endpoints.len()
    }

    /// Scorre gli endpoint dichiarati in ordine deterministico (§P4).
    pub fn iter(&self) -> impl Iterator<Item = &DeclaredEndpoint> {
        self.endpoints.values()
    }
}
