//! La decisione più delicata del progetto: **quale segmento di path è
//! variabile** (§9, Fase 2).
//!
//! # Il fallimento da evitare
//!
//! Non è "non riconoscere un identificatore": è il **falso raggruppamento**. Se
//! `/api/users/admin` finisce dentro `/api/users/{id}`, un endpoint
//! amministrativo — magari senza autenticazione — sparisce dentro un gruppo
//! dall'aria normale, e nessuno lo guarda più. Il tool nasconderebbe
//! attivamente qualcosa che era nei dati grezzi: il contrario del suo scopo.
//!
//! Il costo dei due errori non è simmetrico. Due righe che sono la stessa cosa
//! l'utente le riconosce e le ignora. Un endpoint che sparisce non lo vede
//! nessuno, per definizione (§P3).
//!
//! # La regola: due condizioni, entrambe necessarie
//!
//! Un segmento diventa `{id}` **solo se**:
//!
//! 1. la **posizione** mostra evidenza forte di essere variabile — almeno
//!    [`MIN_DISTINCT_VALUES_FOR_VARIABLE`] valori distinti tipo-ID, nella stessa
//!    posizione e **sotto lo stesso prefisso** già deciso;
//! 2. **quel singolo valore** ha una forma tipo-ID ([`super::path::is_id_like`]).
//!
//! È la seconda condizione a salvare `admin`: la posizione può anche essere
//! variabilissima, ma `admin` non ha forma di identificatore e resta letterale,
//! producendo un pattern tutto suo. Nessuna soglia di cardinalità, per quanto
//! alta, può inghiottirlo.
//!
//! # Perché la decisione si prende durante la lettura
//!
//! La regola chiede *almeno* tre valori distinti: **oltre il terzo, la
//! decisione non cambia più**. Tenerne in memoria cinquantamila non aggiunge
//! nulla al verdetto, e su un log ad alta cardinalità — cioè proprio quello in
//! cui la normalizzazione serve di più — costa gigabyte.
//!
//! Quindi l'albero decide mentre viene costruito: ogni posizione conserva al
//! massimo i valori tipo-ID visti finora, e appena arrivano al terzo li fonde in
//! un unico ramo `{id}`, dimenticandoli. Chi arriva dopo entra direttamente nel
//! ramo fuso.
//!
//! **Non è un'euristica diversa, è la stessa senza accumulare dati inutili.**
//! Il risultato è identico a quello che darebbe una decisione presa alla fine su
//! tutto l'insieme, e in particolare **non dipende dall'ordine** in cui le
//! richieste arrivano: quando una posizione collassa, i rami già creati per i
//! valori precedenti vengono fusi retroattivamente, non lasciati indietro.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::ObservedRequest;
use crate::ruleset::{ID_PLACEHOLDER, MIN_DISTINCT_VALUES_FOR_VARIABLE};

use super::path::{is_id_like, join_segments};
use super::PathObservations;

/// L'albero dei path osservati, che decide la variabilità mentre cresce.
#[derive(Debug, Clone, Default)]
pub(super) struct PatternTree {
    root: Node,
}

impl PatternTree {
    /// Registra una richiesta sul suo path canonico.
    pub(super) fn observe(&mut self, segments: &[String], request: &ObservedRequest) {
        let mut node = &mut self.root;
        for segment in segments {
            let key = node.key_for(segment);
            node = node.children.entry(key).or_default();
        }
        node.observations
            .get_or_insert_with(PathObservations::default)
            .add(request);
    }

    /// Raccoglie i pattern con le osservazioni che vi sono confluite.
    ///
    /// La discesa è iterativa e non ricorsiva: un path con decine di migliaia di
    /// segmenti è un input che qualcuno può costruire apposta, e non deve poter
    /// esaurire lo stack.
    pub(super) fn collect(self) -> BTreeMap<String, PathObservations> {
        let mut out: BTreeMap<String, PathObservations> = BTreeMap::new();
        let mut stack: Vec<(Vec<String>, Node)> = vec![(Vec::new(), self.root)];

        while let Some((prefix, node)) = stack.pop() {
            if let Some(observations) = node.observations {
                out.entry(join_segments(&prefix))
                    .or_default()
                    .merge(observations);
            }
            for (segment, child) in node.children {
                let mut child_prefix = prefix.clone();
                child_prefix.push(segment);
                stack.push((child_prefix, child));
            }
        }

        out
    }
}

#[derive(Debug, Clone, Default)]
struct Node {
    /// Figli, indicizzati per segmento **già deciso**: un valore letterale
    /// oppure il segnaposto.
    children: BTreeMap<String, Node>,

    /// Valori tipo-ID distinti visti in questa posizione, finché la posizione
    /// non collassa. Si svuota al collasso: da quel momento non servono più,
    /// ed è questo a tenere piatta la memoria.
    id_like_seen: BTreeSet<String>,

    /// La posizione dei figli è già stata giudicata variabile.
    collapsed: bool,

    /// Presente se un path finisce esattamente qui.
    observations: Option<PathObservations>,
}

impl Node {
    /// Decide con quale chiave scendere, applicando le due condizioni.
    fn key_for(&mut self, segment: &str) -> String {
        // Seconda condizione, e la più importante: se il valore non ha forma di
        // identificatore, nessuna cardinalità lo tocca.
        if !is_id_like(segment) {
            return segment.to_string();
        }
        if self.collapsed {
            return ID_PLACEHOLDER.to_string();
        }

        self.id_like_seen.insert(segment.to_string());
        if self.id_like_seen.len() >= MIN_DISTINCT_VALUES_FOR_VARIABLE {
            self.collapse();
            return ID_PLACEHOLDER.to_string();
        }
        segment.to_string()
    }

    /// La posizione diventa variabile: i rami già creati per i valori tipo-ID
    /// vengono **fusi retroattivamente** in un unico ramo `{id}`.
    ///
    /// È la fusione retroattiva a rendere il risultato indipendente dall'ordine
    /// di arrivo delle richieste (§P4): i primi due identificatori non restano
    /// indietro come pattern separati.
    fn collapse(&mut self) {
        self.collapsed = true;
        let keys: Vec<String> = self.id_like_seen.iter().cloned().collect();
        self.id_like_seen.clear();

        let mut merged = self.children.remove(ID_PLACEHOLDER).unwrap_or_default();
        for key in keys {
            if let Some(child) = self.children.remove(&key) {
                merged.merge(child);
            }
        }
        self.children.insert(ID_PLACEHOLDER.to_string(), merged);
    }

    /// Fonde un altro sottoalbero in questo.
    fn merge(&mut self, other: Node) {
        match (&mut self.observations, other.observations) {
            (Some(mine), Some(theirs)) => mine.merge(theirs),
            (slot @ None, theirs @ Some(_)) => *slot = theirs,
            (_, None) => {}
        }

        // Se uno dei due rami aveva già deciso che la posizione è variabile, la
        // decisione vale per il ramo fuso: non si torna indietro.
        if other.collapsed && !self.collapsed {
            self.collapse();
        }
        if !self.collapsed {
            self.id_like_seen.extend(other.id_like_seen);
        }

        for (key, child) in other.children {
            // In un ramo già collassato, un figlio tipo-ID che arriva da fuori
            // finisce nel segnaposto: altrimenti riapparirebbe come letterale.
            let key = if self.collapsed && is_id_like(&key) {
                ID_PLACEHOLDER.to_string()
            } else {
                key
            };
            match self.children.entry(key) {
                Entry::Occupied(mut existing) => existing.get_mut().merge(child),
                Entry::Vacant(slot) => {
                    slot.insert(child);
                }
            }
        }

        // La fusione può aver portato la posizione oltre la soglia.
        if !self.collapsed && self.id_like_seen.len() >= MIN_DISTINCT_VALUES_FOR_VARIABLE {
            self.collapse();
        }
    }
}
