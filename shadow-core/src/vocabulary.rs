//! Il vocabolario canonico del progetto (§5) — riferimento nel codice.
//!
//! Questi sono i "nomi globali" di Shadow. Vanno usati **identici** in codice,
//! commenti, output e documentazione: la coerenza terminologica vale più
//! dell'eleganza personale, ed è il meccanismo che impedisce al progetto di
//! derivare in sinonimi che non combaciano più (§0, §11).
//!
//! Il modulo non contiene codice: è la mappa fra il termine del blueprint e il
//! punto del sorgente in cui quel termine vive. Se un termine non ha ancora una
//! controparte in codice, qui si dice **in quale fase** arriverà, così che
//! nessuno lo reinventi altrove nel frattempo.
//!
//! # Convenzione di lingua (§5)
//!
//! **Identificatori pubblici** e **testo rivolto all'utente** (CLI, report,
//! messaggi d'errore) in **inglese** — i termini canonici lo sono già e devono
//! uscire scritti esattamente così. Commenti, documentazione e **nomi dei
//! test** in **italiano**: i nomi dei test sono documentazione interna a tutti
//! gli effetti e non escono mai verso l'utente.
//!
//! È testo rivolto all'utente anche ogni stringa che nasce nel codice e finisce
//! in un output: l'evidenza dei [`Finding`](crate::Finding), i motivi di scarto
//! del [`RunManifest`](crate::RunManifest), i valori canonici delle chiavi di
//! configurazione.
//!
//! # Termini con controparte già definita (Fase 0)
//!
//! | Termine (§5) | Significato | Dove vive |
//! |---|---|---|
//! | **Shadow** | Endpoint osservato nel traffico ma assente dall'inventario dichiarato | [`Classification::Shadow`](crate::Classification::Shadow) |
//! | **Zombie** | Endpoint presente nel dichiarato ma non più osservato da oltre la finestra di staleness | [`Classification::Zombie`](crate::Classification::Zombie) |
//! | **Known** | Endpoint osservato che combacia con uno dichiarato | [`Classification::Known`](crate::Classification::Known) |
//! | **Undetermined** | Caso ambiguo: match parziale o incerto. Categoria di prima classe, mai "Known di default" | [`Classification::Undetermined`](crate::Classification::Undetermined) |
//! | **AuthObservation** | Verdetto sull'autenticazione di un `EndpointPattern`: sì / no / mista / non osservabile | [`AuthObservation`](crate::AuthObservation) |
//! | **NotObservable** | Il formato di log non trasporta l'informazione di autenticazione. **Non** equivale ad "assente" | [`AuthObservation::NotObservable`](crate::AuthObservation::NotObservable), [`ObservedAuth::NotObservable`](crate::ObservedAuth::NotObservable) |
//! | **ObservedRequest** | Unità atomica: una singola richiesta estratta da una riga di log | [`ObservedRequest`](crate::ObservedRequest) |
//! | **EndpointPattern** | Endpoint logico dopo normalizzazione (es. `/api/users/{id}`) | [`EndpointPattern`](crate::EndpointPattern) |
//! | **DeclaredEndpoint** (termine di §6, non in tabella §5) | L'endpoint atteso, estratto dall'OpenAPI | [`DeclaredEndpoint`](crate::DeclaredEndpoint) |
//! | **Finding** | Un risultato classificato con evidenza, severità, confidenza | [`Finding`](crate::Finding) |
//! | **Classification** | Enum: `Shadow` / `Zombie` / `Known` / `Undetermined` | [`Classification`](crate::Classification) |
//! | **RunManifest** | Metadati del singolo run: versione tool, versione ruleset, hash input, conteggi, timestamp | [`RunManifest`](crate::RunManifest) |
//! | **RulesetVersion** | Versione dell'insieme di regole/euristiche usato in un run | [`RulesetVersion`](crate::RulesetVersion) |
//!
//! # Termini che arrivano con le fasi successive
//!
//! Nessuno di questi va materializzato prima della sua fase, e nessuno va
//! definito fuori dal posto indicato (§P7, §P8).
//!
//! | Termine (§5) | Significato | Quando / dove |
//! |---|---|---|
//! | **ObservedInventory** | Insieme degli `EndpointPattern` ricavati dal traffico | Fase 2, in `shadow-core` |
//! | **DeclaredInventory** | Insieme degli endpoint dichiarati (da OpenAPI/Swagger fornito dall'utente) | Fase 3, in `shadow-core` |
//! | **Collector** | Modulo che traduce un formato di log specifico in `ObservedRequest` | Fase 1, in `shadow-collectors` |
//! | **Redactor** | Componente che maschera segreti/PII prima di output e persistenza | Fase 4 |
//! | **Allowlist** | Elenco locale di finding noti-innocui da silenziare (non cancellare) | Fase 4 |
