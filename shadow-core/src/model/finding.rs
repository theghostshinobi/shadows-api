//! `Finding` e `Classification` — il risultato dell'analisi (§5, §6).

use std::collections::BTreeSet;

use super::endpoint_pattern::EndpointId;

/// `Classification` — il verdetto su un endpoint (§5).
///
/// Le quattro categorie sono il vocabolario canonico del prodotto e vanno usate
/// identiche in codice, commenti e output (§5).
///
/// Non esiste un valore di default e **non deve esistere**: §P9 impone che
/// nessun caso finisca in una categoria per comodità o per omissione. Chi
/// costruisce un [`Finding`] sceglie esplicitamente, e se non può scegliere con
/// certezza il valore corretto è [`Self::Undetermined`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Classification {
    /// Endpoint osservato nel traffico ma assente dall'inventario dichiarato (§5).
    Shadow,

    /// Endpoint presente nel dichiarato ma non più osservato da oltre la
    /// finestra di staleness (`zombie_staleness_days`, §7) (§5).
    Zombie,

    /// Endpoint osservato che combacia con uno dichiarato (§5).
    ///
    /// Il match che porta qui è **stretto** (§P3): un match parziale o
    /// approssimativo non è `Known`, è [`Self::Undetermined`]. Un `Known`
    /// sbagliato è un falso negativo silenzioso, il fallimento peggiore del
    /// prodotto (§P1).
    Known,

    /// Caso ambiguo: match parziale o incerto (§5).
    ///
    /// **Categoria di prima classe**, mai "`Known` di default". Esiste perché
    /// l'onestà dell'output è un requisito (§P9): un caso incerto viene
    /// etichettato come incerto, non spacciato per verdetto sicuro.
    Undetermined,
}

impl Classification {
    /// Termine canonico corrispondente (§5), da usare identico in ogni output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shadow => "Shadow",
            Self::Zombie => "Zombie",
            Self::Known => "Known",
            Self::Undetermined => "Undetermined",
        }
    }
}

/// Livello di confidenza di un [`Finding`]: alta / media / bassa (§6).
///
/// Va dichiarato esplicitamente su ogni finding, soprattutto su quelli
/// euristici: è ciò che permette all'utente di distinguere pochi segnali forti
/// dal rumore, invece di smettere di leggere (Fase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Confidenza bassa.
    Low,
    /// Confidenza media.
    Medium,
    /// Confidenza alta.
    High,
}

/// Severità suggerita di un [`Finding`] (§6).
///
/// "Suggerita": è un'indicazione di priorità prodotta dal tool, non un giudizio
/// di rischio definitivo — Shadow è uno strumento di supporto alla discovery,
/// non una certificazione di sicurezza (§1).
///
/// La scala ha **esattamente tre livelli** (§6): non se ne aggiungono altri, né
/// sopra né sotto. Una scala che cresce è una scala che smette di ordinare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Severità bassa.
    Low,
    /// Severità media.
    Medium,
    /// Severità alta.
    High,
}

/// Soggetto di un [`Finding`]: a cosa si riferisce il verdetto (§6).
///
/// Un finding punta a un [`EndpointPattern`](super::EndpointPattern) osservato,
/// oppure — nel caso `Zombie`, dove per definizione il traffico non c'è — a un
/// [`DeclaredEndpoint`](super::DeclaredEndpoint).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FindingSubject {
    /// Finding su un endpoint osservato, riferito tramite il suo
    /// identificatore stabile (§6).
    ObservedEndpoint(EndpointId),

    /// Finding su un endpoint dichiarato ma non osservato (caso `Zombie`).
    ///
    /// Il `DeclaredInventory` non ha un identificatore stabile in §6, e non se
    /// ne inventa uno finché non serve davvero: il riferimento usa i campi che
    /// identificano l'endpoint nella specifica — pattern del path e metodi. Se
    /// un identificatore stabile per il dichiarato servirà, emergerà in Fase 3.
    DeclaredEndpoint {
        /// Pattern del path dichiarato.
        path_pattern: String,
        /// Metodi dichiarati a cui il finding si riferisce.
        methods: BTreeSet<String>,
    },
}

/// `Finding` — un risultato classificato con evidenza, severità, confidenza (§5, §6).
///
/// È l'unità che l'utente legge. Ogni campo esiste per renderlo verificabile:
/// l'utente deve poter capire **perché** il tool dice quello che dice, e quanto
/// il tool stesso ci creda (§P9).
///
/// Prima di essere stampato o persistito, un `Finding` passa dal `Redactor`
/// (§P5, Fase 4): il report non deve mai diventare esso stesso una falla.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Finding {
    /// A cosa si riferisce il finding (§6).
    pub subject: FindingSubject,

    /// Il verdetto (§6).
    pub classification: Classification,

    /// Quanto il tool è sicuro del verdetto (§6).
    pub confidence: Confidence,

    /// Marca esplicitamente i casi ambigui (§6).
    ///
    /// Convive con [`Classification::Undetermined`] senza sostituirla: la
    /// classificazione dice *cosa* è stato deciso, questo flag dice che la
    /// decisione poggia su basi incerte.
    pub is_ambiguous: bool,

    /// Perché è stato classificato così (§6).
    ///
    /// Una voce per ciascuna ragione, in linguaggio leggibile. Sono stringhe
    /// che l'utente legge nel report, quindi **in inglese** (§5): es.
    /// `"not present in the OpenAPI spec"`, `"no auth observed while sibling
    /// patterns require it"`, `"path suggests a test environment"`. L'elenco
    /// delle ragioni possibili nasce con le euristiche in Fase 3.
    ///
    /// Passano dal `Redactor` come ogni altro testo persistito: un'evidenza che
    /// cita un path completo può contenere segreti e percorsi sensibili (§P5).
    ///
    /// Un `Finding` senza evidenza è un verdetto senza motivazione: non deve
    /// esistere nell'output.
    pub evidence: Vec<String>,

    /// Severità suggerita (§6).
    pub severity: Severity,
}
