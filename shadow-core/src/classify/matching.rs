//! Confronto fra un pattern osservato e un pattern dichiarato — **stretto per
//! definizione** (§9, Fase 3).
//!
//! # Il caso peggiore che questo modulo esiste per evitare
//!
//! Un OpenAPI stantio o bugiardo, di cui il team si fida *perché Shadow non ha
//! detto niente*. Se il confronto fosse permissivo — per esempio solo sul
//! prefisso — un endpoint pericoloso sotto un prefisso dichiarato passerebbe
//! come `Known` mentre la documentazione parlava d'altro. È il falso negativo
//! silenzioso, il fallimento peggiore del prodotto (§P1).
//!
//! Quindi il confronto è per **segmenti**, e produce tre esiti, non due: o
//! combacia esattamente, o **non si sa**, o non combacia. Il caso di mezzo
//! esiste apposta perché non venga risolto verso `Known` per comodità (§P9).

use crate::ruleset::ID_PLACEHOLDER;

/// Un segmento di pattern: un valore fisso o una posizione variabile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Segmento fisso, da confrontare carattere per carattere.
    Literal(String),
    /// Posizione variabile: `{id}` nei pattern osservati, `{qualsiasi-nome}` in
    /// quelli dichiarati.
    Variable,
}

/// Esito del confronto fra un pattern osservato e uno dichiarato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMatch {
    /// Stessa forma, segmento per segmento: fissi uguali e variabili allineate.
    Exact,

    /// Forma compatibile ma **non identica**: da qualche parte uno dei due ha un
    /// valore fisso dove l'altro ha una variabile.
    ///
    /// È il caso di `/api/users/admin` osservato contro `/api/users/{id}`
    /// dichiarato: *potrebbe* essere la stessa risorsa, e potrebbe essere un
    /// endpoint amministrativo che la specifica non nomina. Non si sa, e non
    /// saperlo va detto (§P9).
    Partial,

    /// Il dichiarato è **più corto**, finisce con una variabile, e tutto ciò
    /// che viene prima è compatibile: la variabile finale dovrebbe estendersi su
    /// più segmenti per coprire l'osservato.
    ///
    /// È il caso di `/api/v1/repos/{owner}/{repo}/contents/{filepath}` osservato
    /// come `.../contents/config/app.yaml`. Un `{filepath}` che attraversa le
    /// barre esiste davvero — Gitea, GitLab e ogni API che serve file lo
    /// fanno — e OpenAPI non ha modo di dichiararlo: non c'è niente nel
    /// documento che dica se quel parametro contiene barre o no.
    ///
    /// Quindi Shadow **non lo sa**, e questa variante esiste per dirlo. Prima
    /// era una differenza di lunghezza come un'altra, cioè [`PathMatch::None`],
    /// cioè `Shadow` conclamato con confidenza alta: il tool affermava che un
    /// endpoint era assente dall'inventario mentre un path dichiarato poteva
    /// benissimo coprirlo, e faceva fallire una pipeline per questo.
    Spanning,

    /// Forme incompatibili: lunghezze diverse, o due valori fissi diversi nella
    /// stessa posizione.
    None,
}

/// Scompone un pattern **osservato** — dove la variabile è esattamente il
/// segnaposto del ruleset.
pub fn observed_segments(pattern: &str) -> Vec<Segment> {
    split(pattern, |segment| segment == ID_PLACEHOLDER)
}

/// Scompone un pattern **dichiarato** — dove la variabile è un nome fra graffe,
/// scelto da chi ha scritto la specifica: `{id}`, `{userId}`, `{user_id}`.
///
/// I nomi non si confrontano: due specifiche che chiamano diversamente la stessa
/// posizione descrivono lo stesso endpoint.
pub fn declared_segments(pattern: &str) -> Vec<Segment> {
    split(pattern, |segment| {
        segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2
    })
}

fn split(pattern: &str, is_variable: impl Fn(&str) -> bool) -> Vec<Segment> {
    let trimmed = pattern.strip_prefix('/').unwrap_or(pattern);
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed
        .split('/')
        .map(|segment| {
            if is_variable(segment) {
                Segment::Variable
            } else {
                Segment::Literal(segment.to_string())
            }
        })
        .collect()
}

/// Confronta un pattern osservato con uno dichiarato.
pub fn compare(observed: &[Segment], declared: &[Segment]) -> PathMatch {
    if observed.len() != declared.len() {
        return spanning(observed, declared);
    }

    let mut exact = true;
    for (left, right) in observed.iter().zip(declared) {
        match (left, right) {
            (Segment::Literal(a), Segment::Literal(b)) if a == b => {}
            (Segment::Variable, Segment::Variable) => {}
            // Un valore fisso da un lato e una variabile dall'altro: la forma
            // regge, ma non è la stessa cosa. Non si decide, si dichiara
            // l'incertezza.
            (Segment::Literal(_), Segment::Variable) | (Segment::Variable, Segment::Literal(_)) => {
                exact = false;
            }
            // Due valori fissi diversi: sono endpoint diversi, punto.
            (Segment::Literal(_), Segment::Literal(_)) => return PathMatch::None,
        }
    }

    if exact {
        PathMatch::Exact
    } else {
        PathMatch::Partial
    }
}

/// Quanto un pattern dichiarato **dista** da uno osservato, fra quelli che
/// combaciano parzialmente. Più piccolo è, più il dichiarato è vicino.
///
/// # Perché serve un ordine, e perché non poteva essere quello di arrivo
///
/// Il collaudo su corpus reale ha trovato questo difetto leggendo i finding:
/// quando un endpoint osservato combacia parzialmente con **più** path
/// dichiarati, l'evidenza ne nominava uno qualsiasi — il primo nell'ordine
/// della specifica. Il verdetto era giusto, la spiegazione fuorviante: per
/// `/api/v1/repos/collaudo/billing-service/issues/{id}` veniva nominato
/// `/api/v1/repos/{owner}/{repo}/issues/comments`, mentre il vicino ovvio era
/// `/issues/{index}`. In un report la spiegazione è ciò su cui l'utente decide,
/// quindi una spiegazione arbitraria è un difetto anche a verdetto corretto.
///
/// # Come si misura la distanza
///
/// Tutti i candidati parziali hanno **lo stesso numero di segmenti**
/// dell'osservato — [`compare`] restituisce [`PathMatch::None`] quando le
/// lunghezze differiscono — quindi l'unica cosa che li distingue è **dove** e
/// **quanto** divergono:
///
/// 1. **quante** posizioni divergono: meno divergenze, più vicino;
/// 2. a parità, **quanto tardi** comincia la divergenza: un prefisso che regge
///    più a lungo è un endpoint più imparentato.
///
/// L'ordine è totale e non dipende dall'ordine della specifica (§P4).
pub fn partial_distance(observed: &[Segment], declared: &[Segment]) -> (usize, usize) {
    // Un dichiarato che dovrebbe estendersi è **più lontano** di uno che
    // combacia in lunghezza, e più lontano quanti più segmenti dovrebbe
    // inghiottire: ogni segmento in più è una cosa che non sappiamo.
    let mut mismatches = observed.len().saturating_sub(declared.len());
    let mut first_mismatch = observed.len();
    for (index, (left, right)) in observed.iter().zip(declared).enumerate() {
        let same = match (left, right) {
            (Segment::Literal(a), Segment::Literal(b)) => a == b,
            (Segment::Variable, Segment::Variable) => true,
            _ => false,
        };
        if !same {
            mismatches += 1;
            first_mismatch = first_mismatch.min(index);
        }
    }
    (mismatches, observed.len() - first_mismatch)
}

/// In che **verso** un match parziale diverge.
///
/// [`PathMatch::Partial`] è simmetrico — lo stesso braccio di [`compare`]
/// cattura *fisso osservato contro variabile dichiarata* e *variabile osservata
/// contro fisso dichiarato* — mentre la frase che lo spiega all'utente non può
/// esserlo: dire «il dichiarato ha una variabile dove questo endpoint ha un
/// valore fisso» quando è vero il contrario è un'affermazione falsa, e nel
/// verso sbagliato racconta una storia opposta a quella vera.
///
/// Il caso non è raro: è quello centrale del prodotto. Traffico su
/// `/api/users/1|2|3` normalizzato in `/api/users/{id}` contro una specifica
/// che dichiara solo `/api/users/me` diverge esattamente così.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergence {
    /// L'osservato ha un valore fisso dove il dichiarato ha una variabile.
    ObservedIsFixed,
    /// L'osservato ha una variabile dove il dichiarato ha un valore fisso.
    ObservedIsVariable,
    /// Tutti e due i versi, in posizioni diverse.
    Both,
}

/// Da che parte sta la differenza fra un osservato e un dichiarato parziali.
///
/// Restituisce `None` se non divergono, cioè se il match era esatto.
pub fn divergence(observed: &[Segment], declared: &[Segment]) -> Option<Divergence> {
    let mut fixed_here = false;
    let mut variable_here = false;
    for (left, right) in observed.iter().zip(declared) {
        match (left, right) {
            (Segment::Literal(_), Segment::Variable) => fixed_here = true,
            (Segment::Variable, Segment::Literal(_)) => variable_here = true,
            _ => {}
        }
    }
    match (fixed_here, variable_here) {
        (true, true) => Some(Divergence::Both),
        (true, false) => Some(Divergence::ObservedIsFixed),
        (false, true) => Some(Divergence::ObservedIsVariable),
        (false, false) => None,
    }
}

/// Riconosce il caso in cui una variabile **finale** del dichiarato dovrebbe
/// estendersi su più segmenti per coprire l'osservato.
///
/// # Perché solo l'ultima, e perché non si fonde niente
///
/// Solo l'ultima, perché una variabile in mezzo che si estende renderebbe la
/// forma ambigua in due modi contemporaneamente — dove comincia e dove finisce —
/// e allargarla sarebbe il modo di far combaciare quasi tutto con quasi tutto.
/// §P3 dice che nel dubbio non si aggrega, e questa è la versione della regola
/// applicata al confronto invece che alla normalizzazione.
///
/// E non si fonde niente: il risultato **non è** un match. `/api/users/{id}`
/// dichiarato non «copre» `/api/users/1/comments/5` osservato. Il risultato è
/// [`PathMatch::Spanning`], che il classificatore tratta come un'incertezza da
/// dichiarare, non come una copertura da concedere — o si sarebbe scambiato un
/// falso positivo rumoroso con un falso negativo silenzioso, che §P1 chiama il
/// fallimento peggiore del prodotto.
fn spanning(observed: &[Segment], declared: &[Segment]) -> PathMatch {
    if declared.is_empty() || observed.len() <= declared.len() {
        return PathMatch::None;
    }
    if !matches!(declared.last(), Some(Segment::Variable)) {
        return PathMatch::None;
    }
    let head = declared.len() - 1;
    for (left, right) in observed.iter().take(head).zip(declared.iter().take(head)) {
        match (left, right) {
            (Segment::Literal(a), Segment::Literal(b)) if a == b => {}
            (Segment::Variable, Segment::Variable) => {}
            (Segment::Literal(_), Segment::Variable) | (Segment::Variable, Segment::Literal(_)) => {}
            // Due valori fissi diversi: sono endpoint diversi, e nessuna
            // variabile finale può rimediare.
            (Segment::Literal(_), Segment::Literal(_)) => return PathMatch::None,
        }
    }
    PathMatch::Spanning
}

/// Quanti segmenti la variabile finale dovrebbe inghiottire.
///
/// Serve all'evidenza: dire *«dovrebbe estendersi su 2 segmenti»* è un'altra
/// cosa rispetto a *«su 7»*, e chi legge decide anche su questo.
pub fn spanned_segments(observed: &[Segment], declared: &[Segment]) -> usize {
    observed.len().saturating_sub(declared.len()) + 1
}
