# PROGRESS — stato dei cancelli di Shadow

Meccanismo anti-deriva di §11 del [blueprint](blueprint.md). Questo file è la
memoria del progetto fra una sessione e l'altra.

**Prima di lavorare, in ogni sessione:** rileggi §5 (vocabolario canonico) e
questo file. Sappi sempre: in quale fase sei, cosa esisteva prima, cosa aggiunge
la fase corrente.

**Definizione di "fatto" per una fase (§11) — servono tutti e tre:**

1. cancello di cablaggio verde (verificato eseguendo, non a sensazione)
2. fixture della fase verdi
3. nessun codice morto (§P8: tutto raggiungibile dall'entrypoint della CLI)

**Trigger di STOP (§0, §11):** se una decisione non è coperta dal blueprint,
fermarsi e chiedere al fondatore. §3 elenca le decisioni note ancora aperte:
non vanno inventate.

---

## Fase corrente

**Fasi 0–4 chiuse. Fase 5 aperta: la prima fetta è cablata e verde.** In mezzo c'è il **collaudo**, e in
mezzo al collaudo c'è la **taratura**: le due correzioni che il collaudo su
corpus reale ha reso specifiche invece che ipotesi. Sono state fatte il
2026-09-05 e sono descritte sotto, in *Taratura post-collaudo*.

La pipeline va dal log al report consegnabile: log nginx → Collector →
`ObservedRequest` → normalizzazione → `ObservedInventory` → classificazione →
`Finding` → **redazione** → report (terminale, JSON, Markdown) + `RunManifest`.

Blueprint alla **v1.12** (dodici revisioni esplicite del fondatore, tutte in §13
del [blueprint](blueprint.md)). `RulesetVersion` **0.8.0**; **241** verifiche
automatiche verdi.

**§6 è stato riaperto una volta**, nella v1.6, per un solo campo — la
configurazione effettiva del run nel `RunManifest` — ed è stato **richiuso
subito dopo**. Nessun'altra fase ne ha avuto bisogno.

Il passo successivo **non è la Fase 5**: è il **collaudo**. §9 dice di non
iniziare la Fase 5 finché le Fasi 0–4 non sono complete *e collaudate*, e
collaudate significa su log veri — le fixture le ha scritte l'agente sapendo
cosa dovevano trovare, un log vero no.

Il piano è in [COLLAUDO.md](COLLAUDO.md). **La prima passata è stata eseguita**
il 2026-08-30 su due applicazioni reali dietro un reverse proxy che scrive
`combined`: Gitea 1.27.3 (parametri di path basati su **nomi**) e Vikunja 2.5.0
(parametri **numerici**), ciascuna con la propria specifica Swagger servita
dalla stessa build che serve il traffico.

Corpus, specifiche, driver e varianti di `log_format` sono versionati in
`collaudo/corpus/`: il collaudo si rifà identico fra sei mesi.

**Le statistiche sui finding sono in `collaudo/misure/` e non vanno lette prima**
di aver etichettato `collaudo/DA-GIUDICARE.md` (trenta finding, stratificati per
tipo, senza numeri allegati). Vederle prima significherebbe confermarle invece
che misurarle — è il metodo scritto in COLLAUDO.md, e vale anche per chi l'ha
scritto.

---

## Fase 0 — Fondamenta e contratti

**Stato: COMPLETATA** (verificata il 2026-08-29 con i comandi riportati sotto).

### Deliverable (§9)

- [x] Workspace Cargo con i crate `shadow-core`, `shadow-collectors`, `shadow-cli`
      (vuoti ma compilanti), struttura di §8
- [x] Modello dati canonico (§6) definito in `shadow-core`
- [x] Vocabolario (§5) come riferimento nel codice → `shadow-core/src/vocabulary.rs`
- [x] Scheletro di `RunManifest`
- [x] File di licenza BSL 1.1 con parametri segnaposto (§3) → `LICENSE`
- [x] `PROGRESS.md` inizializzato con la checklist dei cancelli

### Cancello di cablaggio (§9)

> Il workspace compila; la CLI si avvia e stampa versione + aiuto.

- [x] `cargo build --workspace` → verde
- [x] `./target/debug/shadow --version` → `shadow 0.0.0` + ruleset + fase
- [x] `./target/debug/shadow --help` → aiuto completo **su stdout**: è output
      che l'utente ha chiesto, non diagnostica (§7, v1.2)
- [x] `./target/debug/shadow` (senza argomenti) → aiuto **su stderr**, stdout vuoto (§7)
- [x] `./target/debug/shadow access.log` → errore d'uso, uscita 2 (§P2: mai
      accettare in silenzio un input che non si sa gestire)

### Criteri di riuscita (§9)

- [x] `cargo build` verde su tutto il workspace
- [x] Il modello dati è definito **una sola volta** e in `shadow-core`
- [x] `PROGRESS.md` esiste e riflette lo stato

### Verifica avversaria (§9)

> Rischio: definire modelli dati incoerenti o duplicati. Controllo: nessun crate
> oltre `shadow-core` definisce record propri per rappresentare richieste/endpoint.

**Esito: superata.** Log completo in fondo a questo file.

### Fixture (§10)

Nessuna fixture di analisi, e **non serve un'esenzione**: il fondatore ha
corretto §11 alla radice (v1.3), che ora chiede *verifiche automatiche verdi
appropriate alla fase* — in Fase 0 i test sui contratti, dalla Fase 1 in poi le
fixture dorate più le avversarie obbligatorie. La regola non ha più un buco al
primo passo. I contratti sono coperti da 6 test in
`shadow-core/tests/model_contracts.rs` (`cargo test --workspace` → verde),
inclusi i due contratti chiusi nella revisione v1.1 (quattro stati dell'auth con
la sua base di calcolo, digest sempre accompagnato dal proprio algoritmo).
`tests/fixtures/` esiste con l'elenco delle fixture obbligatorie per fase.

### Come si legge §P8 in Fase 0

Preso alla lettera — "tutto raggiungibile dall'entrypoint della CLI" — §P8 in
Fase 0 sarebbe inapplicabile: §9 elenca il modello dati **fra i deliverable
della fase** e fissa il cancello a "workspace che compila + CLI che stampa
versione e aiuto". Nessun record di §6 può essere raggiunto da `main` finché non
esiste una pipeline che li attraversa.

Lettura **ratificata dal fondatore il 2026-08-29** (§13, v1.3), insieme alle
rimozioni che ne erano già discese:

- in Fase 0 §P8 vieta i **componenti speculativi** — costruttori, accessori,
  `impl` scritti "perché prima o poi serviranno" e che nessuna decisione
  richiede. Questi vanno rimossi, non commentati;
- il modello di §6 è il deliverable della fase, e i test di contratto sono la
  prova che §10 prescrive: è lì che il modello viene esercitato;
- **dalla Fase 1 in poi §P8 vale nella sua forma piena**, perché la pipeline
  raggiunge il modello partendo dall'entrypoint. Questa deroga scade con la
  Fase 0.

Due confini che la Fase 1 ha reso necessario tracciare, e che vanno scritti
perché non diventino comodi:

- **`impl` scritti a mano contro `derive`.** Un `impl` che nessuno chiama è
  codice speculativo e va rimosso — così sono spariti `impl Error for
  IngestError` e i `Display` orfani. Un `derive` no: è meccanico, non si
  manutiene, ed è parte della forma normale di un tipo.
- **il modello di §6 non è codice speculativo.** `EndpointPattern`, `Finding`,
  `Classification` e gli altri non sono ancora raggiunti da `main` in Fase 1, ma
  sono un deliverable **ratificato e congelato dal fondatore**, non qualcosa che
  l'agente ha inventato "per dopo". §P8 governa ciò che l'agente costruisce di
  propria iniziativa.

### Cosa NON è stato costruito (deliberatamente)

Elenco esplicito, così che nessuno lo cerchi credendolo perso:

- **nessun contratto `Collector`** — è deliverable di Fase 1; fissarne ora la
  forma significherebbe decidere streaming e gestione errori senza un consumatore;
- **nessun tipo `ObservedInventory` / `DeclaredInventory`** — sono termini di §5
  senza descrizione dei campi in §6; il contenitore concreto nasce in Fase 2/3;
- **nessun parser, nessuna normalizzazione, nessuna classificazione** (Fasi 1-3);
- **nessun flag operativo nella CLI** — le chiavi di configurazione di §7
  esistono come contratto nel blueprint, ma esporle nell'aiuto prima che facciano
  qualcosa sarebbe output disonesto (§P9);
- **exit code `1` e `3` non ancora emessi** — i valori sono ora fissati in §7
  (`0` nessun finding, `1` errore di esecuzione, `2` errore d'uso, `3` finding
  `Shadow`/`Zombie`) e il contratto è documentato in `shadow-cli/src/main.rs`,
  ma introdurre costanti che nessuno raggiunge sarebbe codice morto (§P8): li
  emetterà la fase che avrà qualcosa da segnalare. Oggi il binario produce `0` e
  `2`, entrambi già conformi al contratto.

---

## Fase 1 — Walking skeleton: ingestione nginx end-to-end

**Stato: COMPLETATA** (2026-08-30, verificata eseguendo).

**Cancello:** eseguire la CLI su un file di log nginx reale produce, end-to-end,
un elenco stampato di endpoint osservati **e** un `RunManifest` con i conteggi
(righe totali / parsate / scartate) **e il digest SHA-256 del file di input**.

- [x] Lettore a flusso (mai tutto in memoria) → `shadow-collectors/src/ingest.rs`
- [x] Digest SHA-256 calcolato **nella stessa passata** della lettura e
      registrato nel manifest con il proprio algoritmo (§6, v1.2)
- [x] Parser nginx (primo Collector) dietro l'interfaccia Collector
      → `shadow-collectors/src/{collector,nginx}.rs`
- [x] Validazione del formato all'avvio: nelle prime 100 righe non vuote, se
      **nessuna** combacia, il run si rifiuta (§P2)
- [x] Righe malformate: skip + conteggio + **perché**, registrato nel manifest
- [x] Output minimale in terminale, sullo stream giusto (§7)
- [x] Un log nginx valido produce endpoint osservati + manifest
- [x] Il manifest riporta il digest SHA-256 dell'input, con l'algoritmo accanto
- [x] Il digest è calcolato nella stessa passata: nessuna seconda lettura
- [x] Memoria stabile a prescindere dalla dimensione del file (§P10) —
      **misurata**: RSS di picco identico (6.520.832 byte) su 11 MB e su 211 MB
- [x] Le righe malformate vengono contate, non fanno crashare il run
- [x] Un errore di esecuzione esce con exit code `1` (§7)
- [x] Fixture avversarie verdi: riga corrotta a metà file, file grande, formato
      non riconosciuto, encoding sporco, riga senza ritorno a capo

**Verifiche automatiche (§11):** 28 test verdi — 6 sui contratti del modello,
10 su ingestione e fixture, 12 sul cancello eseguendo il binario.

**Cosa NON è stato costruito, deliberatamente:** nessuna normalizzazione dei
path, nessun `EndpointPattern`, nessun raggruppamento logico. In Fase 1 l'output
sono **richieste osservate**: `/api/users/1` e `/api/users/2` restano due righe
distinte, e un test lo verifica. Quando la Fase 2 le fonderà in
`/api/users/{id}`, il cambiamento di questo stesso output sarà la prova che le
due fasi sono collegate.

## Fase 2 — Inventario osservato + normalizzazione dei path

**Stato: COMPLETATA** (2026-08-30, verificata eseguendo).

**Cancello:** l'output della Fase 1 alimenta la normalizzazione; eseguendo la CLI
si ottiene un inventario di `EndpointPattern`, non più richieste grezze. Sullo
**stesso file** `valid.log`: 5 richieste distinte in Fase 1, **3 endpoint** in
Fase 2, con `/api/users/{id}` che ne somma tre.

- [x] Normalizzazione conservativa dei path → `shadow-core/src/inventory/`
- [x] Costruzione dell'`ObservedInventory`
- [x] Assegnazione dell'identificatore stabile (digest del solo pattern)
- [x] Regola di propagazione dell'autenticazione, con la base numerica popolata
- [x] `/api/users/1|2|3` collassano in `/api/users/{id}`
- [x] Endpoint realmente diversi restano separati
- [x] Lo stesso endpoint riceve lo stesso identificatore in run diversi (§P4),
      e non cambia se cambia il traffico osservato
- [x] Un pattern senza richieste osservabili risulta `NotObservable`, non
      "senza auth"; il numero di osservabili è mostrato accanto al verdetto
- [x] Fixture avversarie verdi: endpoint-admin nascosto fra 5.000 identificatori,
      path con encoding e doppia codifica, separatore codificato

**Verifiche automatiche (§11):** 46 test verdi — 6 sui contratti del modello,
15 sulla normalizzazione, 10 su ingestione e fixture, 15 sui cancelli eseguendo
il binario.

**La regola, in due condizioni entrambe necessarie.** Un segmento diventa `{id}`
solo se (1) la posizione mostra almeno 3 valori distinti tipo-ID sotto lo stesso
prefisso già deciso **e** (2) quel singolo valore ha forma di identificatore. È
la seconda a salvare `admin`: nessuna cardinalità, per quanto alta, può
inghiottire una parola. Verificato con 5.000 identificatori attorno.

**Cosa NON è stato costruito:** nessun OpenAPI, nessun `DeclaredInventory`,
nessuna classificazione, nessun `Finding`. Sono Fase 3.

## Fase 3 — Classificazione (con e senza OpenAPI)

**Stato: COMPLETATA** (2026-08-30, verificata eseguendo).

**Cancello:** pipeline completa log → inventario → classificazione. Con OpenAPI
si ottengono finding etichettati per categoria, senza si ottengono sospetti
euristici. Il collegamento con le fasi precedenti è dimostrato dal fatto che i
finding riferiscono gli `EndpointPattern` della Fase 2 con il loro
identificatore stabile.

- [x] Parsing OpenAPI/Swagger → `DeclaredInventory` → crate **`shadow-spec`**
- [x] Confronto stretto osservato vs dichiarato → `shadow-core/src/classify/`
- [x] Motore euristico per il caso senza dichiarato
- [x] Le euristiche sull'assenza di auth non si attivano su `NotObservable`
- [x] Evidenza, confidenza e `is_ambiguous` popolati su ogni `Finding`
- [x] Endpoint osservato assente dall'OpenAPI → `Shadow`
- [x] Endpoint dichiarato non osservato oltre `zombie_staleness_days` → `Zombie`
- [x] Match incerto → `Undetermined`, **mai** `Known`
- [x] Senza OpenAPI nessun finding è `Shadow`, e un inventario tranquillo
      produce **zero** righe
- [x] Exit code: `3` solo su conclamati, `0` su soli `Undetermined`,
      `--fail-on-undetermined` opt-in
- [x] Fixture avversarie verdi: OpenAPI stantio con endpoint pericoloso sotto un
      prefisso dichiarato, YAML rifiutato con una via d'uscita, finestra troppo
      corta per dire `Zombie`

**Verifiche automatiche (§11):** 77 test verdi — 6 sui contratti, 19 sulla
normalizzazione, 13 sulla classificazione, 6 sulla lettura della specifica, 10
su ingestione e fixture, 23 sui cancelli eseguendo il binario.

**Le due regole che tengono in piedi la fase.** Il confronto è per segmenti e ha
**tre** esiti, non due: combacia, non si sa, non combacia. Il caso di mezzo — un
valore fisso osservato dove la specifica ha una variabile — esiste apposta
perché non venga risolto verso `Known` per comodità. E senza inventario
dichiarato non si dice mai `Shadow`: l'assenza da un inventario che non c'è non
è verificabile, quindi è un sospetto e si chiama `Undetermined`.

**Cosa NON è stato costruito:** nessun `Redactor`, nessuna allowlist, nessun
export JSON o Markdown. Sono Fase 4.

## Fase 4 — Output, redazione, allowlist, manifest

**Stato: COMPLETATA** (2026-08-30, verificata eseguendo).

**Cancello:** eseguire la CLI su un log reale con parametri sensibili produce un
report (a) azionabile, (b) **con i segreti mascherati**, (c) accompagnato da un
manifest, (d) con i finding in allowlist silenziati.

- [x] Output terminale leggibile + export **JSON** e **Markdown**
- [x] `Redactor` applicato prima di ogni stampa e di ogni scrittura → `redact.rs`
- [x] Nessun token, email, chiave o password nell'output di default
- [x] Percorsi di filesystem mascherati nel manifest, negli avvisi **e nei
      messaggi d'errore** (§P5 esteso)
- [x] `--show-raw-values` opt-in, e il report dice a chi legge che è grezzo
- [x] `Allowlist` che silenzia il report **senza** togliere dai conteggi
- [x] Avviso sulle regole di allowlist troppo ampie
- [x] `RunManifest` completo, con la **configurazione effettiva e la provenienza**
- [x] `shadow ... > report.json` produce un file pulito (§7)
- [x] Due run sullo stesso input danno lo stesso JSON, timestamp a parte (§P4)
- [x] Un finding in allowlist non compare nel report ma resta nei conteggi

**Verifiche automatiche (§11):** 106 test verdi — 6 sui contratti, 19 sulla
normalizzazione, 13 sulla classificazione, 7 sulla redazione, 7 sull'allowlist,
9 sulla lettura della specifica, 10 su ingestione e fixture, 35 sui cancelli
eseguendo il binario. Conteggio riverificato eseguendo `cargo test --workspace`
il 2026-09-05: i due in più rispetto alla chiusura della fase sono il saldo del
recepimento della v1.7 sui cancelli. La taratura successiva ne ha aggiunti 49,
portando il totale a **155**.

**La regola che attraversa la fase:** niente esce senza passare dal `Redactor`.
Non è una raccomandazione, è il motivo per cui ogni funzione del modulo del
report riceve un `Redactor` invece di una stringa già formattata: così non
esiste un percorso di codice che stampi qualcosa senza aver deciso cosa fare
della redazione.

**Cosa NON è stato costruito:** nessuna persistenza, nessun demone, nessun
report di compliance, nessuna multi-tenancy. Sono Fase 5, e §9 dice di non
iniziarla finché le Fasi 0–4 non sono **collaudate**.

## Fase 5 — Persistenza + demone + report compliance (post-MVP, tier a pagamento)

**Stato: prima fetta COMPLETATA** (2026-09-05, verificata eseguendo). Le quattro
decisioni che la bloccavano le ha prese il fondatore — blueprint v1.10,
decisioni 35–38 — e sono in §13, non in chat.

**Cancello di cablaggio:** due esecuzioni sullo stesso input con `--history`; la
prima non ha mai visto niente, la seconda ha già visto tutto; `shadow alerts`
elenca gli shadow non presi in carico. Che sia cablata e non affiancata lo
dimostra una cosa sola: gli endpoint che lo storico ricorda sono gli
`EndpointPattern` della **Fase 2** con il loro identificatore stabile, e i
verdetti sono quelli della **Fase 3**. La Fase 5 non ha aggiunto analisi: ha
aggiunto memoria.

- [x] Storico su SQLite → crate **`shadow-history`**, schema con migrazioni
      versionate in `PRAGMA user_version`
- [x] Alert su nuovo endpoint shadow, **senza egress**: riga marcata nello
      storico + `shadow alerts` + il codice di uscita del run
- [x] Multi-tenancy come **separazione dei bersagli**: `--target`, e gli
      inventari di due servizi non si mescolano mai
- [x] `shadow alerts --acknowledge <ID>`: silenzia senza cancellare l'evidenza
- [x] A input costante il report è **identico** con e senza storico (§P4)
- [x] Fixture avversarie obbligatorie della fase, tutte verdi: log ruotato
      durante la lettura, log troncato durante la lettura, orologio che va
      all'indietro, database corrotto, storico scritto da una versione più
      nuova, due esecuzioni concorrenti sullo stesso storico, retention che
      prova a cancellare evidenza ancora citata
- [x] **Esecuzione continua come demone** → crate `shadow-daemon` +
      `shadow daemon`, con lettura incrementale e digest cumulativo
- [x] **Report di compliance** → crate `shadow-compliance` + `shadow compliance`,
      in **CSV, Markdown e JSON** (`shadow-compliance/1`)
- [x] **Interfaccia di supervisione**, tutte e quattro le superfici: `shadow
      status` (terminale), `shadow dashboard` (pagina HTML autonoma), `shadow
      serve` (server locale in sola lettura), e **Shadow.app** per macOS nella
      barra dei menu. Studio in [INTERFACCIA.md](INTERFACCIA.md)
- [ ] Politica di retention — **ancora da decidere**

**Verifiche automatiche (§11):** 214 test verdi, di cui 20 sullo storico e 14 sui
cancelli della Fase 5 eseguendo il binario.

### La revisione avversaria, e gli otto difetti che ha trovato in questa fetta

Terza volta su tre: le fixture erano verdi e la fetta era **sbagliata**. Dei
diciassette rilievi confermati, deduplicati, restano otto difetti distinti. Il
primo è il peggiore che questa fase potesse avere.

| # | Difetto | Perché era grave | Correzione |
|---|---|---|---|
| 1 | **Un run senza `--openapi-spec` azzerava in silenzio la coda degli alert.** L'alert si deduceva dall'ultima classificazione, e un run che su quell'endpoint non aveva niente da dire ci scriveva `NULL` | È la modalità d'uso **preimpostata** di Shadow. Un job notturno senza specifica cancellava gli alert del job di CI che ce l'aveva, e nessuno li aveva presi in carico: erano spariti. Il falso negativo silenzioso di §P1, proprio sul meccanismo che esiste per non far perdere un endpoint | L'alert è un **fatto avvenuto**, con una colonna sua (migrazione 2): nasce quando un endpoint è visto `Shadow`, e lo toglie solo chi lo prende in carico. Il verdetto non si sovrascrive più con «nessun verdetto» |
| 2 | **Due primi run concorrenti si uccidevano a vicenda**: la migrazione girava in transazione differita e il perdente la rieseguiva su un database già migrato | Misurato: **venti fallimenti su venti** iterazioni con due soli processi. Due job di CI che partono insieme non sono un caso avversario | Transazione `Immediate` e `user_version` riletta **dentro** la transazione — la stessa difesa che `record_run` usava già |
| 3 | **Un `user_version` negativo** mandava il cast in wraparound: nessuna migrazione, `migrate` dichiarava lo schema corrente, e lo storico si apriva **senza tabelle** | Il run se ne accorgeva solo a fine analisi — con il gigabyte già macinato, contro la garanzia scritta nel codice — e con un messaggio che parlava di permessi | Una versione che non riconosciamo si restituisce com'è e ferma l'apertura |
| 4 | **`shadow alerts` creava lo storico** se il percorso era sbagliato | A un errore di battitura rispondeva «nessun endpoint shadow da prendere in carico», uscita 0: diceva *tutto a posto* proprio quando non aveva guardato niente | Uno storico da leggere deve già esistere (`open_existing`) |
| 5 | **Un refuso in `--target`** dava «nessun alert», uscita 0 | Un bersaglio che non c'è non è un bersaglio tranquillo | Lo dice, ed elenca quali ci sono |
| 6 | **`--acknowledge` di un id inesistente** usciva con successo | Uno script non poteva accorgersene | Uscita `1`: non ha fatto ciò che gli era stato chiesto |
| 7 | **Il percorso dello storico usciva in chiaro** dentro il testo d'errore di SQLite, mascherato nell'altra metà della stessa riga; e `alerts` lo mostrava in chiaro anche quando veniva dall'ambiente | §P5, sulla superficie nuova che questa fase introduce | L'errore passa da `redact_paths` come quelli dell'ingestione e della specifica; in `alerts` l'eccezione vale solo per ciò che l'utente ha digitato |
| 8 | **`--history 'file::memory:'`** apriva un database volatile | Il tool avrebbe detto di ricordare senza ricordare niente, e ogni run avrebbe trovato tutto nuovo. Togliere il flag `SQLITE_OPEN_URI` non basta: SQLite è compilato con `SQLITE_USE_URI` | Il valore si rifiuta rumorosamente: una chiave di §7 è un percorso, non un piccolo linguaggio |

In più, `shadow alerts` ora rispetta la **precedenza di §7** (flag > ambiente >
default) come tutto il resto: prima leggeva direttamente da clap, quindi
`SHADOW_TARGET` valeva per un'analisi e non per la lettura degli alert.

**Una decisione presa scegliendo fra due letture, e scritta perché si veda.** Un
endpoint che viene documentato *dopo* — da `Shadow` a `Known` — **resta in coda**
finché qualcuno non lo prende in carico. L'alert dice «questo endpoint è comparso
senza essere documentato», e resta un fatto avvenuto anche se poi la specifica è
stata aggiornata: toglierlo da solo significherebbe che nessuno si accorge di ciò
che è successo (§P1, §P3). Il verdetto corrente resta però aggiornato accanto,
così chi legge vede tutt'e due le cose.

### Demone e report d'audit (2026-09-06)

**Il demone.** `shadow daemon <log> --history h.db --interval 60`. Legge **solo
ciò che è cresciuto**, ma classifica l'inventario **dall'inizio**: classificare
il solo pezzo nuovo darebbe verdetti diversi da quelli di un comando singolo
sullo stesso file, e §P4 dice che a input costante il verdetto non cambia. Il
costo è la memoria dell'inventario, che cresce con gli endpoint **distinti** e
non con la dimensione del log (§P10) — *«se questo costa in prestazioni, paga»*.

- Il **digest è cumulativo**: quello dei primi N byte letti, non del solo pezzo
  nuovo. Un manifest che dichiarasse mille byte mentre i finding ne coprono un
  milione mentirebbe su cosa ha analizzato (§6). Una fixture verifica che il
  digest di due letture incrementali sia **identico** a quello del file intero.
- Una **rotazione** viene detta, mai ripresa in silenzio: se si ricominciasse
  senza dirlo, tutto ciò che era già stato visto risulterebbe nuovo.
- Il crate `shadow-daemon` contiene **solo il tempo** — quando guardare, quante
  volte, quando fermarsi. Niente della pipeline: un demone con dentro una copia
  della pipeline è il modo più rapido di far divergere `shadow` da
  `shadow daemon`.
- Un **intervallo zero senza limite di cicli è rifiutato**: sarebbe un ciclo
  stretto che consuma una macchina in silenzio (§P2).
- Il demone **non scrive su stdout**: il report si chiede con `alerts` e
  `compliance`. Ma **produce manifest**, uno per giro, nello storico: «nessun
  manifest, nessun verdetto» vale anche qui (§7).

**Il cancello, e la cosa che deve valere sopra ogni altra:** demone e comando
singolo, sullo stesso input, danno **lo stesso inventario e gli stessi
verdetti**. Il test li confronta esportando l'inventario d'audit da entrambi.

**Il report di compliance.** `shadow compliance --history h.db --format csv`.
Tre formati perché servono a tre persone: il **CSV** è ciò che un auditor apre
davvero, il **Markdown** è la versione che si legge con i limiti dichiarati in
testa, il **JSON** è un contratto versionato `shadow-compliance/1` — un
identificatore **distinto** da `shadow-report/1`, perché è un documento diverso
con un pubblico diverso.

- **I quattro valori dell'autenticazione non collassano mai in un binario.** Non
  è una raccomandazione nel codice: sono una colonna con cinque righe possibili,
  e la tabella riassuntiva mostra **anche gli zeri**, perché uno stato che
  sparisce quando è vuoto fa credere che non esista.
- Accanto c'è sempre **su quante richieste osservabili** il verdetto si regge:
  «senza auth» e «senza auth, su 3 richieste su 5000» non sono la stessa
  affermazione (§6).
- C'è un **quinto stato che non è di §6**: `not recorded`, per le righe scritte
  prima che lo storico registrasse l'auth. Non è «non osservabile» — è «non lo
  abbiamo scritto» — e confonderle sarebbe la stessa perdita silenziosa in un
  piano diverso.
- Il documento **dichiara di sé** cosa non è, in tutti e tre i formati: §1 mette
  la certificazione fra i non-obiettivi e chiede che l'onestà si veda
  nell'output.
- Un bersaglio inesistente **non produce un inventario vuoto**: consegnare a un
  auditor un documento vuoto per un refuso sarebbe la risposta peggiore.

**Migrazione 3 dello storico:** metodi, conteggio osservazioni, stato dell'auth
e sua base di calcolo. Senza, il report d'audit non avrebbe potuto portare i
quattro valori che §9 gli impone.

### Le quattro superfici di supervisione (2026-09-06)

Il fondatore le ha volute tutte e quattro, più l'applicazione macOS. **Una vista
sola, quattro modi di guardarla:** il crate `shadow-view` contiene la struttura,
e nessuna superficie ne calcola una propria. Se ognuna calcolasse la sua, prima
o poi direbbero numeri diversi sullo stesso storico, e chi guarda il cruscotto e
chi guarda il terminale non potrebbero discutere della stessa cosa.

| Superficie | Comando | Dove serve |
|---|---|---|
| Terminale | `shadow status` | Linux, sessioni ssh, `cron` |
| Pagina autonoma | `shadow dashboard --out stato.html` | Si condivide, si archivia con una data |
| Server locale | `shadow serve` | Guardare da un'altra macchina |
| Barra dei menu macOS | `Shadow.app` | Averlo sott'occhio senza cercarlo |

**La liveness senza socket.** Il demone accetta `--dashboard stato.html` e
riscrive la pagina a ogni giro; la pagina porta un `<meta http-equiv="refresh">`
e un browser lasciato aperto **segue da solo**, rileggendo un file da disco.
Copre quasi tutto ciò che si voleva dal server, senza aprire niente. La pagina si
scrive **atomicamente** — prima accanto, poi si sposta — perché chi la sta
leggendo veda quella di prima o quella nuova, mai una a metà.

**Il server, e cosa cambia.** È l'unica cosa del progetto che apre un socket, e
sta in un crate suo perché la superficie di rete si legga tutta in una volta.
Tre difese, in ordine:

1. **Non si apre niente se non lo si chiede**: nessun'altra parte del progetto
   usa quel crate, solo il sottocomando `shadow serve`.
2. **Ascolta su `127.0.0.1`** se non si dice altro, e **avvisa a voce alta**
   quando l'indirizzo è raggiungibile da fuori.
3. **È in sola lettura**: non esiste nessun percorso che scriva nello storico.
   Prendere in carico un alert resta una cosa del terminale — un server che
   scrive è una superficie di scrittura remota su uno strumento offline, ed è
   una decisione diversa da quella che è stata presa.

In più: nessuna dipendenza di rete (libreria standard), `Content-Security-Policy`
che vieta tutto ciò che non è la pagina, `nosniff`, nessun percorso da
attraversare perché i percorsi serviti sono **due fissi**, e un limite di
richieste che esiste per poterlo verificare — un server che si può solo avviare
non si può mettere sotto test.

**L'applicazione macOS.** `Shadow.app`, nella barra dei menu, `LSUIElement` così
non compare nel Dock. Primo clic: i dettagli in un popover. Ancora: la finestra
di visione, che mostra **la stessa pagina** di `shadow dashboard` in un
`WKWebView` **con JavaScript spento** — la pagina non ne contiene, e una vista
che non esegue niente non può essere sorpresa. Non parla con il database: chiama
`shadow status --format json`, quindi non ha una superficie di attacco propria.
Il numero accanto al segno compare **solo quando c'è qualcosa da decidere**: una
barra dei menu che mostra sempre un numero smette di essere guardata.

**Il segno del prodotto** sono tre barre orizzontali (scelta del fondatore, che
ha mandato l'immagine): disegnate come *template image* nella barra dei menu —
macOS le ricolora da sé secondo il tema — e come carattere `≡` nei testi.

### L'avversaria che conta più delle altre, su questa superficie

**Un HTML è un'uscita ostile: un browser esegue.** Il pattern di un endpoint
viene dai dati analizzati, cioè da chi ha scritto le richieste — che su uno
strumento di sicurezza è per definizione qualcuno di cui non ci si fida. Un path
costruito ad arte che finisse nella pagina non neutralizzato sarebbe **§P5
rovesciato**: non un segreto che esce, un'iniezione che entra.

- Nessuna stringa che venga dai dati raggiunge la pagina senza passare da
  `escape`, e si neutralizzano anche `'` e `"` — non solo i tre caratteri che
  «bastano» nel corpo — perché il giorno in cui una finisse in un attributo il
  buco esisterebbe già.
- Una fixture prova con
  `/api/</table><script>alert(1)</script><table>/x` e verifica che non arrivi
  nessun tag: il path resta **leggibile**, perché chi guarda deve poter vedere
  che qualcuno ha provato.
- La `Content-Security-Policy` del server è la seconda rete sotto la prima.

**Un difetto mio, trovato eseguendo:** il primo test del server lo lasciava in
attesa senza che nessuno chiedesse niente, e bloccava l'intera suite. Ora il test
si collega davvero, ed è il motivo per cui `--requests` esiste.

### Il difetto Unicode, corretto (2026-09-06) — `RulesetVersion` 0.8.0

Trovato da una revisione avversaria **interrotta a 12 agenti su ~35**: era già
il primo rilievo confermato.

**Cos'era.** I caratteri invisibili e di direzione attraversavano la
normalizzazione intatti e arrivavano in terminale, nella pagina, nel report
d'audit. Due conseguenze, entrambe riprodotte sul binario:

```
cc7359a30fd4e062  GET  /api/admin      <- due endpoint diversi,
2e9389b15f846e5f  GET  /api/ad​min      <- tipograficamente identici
```

e `U+202E` rovescia il testo che segue: `/api/‹U+202E›txt.exe` **si legge**
`/api/exe.txt`. Un path può presentarsi come qualcosa che non è, in un documento
che va in mano a un auditor.

**Non è un allargamento della regola: è la sua ultima riga applicata alla
lettera.** Il commento di `reencode` diceva già *«i caratteri di controllo
(illeggibili in un report, o peggio)»*. Il «o peggio» è questo.

- La regola sta ora in **un posto solo**, `ruleset::must_be_reencoded`, dove §7
  vuole le cose che decidono l'esito di un'analisi.
- Copre C0, **C1** (che il controllo per byte di prima si lasciava sfuggire),
  i marcatori di direzione e isolamento, lo spazio a larghezza zero, il BOM, il
  trattino morbido e gli spazi che non si distinguono da uno normale.
- **Ri-codificare non nasconde e non fonde niente:** rende visibile ciò che era
  invisibile, ed è reversibile. È l'opposto di un'aggregazione, quindi non tocca
  §P3.
- **Ciò che si vede resta leggibile:** `città`, `日本`, `naïve`, `Ελλάδα` passano
  intatti. Ri-codificarli sarebbe stato il difetto che questa correzione esiste
  per non causare, e ha una fixture sua.
- Corretto anche un difetto vero nel codice esistente: `push_escape` prendeva
  **un byte solo**, quindi su un carattere multi-byte avrebbe cambiato il dato
  invece di renderlo visibile.

**Una fixture che asseriva la cosa sbagliata, e l'ho corretta io.** Avevo scritto
che il carattere e la sua codifica percentuale dovessero restare due endpoint:
non è vero, sono lo **stesso** path, e la Fase 2 li unifica di proposito come fa
con `..` e `%2e%2e`. La doppia codifica invece resta distinta, e il test ora
verifica entrambe le cose.

### Tre emendamenti da ratificare

1. **`shadow-view`** in §8 — la vista condivisa. Zero rete.
2. **`shadow-serve`** in §8 — la sola cosa che apre un socket.
3. **Il senso della decisione 36 cambia.** Il 5 settembre avevi scelto l'alert
   senza egress perché *«la frase "questo binario non apre socket" resta
   verificabile dall'esterno»*. Con `shadow serve` quella frase non è più vera
   così com'è, e la formulazione che propongo è:

   > L'unica rete di Shadow sta in `shadow-serve`, si legge tutta in un file, si
   > apre solo su richiesta esplicita con `shadow serve`, ascolta solo questa
   > macchina se non si dice altro, ed è in sola lettura.

   Non l'ho scritta in §P6 né in §13: quella è una revisione tua, e la propongo
   invece di applicarla.

### L'interfaccia di supervisione: studiata, non costruita

Alla domanda *«c'è un'interfaccia web / terminale / applicativo locale?»* la
risposta era **no**: quattro comandi e zero socket in tutto il progetto. Il
fondatore ha chiesto tutte e tre le forme — HTML statico, TUI, server locale —
più una domanda sulla distribuzione, e ha detto **«studiamocela prima di
intervenire»**.

Lo studio è in [INTERFACCIA.md](INTERFACCIA.md) e **non è stato implementato
niente**. I punti che decidono:

- **Lo storico contiene già tutto**: le tre forme sono tre viste sulla stessa
  tabella, nessuna calcola niente di nuovo. Si può fare una forma alla volta
  senza pentirsene.
- **Il server locale rompe la decisione 36**, presa il giorno prima: *«la frase
  "questo binario non apre socket" resta verificabile dall'esterno»*. Non per
  esfiltrazione — è in ascolto, non chiama nessuno — ma perché quella frase è
  binaria. La mitigazione che la mantiene quasi vera è una **feature di
  compilazione**, e la decisione è del fondatore.
- **L'HTML ha un rischio suo che le altre due non hanno:** un browser esegue. Un
  pattern di endpoint che finisse nella pagina senza essere neutralizzato
  sarebbe §P5 rovesciato — non un segreto che esce, ma un'iniezione che entra.
- **La distribuzione viene dopo il nome.** npm è fattibile e va fatto senza
  script di postinstall che scaricano binari — è il pattern che le catene di
  fornitura hanno visto usare per compromettere, ed è ciò da cui l'offline-first
  tiene Shadow lontano. Ma pubblicare su un registro prende il nome davvero, e
  §3 tiene «Shadow» come **nome di lavoro** con un conflitto ancora da
  verificare.
- **Manca un `shadow status`** più di quanto manchi una UI: è la vista che le
  altre tre condividerebbero, e farlo per primo evita di scriverla tre volte.

### Le tre regole che tengono in piedi la fase

**§P4 vale anche col tempo di mezzo.** Un demone introduce orologio, ordine di
arrivo e stato accumulato: tre modi di far dipendere un verdetto da qualcosa che
non è l'input. La separazione è strutturale, non una promessa: `shadow-core`
classifica senza sapere che lo storico esista, e la novità — *questo endpoint non
c'era* — è un'annotazione **accanto** al finding, mai un ingrediente. Un test
confronta i due report carattere per carattere.

**L'ordine non lo decide l'orologio.** I run sono ordinati dal proprio
identificatore progressivo. Un orologio all'indietro viene **detto**, non
aggiustato — se l'ordine dipendesse dal tempo, un endpoint già visto tornerebbe
nuovo.

**Nessun manifest, nessun verdetto, anche qui.** `shadow alerts` legge una
memoria: niente manifest, e non afferma «nessun finding». E se lo storico non si
lascia scrivere il run si ferma **prima** di stampare, perché un report stampato
con la memoria non aggiornata farebbe risultare tutto nuovo all'esecuzione dopo.

### Due emendamenti — **ratificati** (blueprint v1.11, decisioni 39 e 40)

Erano inferenze dell'agente, segnalate come tali. Ratificate dal fondatore il
2026-09-05, e ora sono nel documento:

1. **Il crate `shadow-history`** entra in §8. Lo storico non è né la CLI né il
   demone: entrambi lo scrivono, e metterlo nel demone farebbe dipendere la CLI
   dal demone — l'arco inverso che §8 vieta. §8 registra anche la regola delle
   **migrazioni versionate**: si aggiunge in coda, non si modifica una
   migrazione già rilasciata, e una versione sconosciuta ferma l'apertura.
2. **`history_path` e `target`** entrano fra le chiavi di §7, con la precedenza
   e la registrazione nel manifest di tutte le altre. §7 scrive anche che
   `history_path` è un percorso e **non** un URI.

Nessuna delle due fa salire la `RulesetVersion`: una chiave di configurazione in
più non è una regola che decide l'esito di un'analisi, e il codice le
implementava già.

### Cosa NON è stato costruito

Nessun demone: senza uno storico non avrebbe avuto niente da accumulare, e con
uno storico è il passo successivo. Nessun report di compliance: il formato è
ancora da decidere. Nessuna retention: lo **schema** garantisce già che una
cancellazione non possa portarsi via evidenza ancora citata da un endpoint — un
vincolo di chiave esterna, non una promessa — ma *quando* e *se* cancellare non
è deciso.

## Decisioni aperte (§3) — non inventare, chiedere

- [ ] **Nome "Shadow"**: possibile conflitto con prodotti esistenti. Nome di
      lavoro fino a decisione. *Impatto attuale:* binario `shadow`,
      `shadow_core::TOOL_NAME`, nomi dei crate, `Licensed Work` nel `LICENSE`.
- [ ] **Parametri BSL residui**: la finestra di conversione è **decisa (4 anni,
      v1.1)**; restano aperti la definizione precisa di "uso aziendale", il
      Licensor, la Change License e il contatto commerciale.
      *Impatto attuale:* 7 segnaposto `<DA DEFINIRE>` nel file `LICENSE`, fra cui
      la data concreta di Change Date (che ora si calcola: rilascio + 4 anni).
- [ ] **Collector oltre nginx**: quale formato aggiungere per secondo.
- [ ] **Politica di retention dello storico** (Fase 5): quando e se cancellare.
      La proprietà di sicurezza è già garantita dallo schema; la politica no.
- [ ] **Formato del report di compliance** (Fase 5).
- [ ] **Confine free/paid**: imposto tecnicamente o solo dalla licenza.

### Decisioni chiuse dalla revisione v1.1 (2026-08-29)

Le domande sollevate dalla Fase 0 sono chiuse e **recepite nel blueprint**: non
vivono più in chat. La tabella canonica è §13 del blueprint (nove decisioni); qui
il riassunto con l'impatto sul codice, dove la conseguenza sulle euristiche è
elencata a parte per leggibilità pur essendo parte della decisione sull'auth.

- [x] **Scala della severità**: tre livelli (`Low`/`Medium`/`High`), nessun
      altro. → §6; `Severity` in `finding.rs`.
- [x] **Auth non osservabile a livello di `EndpointPattern`**: `NotObservable` è
      la quarta variante. Propagazione: nessuna richiesta osservabile → il
      pattern è `NotObservable`; almeno una osservabile → il verdetto si calcola
      sulle sole osservabili, e il loro numero è registrato ed è mostrato
      all'utente. → §6, Fase 3, Fase 4; `AuthObservation` e il nuovo campo
      `auth_observable_count` in `endpoint_pattern.rs`.
- [x] **Euristiche su `NotObservable`**: non si attivano quelle basate
      sull'assenza di auth — eccezione consapevole a §P1, annotata nel
      blueprint. → Fase 3 (deliverable + criteri).
- [x] **Numeri degli exit code**: `0` / `1` / `2` / `3`; gli `Undetermined` da
      soli non attivano il `3`. → §7; contratto documentato in `main.rs`.
- [x] **Algoritmo di hash**: SHA-256 sul contenuto del file, con l'algoritmo
      registrato accanto al digest. → §6; `DigestAlgorithm` e il campo
      `algorithm` di `InputDigest` in `run_manifest.rs`.
- [x] **Riferimento al `DeclaredEndpoint`**: pattern del path + metodi; nessun
      identificatore stabile per il dichiarato finché non serve. → §6.
- [x] **I due allargamenti** (valori multipli per chiave di query, schemi di auth
      dichiarati al plurale): confermati. → §6.
- [x] **`ObservedInventory` / `DeclaredInventory`**: restano termini di
      vocabolario, i tipi nascono in Fase 2/3. → §5 (già corretto).
- [x] **Lingua**: identificatori **pubblici** e testo utente in inglese;
      documentazione, commenti e **nomi dei test** in italiano (ratificato il
      2026-08-29: i nomi dei test sono documentazione interna). → §5;
      `vocabulary.rs`.
- [x] **BSL**: finestra di conversione a 4 anni. → §3; `LICENSE`.

### Consapevolezze da non perdere (non sono domande aperte, sono conseguenze)

- In CI un run di soli `Undetermined` esce con `0` e quindi **risulta verde**.
  Non è una svista: un run senza dichiarato non ha basi per bocciare una build,
  e chi vuole un gate vincolante fornisce l'OpenAPI. Chi vuole il gate anche sui
  sospetti accende `--fail-on-undetermined`, opt-in e mai default (§7, v1.2).
  Il peso dell'onestà resta comunque su report e manifest, dove gli
  `Undetermined` sono visibili ed espliciti.
- Le euristiche spente su `NotObservable` sono l'unica eccezione a §P1 finora
  concessa: se un domani qualcuno le riaccende "per coerenza con P1", sta
  riaprendo una decisione chiusa, non correggendo una svista (Fase 3).

---

## Chiuso dalla revisione v1.2 (2026-08-29)

Le tre contraddizioni bloccanti sono chiuse e recepite nel blueprint (§13, v1.2).
Su una di esse **il blueprint aveva torto**, ed è stato riscritto invece che
aggirato — vale la pena ricordarlo, perché è il precedente che autorizza a
segnalare di nuovo un errore del documento invece di adattarcisi.

- [x] **Contratto stdout/stderr riformulato.** Il criterio non è
      "machine-readable contro leggibile" ma **chi ha chiesto cosa**: stdout
      porta ciò che l'utente ha chiesto (report, `--help`, `--version`), stderr
      tutto il resto. Mandare l'aiuto richiesto su stderr romperebbe
      `shadow --help | less`. → §7; `main.rs`.
- [x] **Nessun manifest, nessun verdetto.** `0` significa *analisi eseguita e
      completata* con zero `Shadow`/`Zombie`; i codici con semantica di finding
      escono solo insieme a un `RunManifest`. Un'invocazione che non analizza
      nulla non afferma mai "nessun finding". → §7, Fase 4; `main.rs`,
      `run_manifest.rs`.
- [x] **Digest SHA-256 spostato in Fase 1**, calcolato nella stessa passata di
      lettura. Un manifest che tace quale file ha analizzato viola la propria
      ragione d'essere. → §9 Fase 1 (deliverable, cancello, criteri).
- [x] **I finding euristici sono `Undetermined`**, mai `Shadow`: un sospetto non
      è un fatto verificabile, e la sfumatura la porta la confidenza. Il `3`
      scatta solo su conclamati; `--fail-on-undetermined` è la via d'uscita
      opt-in. → §7, §9 Fase 3.
- [x] **Ratificate** entrambe le modifiche che avevo fatto di mia iniziativa:
      BSL come scelta compiuta in §2, e i nomi dei test in italiano.

Anticipate nella stessa revisione perché discendono da principi già fissati:

- [x] **§P5 esteso ai percorsi di filesystem.** Un
      `/home/utente/clienti/bancaXYZ/logs/` rivela cliente e struttura interna e
      finisce nel manifest che si consegna all'auditor. → §4 P5, §7;
      `observed_request.rs`, `run_manifest.rs`.
- [x] **Stringhe italiane che escono verso l'utente, corrette** (violavano la
      decisione 8, già chiusa): esempi di evidenza in §6, valore `terminal` di
      `output_format` in §7, motivi di scarto del manifest. → §5, §6, §7;
      `finding.rs`, `run_manifest.rs`.

## Chiuso dalla revisione v1.3 (2026-08-29)

- [x] **Lettura di §P8 ratificata**, incluse le rimozioni già fatte. Confini
      tracciati durante la Fase 1: sopra, in "Come si legge §P8".
- [x] **Nessuna esenzione dalle fixture di Fase 0: corretta §11.** La
      definizione di "fatto" chiede verifiche automatiche verdi *appropriate
      alla fase*. La Fase 0 è fatta senza eccezioni, e la regola non ha più un
      buco al primo passo. §10 allineata di conseguenza.

## Chiuso dalla revisione v1.4 (2026-08-30)

- [x] **Schema di autenticazione neutro** quando il formato non lo dice:
      `ObservedAuth::SCHEME_UNSPECIFIED`. Il fatto si conserva, l'inferenza non
      si spaccia per osservazione. → §6; `observed_request.rs`, `nginx.rs`.
- [x] **I sette motivi di scarto sono contratto congelato**, allo stesso titolo
      di §6: aggiungerne è consentito, rinominarne o rimuoverne no. → §7.
- [x] **Le soglie di ingestione entrano nel `RulesetVersion`.** `MAX_LINE_BYTES`
      e `FORMAT_SAMPLE_LINES` sono migrate in `shadow-core/src/ruleset.rs`,
      insieme alle soglie di normalizzazione della Fase 2. Il criterio generale
      è ora scritto in §7: *se cambiarla cambia l'esito di un'analisi, è parte
      del ruleset*. → §7; `ruleset.rs`.
- [x] **Il default di `log_format` resta, condizionato** al fatto che la
      validazione sia stretta; se si allentasse va rimosso nello stesso
      momento. → §7.

Conseguenza operativa: il ruleset non è più vuoto, quindi `RulesetVersion` è
passata da `0.0.0` a **`0.1.0`**. Questo risponde in pratica alla domanda "chi
incrementa la `RulesetVersion`": **la fase che introduce o cambia una soglia di
§7**. Se vuoi che sia una regola scritta nel blueprint e non solo una prassi,
va detto.

## Chiuso dalla revisione v1.5 (2026-08-30)

- [x] **Collasso incrementale della normalizzazione.** La memoria è passata da
      lineare nei path distinti a **piatta**: 6 MB con un milione di path
      distinti, dove prima ne sarebbero serviti circa 1,3 GB. Tutte le fixture
      avversarie passano **invariate**, inclusa quella a 5.000 identificatori.
      → §9 Fase 2; `inventory/variability.rs`.
- [x] **Regola scritta per la `RulesetVersion`** e schema di numerazione. → §7.
- [x] **Identificatore stabile sul solo pattern: confermato.** → §6.
- [x] **`shadow-spec`**, crate proprio per il dichiarato. → §8.

## Chiuso dalla revisione v1.6 (2026-08-30)

- [x] **La configurazione effettiva entra nel `RunManifest`**, con la provenienza
      di ogni valore. **Unica riapertura di §6 dopo il congelamento**, richiuso
      subito dopo. → §6; `run_manifest.rs`, `config.rs`.
- [x] **YAML supportato** con `serde_norway`, e con limiti espliciti: dimensione,
      profondità misurata senza ricorsione, **alias rifiutati prima del parser**.
      → §7; `shadow-spec`.
- [x] **Granularità per-metodo**: nessun secondo cantiere su §6. L'evidenza
      nomina i metodi non dichiarati e la severità sale di un livello.
- [x] **`zombie_staleness_days` a 30 giorni** e **composizione della severità**:
      confermate.

## Chiuso dalla revisione v1.7 (2026-08-30)

- [x] **Redazione su stderr, con un'eccezione mirata**: i percorsi che l'utente
      ha scritto sulla riga di comando in questa stessa invocazione compaiono in
      chiaro; tutto il resto resta mascherato, e report e manifest non hanno
      eccezioni. → §P5; `main.rs`, `config.rs`.
- [x] **Schema del report chiuso: `shadow-report/1`**, contratto pubblico
      versionato. Aggiungere campi sì, rinominarli o rimuoverli richiede
      `/2`. → §7; **voce rimossa da §3**.
- [x] **Un avviso non è un verdetto**: le soglie che governano avvisi non fanno
      parte del ruleset. Scritto in §7 come precedente.
- [x] **I finding silenziati non contano per il codice di uscita.** → §7, Fase 4.

Nessuna di queste ha fatto salire la `RulesetVersion`, che resta `0.3.0`: è la
decisione 31 applicata a se stessa: cambiano ciò che si legge, non ciò che il
tool conclude.

## Chiuso dalla revisione v1.8 (2026-09-05)

- [x] **Valori canonici delle chiavi booleane**: `true`/`1` e `false`/`0`, senza
      varianti di maiuscole e senza sinonimi. Un valore diverso ferma il run
      invece di valere `false`, e lo stesso vale per una variabile d'ambiente
      che esista ma non sia testo valido. → §7; `config.rs`, `cancello.rs`.

Non fa salire la `RulesetVersion`, che resta `0.5.0`: sta nella lettura della
configurazione, che l'elenco di §7 non nomina fra le cose del ruleset, e a
configurazione valida non cambia nessun verdetto.

**Cosa questa revisione non copriva.** L'emendamento a §7 su `log_format` — la
grammatica dichiarabile — era rimasto fuori di proposito: la v1.8 portava una
decisione sola. È stato ratificato subito dopo, con la **v1.9**.

## Chiuso dalla revisione v1.9 (2026-09-05)

- [x] **La grammatica di log è dichiarabile.** `log_format` accetta o un nome
      canonico o la direttiva del proprio server, riconoscibile perché contiene
      `$`. Stessa strettezza di validazione, dichiarazione ambigua o incompleta
      che ferma il run prima dei dati, valore preimpostato che resta perché il
      vincolo condizionale di §7 non è cambiato. → §7; `log_format.rs`,
      `nginx.rs`, `lib.rs`, `main.rs`.

Non fa salire la `RulesetVersion` di suo: la taratura che la implementa l'aveva
già portata a `0.4.0`, ed è a `0.5.0` dalle correzioni sulla redazione.
Ratificare un comportamento già implementato non cambia nessun verdetto.

**Con questa, il codice e il blueprint tornano allineati.** Non restano
comportamenti implementati che il documento non conosca.

## Chiuso dalla revisione v1.11 (2026-09-05)

- [x] **Il crate `shadow-history`** entra in §8, con la regola delle migrazioni
      versionate. → §8.
- [x] **`history_path` e `target`** entrano fra le chiavi di §7. `history_path`
      è un percorso, non un URI. → §7.

Non fanno salire la `RulesetVersion`, che resta `0.5.0`.

**Non restano inferenze dell'agente in attesa di conferma.** Codice e blueprint
sono allineati: nessun comportamento implementato che il documento non conosca.

## Chiuso dalla revisione v1.12 (2026-09-06)

- [x] **`path_prefix`** entra fra le chiavi di §7: quale parte del traffico è
      l'API. Opt-in, preimpostato «analizza tutto», richieste fuori portata
      contate e dette. → §7; `config.rs`, `main.rs`, `report.rs`.

§13 registra anche le due cose cambiate insieme **senza** essere revisioni: il
buco di `{filepath}` (applicazione della definizione di `Shadow` di §5, non una
regola nuova) e il raggruppamento dei finding nel report (presentazione, che §7
dichiara non essere un contratto).

`RulesetVersion` a **0.7.0**, per `{filepath}` e per `path_prefix`. Il
raggruppamento non contribuisce.

**Non restano inferenze dell'agente in attesa di conferma.**

## Il collaudo, prima della Fase 5

Piano in [COLLAUDO.md](COLLAUDO.md). Le cinque domande: **rumore** (la
principale), leggibilità sotto redazione, qualità della normalizzazione su forme
di URL non immaginate, robustezza del parsing sulle varianti reali di
`log_format`, scala su volume vero.

**Regola del collaudo:** prima si raccoglie, poi si decide insieme. Nessuna
modifica a euristiche, soglie o cataloghi finché i risultati non sono sul
tavolo — osservare e cambiare nello stesso passaggio è il modo di convincersi
che qualcosa funziona.

- [x] **Corpus reale**: due applicazioni, forme di parametro opposte, log
      prodotto da un server vero. Versionato e ripetibile.
- [x] **Soglie confermate** dal fondatore; quella del 30% riformulata come
      innesco di una discussione, non come voto.
- [x] **Previsione su `$request_time` verificata**: confermata. Una variante di
      `log_format` su sette passa — quella senza aggiunte. Tabella in COLLAUDO.md,
      varianti in `collaudo/corpus/varianti-log-format/`.
- [x] **Il passo umano: fatto il 2026-09-06**, dall'agente su autorizzazione
      esplicita del fondatore. Trenta etichette in `collaudo/DA-GIUDICARE.md`,
      scritte **prima** di aprire `collaudo/misure/`. Esito e riserve in fondo a
      [COLLAUDO.md](COLLAUDO.md). **La riserva resta:** questo documento aveva
      riservato il passo al fondatore perché è il tool che altrimenti giudica il
      proprio output, e la riserva non sparisce perché il corpus è pubblico.
      Il numero è un innesco, non un voto, e va confermato.
- [x] **I due difetti raccolti dal collaudo sono stati corretti** il 2026-09-05,
      su indicazione del fondatore di procedere. Vedi *Taratura post-collaudo*.
- [ ] **Resta da decidere insieme**: le quattro conversazioni che il passo
      umano ha aperto (vedi sotto). Nessuna è una taratura di soglia: sono
      decisioni di prodotto, tranne la quarta che è un difetto del confronto.

**Difetto del corpus, trovato rieseguendolo il 2026-09-05:**
`collaudo/corpus/gitea-access.log` è **vuoto (0 byte)**. Il corpus di Vikunja è
intero (66 KB) e si riesegue; quello di Gitea no. COLLAUDO.md dichiara che il
collaudo «si rifà identico fra sei mesi», e **per metà non è vero**: le misure
in `collaudo/misure/gitea-report.json` non sono più riproducibili dal corpus
versionato. Non l'ho rigenerato — servirebbe l'istanza Gitea che l'ha prodotto —
e non ho toccato le misure.

## Esito del passo umano del collaudo (2026-09-06)

Etichettato dall'agente su autorizzazione del fondatore, con le riserve scritte
in `DA-GIUDICARE.md` e in COLLAUDO.md. **1 utile, 23 rumore, 6 «non so»** (i sei
sono tutti `Known`, dove la domanda non si applica).

Sui soli `Undetermined`: **16 su 17 sono rumore, il 94%**, contro una soglia del
30%. Superata di tre volte — quindi si apre la conversazione su come tarare, che
è precisamente ciò che la soglia serviva a innescare.

**L'etichettatura e le misure aggregate, lette solo dopo, indicano le stesse
quattro famiglie.** Ogni finding non-`Known` dei due corpus cade in una di esse,
e su **536 finding non-`Known` uno solo vale la pena di guardarlo**:
`/api/v1/tasks/all` di Vikunja, dove `all` è una parola letterale dove la
specifica ha un identificatore — la stessa forma di `/api/users/admin`, il caso
per cui §P3 esiste. Non l'ho scelto sapendolo: le misure dicono che è l'unico
finding di tutto il report di Vikunja fuori dalla famiglia dominante.

### Le quattro conversazioni che si aprono, e non le apre l'agente

1. **«Dichiarato ma mai osservato, finestra troppo corta»** — 305 finding su 423
   in Gitea, 115 su 127 in Vikunja. Il tool sa già che la finestra non basta:
   potrebbe dirlo una volta sola con il conteggio, invece di 305 volte. Non è
   nascondere informazione — i conteggi del manifest restano interi — è
   deciderne la forma.
2. **Match parziale su un parametro di path letterale** — su un'API basata su
   nomi scatta su ogni utente e ogni repository. È §P3 che funziona, e costa.
3. **La UI web dentro il log** — un log con pagine e API fa risultare ogni
   pagina un endpoint non documentato. Servirebbe un modo di dire quale parte
   del traffico è l'API: è una chiave di configurazione nuova, e non la invento.
4. **`{filepath}` che attraversa le barre** — **corretto il 2026-09-06**, era
   l'unico dei quattro che si correggeva nel codice. Vedi sotto.

### Le altre tre conversazioni, chiuse (2026-09-06)

Su richiesta esplicita del fondatore. Due erano **attenzione**, una era
**portata**.

**1 e 2 — il report raggruppa per la ragione, non per il soggetto.** Il tool non
aveva concluso 305 cose: ne aveva conclusa una, su 305 soggetti. Ora la frase si
scrive una volta e sotto ci sono tutti i soggetti, uno per riga. Su Vikunja la
sezione dei finding passa da **370 righe a 143 — il 61% in meno** — e i 115
«dichiarato ma mai osservato» diventano una riga più 115 nomi. La stessa
correzione chiude anche i diciassette `/api/v1/users/<nome>` con la spiegazione
identica.

- Il raggruppamento è **strutturale, non testuale**: la prima riga di evidenza è
  identica byte per byte fra i membri di una famiglia, perché nomina il path
  *dichiarato* e non quello osservato. Nessuna stringa analizzata per indovinare.
- **Nessun finding sparisce** e il **JSON resta identico**: è il contratto, e §7
  dice che il layout è presentazione. Per questo **non** fa salire la
  `RulesetVersion` — cambia quante volte si legge la stessa frase, non ciò che
  il tool ha concluso.
- Sotto i tre membri il gruppo si stampa per esteso: ripetere una spiegazione
  due volte non costa attenzione a nessuno, ripeterla 115 sì.

**3 — `path_prefix`: quale parte del traffico è l'API.** Un log con pagine e API
faceva risultare ogni pagina un endpoint non documentato, con `Shadow`
conclamato ed exit `3`.

- **Opt-in**, e il preimpostato resta «analizza tutto»: scegliere da soli quale
  traffico sia l'API significherebbe decidere di non guardare qualcosa senza
  dirlo, e §P1 lo vieta.
- Le richieste fuori portata **non spariscono**: sono contate, e il conteggio
  esce su stderr, nel report e nel JSON (`requests_outside_scope`).
- Un prefisso che non comincia con `/` è **rifiutato**: ridurrebbe l'analisi a
  zero in silenzio (§P2).
- `RulesetVersion` **0.6.0 → 0.7.0**: cambia cosa viene analizzato.

**Ratificata dal fondatore il 2026-09-06 → blueprint v1.12, decisione 41.**
La chiave `path_prefix` è in §7, e §13 registra anche cosa è cambiato insieme
senza essere una revisione: il buco di `{filepath}` e il raggruppamento del
report.

### Il buco di `{filepath}`, corretto (2026-09-06) — `RulesetVersion` 0.6.0

**Cos'era.** Un parametro dichiarato che contiene barre — `{filepath}`, che ogni
API che serve file usa — produceva una differenza di lunghezza, e una differenza
di lunghezza era `PathMatch::None`, cioè `Shadow` conclamato con confidenza
alta. Il tool **affermava** che un endpoint era assente dall'inventario mentre
un path dichiarato poteva benissimo coprirlo, e faceva fallire una pipeline per
questo. Erano **18 dei 61 `Shadow`** di Gitea: il 30% dei conclamati.

**La correzione.** Una quarta forma di confronto, `PathMatch::Spanning`: il
dichiarato è più corto, finisce con una variabile, e tutto ciò che viene prima è
compatibile. Non è un match — non copre il dichiarato, esattamente come il match
parziale — ed è `Undetermined`, con un'evidenza che dice **quanti segmenti** la
variabile dovrebbe inghiottire e che OpenAPI non ha modo di dichiararlo.

Non è una taratura di comodo: è la definizione di §5 applicata alla lettera.
`Shadow` significa «osservato e **assente dall'inventario dichiarato**», e
finché un dichiarato potrebbe coprirlo quell'assenza non è verificabile — lo
stesso ragionamento per cui §9 Fase 3 non dice mai `Shadow` senza specifica.

**Solo l'ultima posizione.** Una variabile in mezzo che si estende renderebbe la
forma ambigua da due lati insieme, e allargarla farebbe combaciare quasi tutto
con quasi tutto. §P3 applicato al confronto invece che alla normalizzazione.

- [x] Un endpoint **davvero** non documentato alla stessa profondità resta
      `Shadow` conclamato ed esce con `3` — è la proprietà che rende accettabile
      la correzione, e ha una fixture sua
- [x] Un dichiarato che finisce con un segmento **fisso** non si estende
- [x] Due valori fissi diversi nella testa: nessuna variabile finale rimedia
- [x] Una variabile che si estende **non copre** il dichiarato, che resta
      candidato zombie come per ogni match parziale
- [x] Vikunja rieseguito: **127 finding prima, 127 dopo, identici** — la
      correzione è inerte dove la famiglia non c'è

**Il prezzo, con una fixture che lo asserisce.** Una sottorisorsa non
documentata sotto una collezione dichiarata che finisce con una variabile —
`/api/users/1/comments/5` contro `/api/users/{id}` — diventa `Undetermined`
invece di `Shadow`, quindi **non fa più fallire una pipeline**. Resta segnalata,
e l'evidenza dice che servirebbe un `{id}` lungo tre segmenti. Se il prezzo
fosse troppo alto la leva è limitare quanti segmenti una variabile può
inghiottire: è una decisione del fondatore. Il test si chiama
`prezzo_noto_una_sottorisorsa_non_documentata_diventa_undetermined`, così il
giorno in cui si rivede fallisce invece di passare inosservato.

### Le due domande rimandate hanno una risposta

**Catalogo dei path sospetti:** il run di Gitea senza specifica produce **zero
finding su 118 endpoint reali**. Il catalogo non è mai scattato. Non fa rumore, e
non fa nemmeno segnale.

**Granularità per-metodo:** mai scattata su nessuno dei due corpus. La domanda
resta **non esercitata**, e serve traffico che contenga il caso.

### Le due cose che hanno retto

**Nessuna parola inghiottita dentro `{id}`**: la controprova sul log grezzo dà 0
segmenti persi su Gitea e 12 su Vikunja tutti ancora presenti nel report. La
sovra-aggregazione è il fallimento invisibile che il collaudo temeva di più, e su
traffico vero non è avvenuto. **Parsing:** zero righe scartate — merito del
proxy, non della grammatica, come già annotato.

## Taratura post-collaudo (2026-09-05)

Due correzioni, entrambe **specificate dal collaudo** e non inventate qui: la
prima ha per specifica la tabella delle sette varianti di COLLAUDO.md, la seconda
l'esempio reale che quel documento riporta. Nessuna delle due tocca §6.

### 1. La grammatica di log diventa dichiarabile

**Il problema misurato:** una variante di `log_format` su sette veniva accettata
— il `combined` nudo. Le altre sei sono configurazioni ordinarie, e su
un'installazione reale almeno una c'è quasi sempre. Il rifiuto era corretto
secondo §P2 e allo stesso tempo rendeva il tool inservibile dove serve.

**La soluzione, che è quella che COLLAUDO.md indicava:** non allentare la
grammatica, **dichiararla**. `log_format` (§7) accetta ora, oltre al nome
canonico, la direttiva `log_format` del proprio server. Le due si distinguono
senza ambiguità: una dichiarazione contiene `$`, un nome canonico no.

```bash
shadow access.log --log-format '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" $request_time'
```

- [x] Tutte e **sette** le varianti sono leggibili dichiarandole
- [x] Senza dichiarazione ne passa ancora **una sola**: il default non si è
      allargato di un millimetro, e un test lo presidia
- [x] Una dichiarazione ambigua (`$a$b`) o incompleta è rifiutata **prima** di
      aprire il file: nessun manifest, nessun verdetto (§7)
- [x] Gli escape copiati da `nginx.conf` (`\"`) sono rifiutati con l'istruzione
      su cosa fare, invece di produrre una grammatica che non combacia mai
- [x] Senza `$remote_user` nella dichiarazione l'auth è `NotObservable`, non
      `Absent` (§6) — è il punto in cui sarebbe stato più facile mentire
- [x] Le due grammatiche danno lo stesso risultato sulle stesse fixture, e un
      test lo verifica invece di affidarlo a un commento

**Perché il default di §7 sopravvive.** §7 lega il default a un vincolo
condizionale: *se la validazione si allentasse, il default va rimosso nello
stesso momento*. Qui non si allenta: senza dichiarazione l'unica grammatica
accettata resta il `combined`, e con una dichiarazione la grammatica dichiarata
è percorsa per intero, campo per campo. Non esiste, né prima né dopo, un
percorso in cui un campo viene letto dalla posizione sbagliata.
**Ratificato dal fondatore il 2026-09-05** (blueprint v1.9, decisione 34): non è
più un'inferenza dell'agente, è in §7 — vedi *Emendamento a §7*, sotto.

### 2. L'evidenza nomina il path dichiarato più vicino

**Il difetto misurato:** con più match parziali l'evidenza nominava il primo in
ordine di specifica. Per `/api/v1/repos/collaudo/billing-service/issues/{id}`
indicava `/api/v1/repos/{owner}/{repo}/issues/comments` mentre il vicino ovvio
era `/issues/{index}`. Verdetto giusto, spiegazione fuorviante — e in un report
la spiegazione è ciò su cui l'utente decide.

**La distanza, definita e non improvvisata.** Tutti i candidati parziali hanno lo
stesso numero di segmenti dell'osservato (`compare` scarta le lunghezze diverse),
quindi li distingue solo *dove* e *quanto* divergono: (1) quante posizioni
divergono, (2) a parità, quanto tardi comincia la divergenza — un prefisso che
regge più a lungo è un endpoint più imparentato. Pareggio finale rotto in ordine
alfabetico, così l'esito **non dipende dall'ordine della specifica** (§P4).

- [x] Sull'esempio documentato viene nominato `/issues/{index}`
- [x] Due specifiche con gli stessi endpoint in ordine diverso danno la stessa
      evidenza — prima no, ed era il difetto
- [x] Quando più dichiarati sono vicini uguali, **l'evidenza lo dice** invece di
      sceglierne uno in silenzio (§P9)
- [x] Il verdetto non cambia: un match parziale resta `Undetermined`
- [x] Corretta anche una frase che era diventata falsa nominando il più vicino:
      diceva «non dichiarati sul path più vicino» mentre il controllo guarda
      **tutti** i candidati parziali, che è la posizione conservativa (§P3)

### La revisione avversaria, e i sei difetti che ha trovato nella taratura stessa

Le fixture della taratura erano verdi — 133 verifiche — e la taratura era
**sbagliata**. Una revisione avversaria indipendente sul cambiamento appena
scritto, condotta contro gli invarianti del blueprint, ha confermato sei difetti
riproducibili, uno dei quali è per nome il «pericolo mortale» di §9 Fase 1.
È lo stesso precedente della bomba YAML annotato in COLLAUDO.md: le fixture
presidiano i fallimenti che chi le scrive ha già immaginato.

| # | Difetto | Perché era grave | Correzione |
|---|---|---|---|
| 1 | **Una variabile che chiude la grammatica si mangiava tutto il resto della riga.** La guardia di coda `rest.is_empty()` era strutturalmente inerte: per una dichiarazione che finisce con una variabile non poteva scattare mai | §P2 alla lettera. Dichiarando quattro campi su un log che ne ha sei, il file veniva **accettato**: `$request_uri` diventava `/api/users 0.042 10.244.1.7:8080`, due richieste identiche diventavano due endpoint distinti, e con una specifica a fianco uscivano `Shadow` inventati ed exit `3`. La mia stessa documentazione del modulo prometteva il contrario | Un campo **non racchiuso fra delimitatori** è separato dagli spazi, quindi non ne contiene: è la stessa semantica della grammatica preimpostata. Un campo racchiuso (`[$time_local]`, `"$request"`) continua a poterne contenere |
| 2 | **`$remote_user` vuoto diventava `Present`** | Un'autenticazione osservata **inventata dal nulla**, che abbassa la severità di uno `Shadow` da alta a media. La grammatica preimpostata non può produrre un campo vuoto; quella dichiarata sì | Vuoto e `-` significano la stessa cosa: `NotObservable` |
| 3 | **`-` accettato come metodo e come target** nel ramo a campi separati (`$request_method` + `$request_uri`) | `-` è un carattere ammesso in un metodo HTTP. Quando il client chiude prima di mandare la richiesta nasceva un endpoint `/-` mai esistito, e con una specifica uno `Shadow` che fa fallire una pipeline. Il ramo `$request` lo rifiutava già, e il commento accanto enunciava la regola che l'altro ramo non applicava | `-` non è una richiesta osservata, in tutti e due i rami |
| 4 | **`${nome` senza graffa di chiusura** diceva «un `$` senza nome al carattere 0» | Manda a cercare il problema dove non è, a chi il nome l'ha scritto | Errore proprio, che nomina la variabile e mostra le due forme valide |
| 5 | **L'evidenza del match parziale affermava un verso solo** | `PathMatch::Partial` è simmetrico, la frase no: diceva sempre «il dichiarato ha una variabile dove questo endpoint ha un valore fisso» anche quando era vero il contrario — cioè nel **caso centrale del prodotto**, un `/api/users/{id}` osservato contro un `/api/users/me` dichiarato. È lo stesso difetto che la taratura 2 doveva chiudere, in un'altra forma: la spiegazione asseriva invece di calcolare | Il verso si calcola (`divergence`), e ci sono tre frasi: osservato fisso, osservato variabile, e **tutti e due i versi** |
| 6 | **La protezione della barra verticale nelle tabelle Markdown copriva solo la configurazione** | Le due tabelle scoperte — endpoint e finding — sono proprio quelle che portano testo deciso da chi ha scritto le richieste. Chi legge non vede che manca un pezzo: vede una tabella storta e si fida | Protetta ogni cella che porta testo non deciso da noi |

Tutti e sei hanno ora una fixture che li presidia, e i nomi dei test dicono da
dove vengono (`avversaria_una_variabile_finale_non_si_mangia_i_campi_non_dichiarati`,
`quando_la_variabile_ce_lha_l_osservato_l_evidenza_lo_dice_nel_verso_giusto`, …).

**Riverificato eseguendo, dopo le correzioni:** lo scenario del difetto 1 esce
con `1` e stdout vuoto; la riga `-` del difetto 3 è scartata come
`invalid-request-line` mentre le altre proseguono; l'evidenza del difetto 5 dice
«has a fixed segment where this endpoint has a variable one»; e le **sette**
varianti restano leggibili dichiarandole.

### Un limite che resta, e non è un difetto da correggere

Un valore che contiene il **carattere delimitatore** sposta la lettura di tutti i
campi successivi. In un formato a barre verticali, una riga il cui URI contiene
`|` viene tagliata nel posto sbagliato. Non è aggirabile: un formato posizionale
non può rappresentare il proprio separatore dentro un valore, e nginx protegge
solo `"` e `\` (`escape=default`). Vale identico per la grammatica preimpostata,
dove un user agent con virgolette non protette produce lo stesso effetto.

Non l'ho «corretto» perché correggerlo significherebbe **indovinare** dove il
campo finisce davvero, che è precisamente ciò che §P2 vieta. È scritto nella
documentazione del modulo perché chi dichiara un formato a delimitatore
personalizzato sappia cosa sta comprando.

### Cosa è cambiato nel verdetto, misurato e non supposto

Rieseguito il corpus di Vikunja (l'unico intero) prima e dopo: **127 finding
prima, 127 dopo, zero evidenze cambiate, zero classificazioni cambiate, zero
severità cambiate**. La taratura è inerte dove non c'era niente da correggere —
che è come deve essere.

### `RulesetVersion` 0.3.0 → 0.4.0

Nessuna soglia di `ruleset.rs` è cambiata, eppure il numero sale, e il motivo è
la lettera di §7: la regola parla di *esito di un'analisi a parità di input*, e
un file che prima veniva rifiutato in blocco e ora viene analizzato è un esito
diverso sullo stesso input. Il precedente della v1.7 — *un avviso non è un
verdetto* — **non copre** il cambio di evidenza: parla degli avvisi, e
l'evidenza sta dentro il `Finding`. Minor, non major: i report vecchi restano
confrontabili con i nuovi.

### Emendamento a §7 — **ratificato** (blueprint v1.9, decisione 34)

Per un tratto il codice ha implementato la dichiarazione mentre il documento non
la conosceva, e §0 dice che i contratti si cambiano solo per revisione esplicita
del fondatore. Quel tratto è chiuso: **ratificato il 2026-09-05**, §7 lo dice, e
§13 registra chi l'ha deciso e perché.

Cosa dice ora §7, in sintesi: `log_format` accetta o un nome canonico o la
direttiva del proprio server, distinta perché contiene `$`; una grammatica
dichiarata è validata con la stessa strettezza di quella preimpostata; una
dichiarazione ambigua o incompleta ferma il run prima di aprire il file; il
valore preimpostato resta, perché il vincolo condizionale che lo giustificava
non è cambiato. Sono scritti in §7 anche il **limite** (un valore che contiene
il carattere delimitatore sposta la lettura, e non è aggirabile senza
indovinare) e il fatto che questo **non** decide quale Collector aggiungere per
secondo, che resta aperta in §3.

### Cosa NON è stato costruito, deliberatamente

Nessun secondo `Collector` nel senso di §3: la decisione «quale formato
aggiungere per secondo» **resta aperta**. Che il `common` e il `combined` di
Apache risultino esprimibili in questa sintassi è una conseguenza — sono gli
stessi campi nello stesso ordine, come COLLAUDO.md già annotava — non la scelta
di un secondo formato. Nessun preset è stato aggiunto: un catalogo di nomi
sarebbe stata proprio quella decisione, presa di nascosto.

Non è stata toccata l'inferenza dell'auth da un header: una dichiarazione che
porti `$http_authorization` **non** viene letta come autenticazione osservata.
Sarebbe un allargamento di §6, e §6 è congelato.

## Due difetti pre-esistenti, corretti su richiesta del fondatore (2026-09-05)

Trovati dalla revisione avversaria della taratura, non appartengono alle due
tarature, ed erano stati riportati **senza toccarli**. Il fondatore ha poi
chiesto di correggerli entrambi.

Correggendoli ne sono emersi altri quattro della stessa famiglia — un valore che
scivola sul default in silenzio, e una regola di redazione disattivata da
un'altra regola del tool — e **la prima versione della correzione 2 era
sbagliata**, smentita da una misura su un corpus che il collaudo non aveva. Sono
sei in tutto, e sotto ci sono tutti e sei.

### 1. Una variabile d'ambiente booleana non riconosciuta valeva `false`, in silenzio

`shadow-cli/src/config.rs`, `resolve_flag`, trattava come `false` **qualunque**
valore diverso da `1` o `true`. Quindi `SHADOW_FAIL_ON_UNDETERMINED=yes`
**spegneva** il gate invece di accenderlo, e il manifest registrava `false` da
`environment` senza un avviso: chi l'aveva scritto credeva di aver acceso una
condizione di fallimento in CI e aveva ottenuto il contrario. Falso negativo
silenzioso su una chiave di sicurezza — §P1 e §P2 insieme.

Ora un valore che non si riconosce **ferma il run**: uscita `1`, stdout vuoto,
messaggio che nomina la chiave e i valori ammessi. È la stessa disciplina che
`output_format` già applicava a un formato inesistente.

- [x] `true` / `1` accendono, `false` / `0` spengono, dall'ambiente come dal flag
- [x] `yes`, `on`, `TRUE`, `True`, `1 ` (con spazio) e la stringa vuota fermano
      il run invece di valere `false`
- [x] Nessun manifest e nessun verdetto quando il run si ferma (§7)
- [x] Vale per **ogni** chiave booleana, `SHADOW_SHOW_RAW_VALUES` compresa: la
      deroga a §P5 è la più pericolosa da credere accesa quando non lo è

**Ratificata dal fondatore il 2026-09-05 → blueprint v1.8, decisione 33.**
L'insieme dei valori canonici è `true`/`1` e `false`/`0`, **senza varianti di
maiuscole e senza sinonimi**: `TRUE` viene rifiutato, non tradotto. Non è più
un'inferenza dell'agente: è in §7, e la regola vale anche per una variabile
d'ambiente che esista ma non sia testo valido (vedi la 6, sotto).

### 2. La redazione si spegneva da sola sui path

`shadow-core/src/redact.rs`: un segmento è "opaco" solo se fatto **interamente**
di caratteri da token. Ma `shadow-core/src/inventory/path.rs`, `reencode`,
ri-codifica in `%XX` le barre, le graffe, i `%` e i caratteri di controllo dentro
**ogni** segmento canonico. Un escape in mezzo a un segmento **incollava** due
parti in una stringa che nessuna regola poteva più riconoscere.

Misurato prima di correggere: lo stesso segreto usciva mascherato senza barra e
in chiaro con una, e la barra è metà dell'alfabeto base64.

**La prima correzione era sbagliata, e a dirlo è stata una misura.** Avevo messo
`%` fra i caratteri ammessi: sembra la mossa ovvia, e sul corpus di Vikunja non
si vedeva niente (0% di mascheramento prima e dopo). Una revisione avversaria ha
costruito il corpus che quella misura non poteva contenere — path GitLab, dove
il progetto è un parametro con la barra codificata,
`/api/v4/projects/gruppo%2Fprogetto/...`, la forma che usano la documentazione
GitLab, `glab` e il provider Terraform — e ha misurato **il mascheramento
passare dallo 0% al 91,2%**, contro la soglia del 15% di COLLAUDO.md §2. Peggio
della percentuale: 186 pattern distinti collassavano su **sei** stringhe
identiche, e in Markdown la tabella produceva 31 righe byte-identiche di fila.

Il meccanismo è netto, e non l'avevo visto: **ogni escape porta per costruzione
una cifra**, e la cifra è metà della prova che `looks_like_secret` richiede.
Mettere `%` nell'alfabeto non aggiungeva un carattere: regalava la prova.

**Quella che è rimasta** tratta l'escape per quello che è — un separatore che il
tool ha codificato — e giudica **ogni parte per conto suo**, come `word` già fa
con una barra vera. Nessun cambio di soglia, nessun cambio di alfabeto:

- un segreto lungo resta riconoscibile anche con un escape dentro
  (`aB3dEfgH7jKlmN9pQr1sT2uVwXyZ%2FmN9pQr` → `<redacted>`);
- `acme-platform%2Fbilling-service-v2` resta leggibile, perché nessuna delle due
  metà è un blob;
- `%7Bcustomer_id%7D-2024-summary` resta visibile, e cioè resta visibile il
  segnale che `reencode` esiste apposta per mostrare — un client che manda il
  template invece del valore.

### 3. Il controllo `chiave=valore` girava prima di spezzare il path

Emerso verificando la correzione precedente. `word()` provava `chiave=valore`
sull'**intera parola** prima di spezzare sul `/`: un path che contiene una parola
sensibile — `session`, `token`, `auth` — diventava il "nome" della coppia, e il
valore mascherato era ciò che stava dopo il primo `=`, cioè quasi niente. Usciva
`/api/session/aB3dEfgH7jKlmN9pQr1sT2uVwXyZ=<redacted>`: **il segreto in chiaro e
la maschera sulla coda vuota.** Il commento del ramo sotto prometteva già
"segmento per segmento"; era il ramo sopra a non lasciarglielo fare.

### 4. La punteggiatura della frase smentiva il valore

`text` spezza in parole solo su spazi e tabulazioni, quindi un path citato dentro
l'evidenza di un finding arriva con la virgola della frase ancora attaccata — e
la virgola lo buttava fuori dall'alfabeto. **Lo stesso identico path usciva
`<redacted>` nella colonna del soggetto e in chiaro due caratteri dopo, sulla
stessa riga**, in terminale, JSON e Markdown. Ora la punteggiatura si toglie
prima di giudicare. Il `=` no: è il riempimento del base64, fa parte del valore.

### 5. Il nome del file arrivava intatto nel manifest

`Redactor::path` teneva `file_name()` alla lettera. La directory era mascherata e
il nome no, anche quando il nome era `mario.rossi@example.com.log` o
`session-<token>.log` — cioè proprio nel file che viaggia (§P5), mentre le stesse
stringhe dentro un path osservato erano mascherate. Ora il nome passa dalla
stessa regola, estensione compresa: si guarda anche il nome senza `.log`, o il
punto smentirebbe la stringa. Un nome ordinario continua a passare: quale file
sia stato analizzato è informazione d'audit legittima.

### 6. Una variabile d'ambiente non-UTF-8 valeva "default"

Stessa famiglia della 1, un passo più in là: `std::env::var` mette nello stesso
`Err` "non c'è" e "c'è ma non è testo valido". Un valore davvero impostato
scivolava sul default, e il manifest scriveva `default` su una chiave che
l'ambiente stava cambiando. Ora ferma il run, e il messaggio **non stampa il
valore**: non essendo UTF-8 non c'è modo di mostrarlo senza inventarselo.

### Quanto costa in leggibilità: misurato, e su due corpus

È la domanda che COLLAUDO.md §2 chiama *leggibilità sotto redazione*, soglia 15%.

| Corpus | pattern con `<redacted>` | note |
|---|---|---|
| Vikunja (reale) | 0 / 12 — **0,0%** | 127 finding, identici a prima, evidenza 0/243 |
| Fixture `nginx-combined` | 3 / 24 — **12,5%** | i tre stanno tutti in `secrets.log`, dove sono voluti |
| GitLab sintetico con `%2F` | 0 / 192 — **0,0%** | con la correzione sbagliata era **91,2%** |

### `RulesetVersion` 0.4.0 → 0.5.0

Per le correzioni 2, 4 e 5, e per la lettera di §7: fra le cose che stanno nel
ruleset l'elenco nomina **i pattern strutturali dei segreti**. Le correzioni 1,
3 e 6 non contribuiscono — la 1 e la 6 stanno nella lettura della
configurazione, che l'elenco non nomina, e la 3 è un ordine di valutazione che a
parità di input non cambia quali stringhe sono segrete.

### Due limiti che restano aperti, con un test ciascuno

Sono in `shadow-core/tests/redaction.rs`, e **asseriscono che un segreto esce in
chiaro**. Non è una svista: il giorno in cui il perimetro si decide, quei test
falliscono, ed è il modo giusto di accorgersene.

1. **Un segreto corto spezzato da un escape** resta in chiaro: le due metà non
   arrivano ai 24 caratteri. Chiuderlo vuol dire abbassare la soglia.
2. **Un segreto base64 senza nessuna cifra** resta in chiaro. Chiuderlo vuol dire
   togliere la richiesta della cifra, e mascherare le parole lunghe.

Tutti e due chiedono di **decidere il perimetro del `Redactor`**, che è la
domanda che la v1.7 ha deliberatamente rimandato al collaudo e che trovi qui
sotto in *Ancora aperto*. Le sei correzioni chiudono i punti in cui una regola
del tool ne disattivava un'altra; quanto in là debba arrivare la redazione è
un'altra domanda, e non l'ho decisa io.

## Materiale pubblico, preparato ma non pubblicato (2026-09-06)

Su richiesta del fondatore, **in inglese**: `README.md`, `docs/TUTORIAL.md` e le
immagini in `docs/images/` (cruscotto in tema chiaro e scuro, resi senza aprire
finestre sullo schermo; l'applicazione macOS, screenshot reale della build
definitiva).

**La lingua non è un'eccezione a §5, è §5.** Ciò che un utente legge è in
inglese; blueprint, PROGRESS e COLLAUDO restano in italiano, e il README lo dice
di sé in fondo così nessuno «lo corregge» fra sei mesi.

**Non è stato pubblicato niente, e non è un repository git.** Tre cose bloccano
la pubblicazione, e nessuna la decide l'agente:

1. **La licenza ha sette segnaposto `<DA DEFINIRE>`** — Licensor, Change Date,
   Change License, definizione di «uso aziendale», contatto commerciale. Una BSL
   con i buchi **non è una licenza**: chi scarica non ha nessun permesso, e su
   uno strumento di sicurezza è la prima cosa che qualcuno guarda.
2. **Il nome è provvisorio** (§3), con un conflitto mai verificato. Pubblicare
   su un registro pubblico lo prende davvero — è lo stesso argomento già scritto
   per npm in [INTERFACCIA.md](INTERFACCIA.md).
3. **`PILOT_DISCOVERY.md`** contiene percorsi assoluti con il nome utente. È un
   file di un'altra sessione, non del prodotto, e andrebbe escluso o ripulito.

**Sui pacchetti scaricabili**, e va detto chiaro: **non esiste nessuna
applicazione iOS**, e non è mai stata costruita — quella che c'è è **macOS**.
L'app macOS non è firmata né notarizzata, quindi scaricata da GitHub verrebbe
bloccata da Gatekeeper: serve un account sviluppatore Apple, che è del fondatore.
Per Linux non c'è un'applicazione ma il binario `shadow`, e compilarlo da questo
Mac richiederebbe una catena incrociata con un compilatore C per SQLite: la
strada normale è una pipeline di rilascio, che però esiste solo dopo il push.

## Ancora aperto

**Deliberatamente rimandate al collaudo** (v1.7): il **formato dell'allowlist**
e il **perimetro del `Redactor`**. Sono le due cose che un report vero risolve da
solo — il rischio "report illeggibile" si vede guardando, non ragionandoci sopra.

**Rimandate al collaudo dalla v1.6:**

- [ ] **Catalogo dei path sospetti** (`test`, `debug`, `old`, `internal`):
      quante volte scatta su traffico vero, e serve?
- [ ] **Granularità per-metodo**: se in un report vero un `DELETE` non
      documentato si perde in mezzo agli altri `Undetermined`, l'evidenza non
      basta e la questione torna su §6.

**Scelte da confermare, senza urgenza:**

- [ ] Identificatore stabile: digest SHA-256 del pattern troncato a 16 caratteri.
- [ ] Inventario indicizzato per pattern del path, non per identificatore.
- [ ] Nome delle variabili d'ambiente: `SHADOW_` + chiave di §7 in maiuscolo.
- [ ] Il livello "file di config" di §7 **non esiste ancora**: nessun formato è
      stato deciso. La precedenza implementata è flag > ambiente > default, e il
      manifest lo dichiara registrando la provenienza di ogni valore.
- [ ] **`InputDigest` ammette algoritmo dichiarato con digest vuoto.** La
      pipeline non lo produce mai; il tipo lo consente a chi lo costruisce a mano.

**Decisioni di §3 ancora aperte:** il **nome "Shadow"** (conflitto da verificare
prima di consolidare il branding), i **parametri BSL residui** (definizione di
"uso aziendale", Licensor, Change License, contatto), e **quale Collector
aggiungere per secondo** — che il collaudo potrebbe decidere da solo, se i log
veri risultano essere di un formato che non leggiamo.

## Log delle verifiche avversarie

### Fase 0 — 2026-08-29

Rischio da smentire (§9): *modelli dati incoerenti o duplicati fuori da `shadow-core`.*

| Controllo | Comando | Esito |
|---|---|---|
| Nessuna definizione di tipo fuori da `shadow-core` | `grep -rnE '^[[:space:]]*(pub )?(struct\|enum\|union\|trait\|type) ' shadow-collectors/src shadow-cli/src` | **0 risultati** |
| Nessuna definizione di tipo nei file di test | stesso grep su `shadow-core/tests` | **0 risultati** |
| Ogni record di §6 definito una volta sola | grep per `ObservedRequest`, `EndpointPattern`, `DeclaredEndpoint`, `Finding`, `RunManifest`, `Classification`, `EndpointId`, `RulesetVersion` | **1 definizione ciascuno**, tutte in `shadow-core/src/model/` |
| `shadow-core` senza I/O (§8) | `grep -rnE 'std::fs\|std::net\|File::\|reqwest\|tokio' shadow-core/src` | **0 risultati** |
| Verso delle dipendenze (§8) | `cargo tree --workspace -e normal` | `shadow-cli` dipende sia da `shadow-collectors` sia — direttamente — da `shadow-core`; `shadow-collectors` dipende da `shadow-core`; `shadow-core` non dipende da nessun crate del workspace. Nessun arco inverso, nessun ciclo |
| Cancello di cablaggio | esecuzione del binario `shadow` | versione, aiuto e contratto stdout/stderr verificati eseguendo |

**Conclusione: nessun crate oltre `shadow-core` definisce modelli dati propri.**

Da rieseguire a ogni fase: la duplicazione del modello è il tipo di errore che
entra silenziosamente quando un Collector ha "solo bisogno di una piccola
struttura di comodo".

### Allineamento alla revisione v1.1 — 2026-08-29

Rischi da smentire: (a) una decisione recepita solo a metà, cioè presente nel
codice ma non nel documento o viceversa; (b) un punto del blueprint rimasto in
contraddizione con le decisioni chiuse; (c) codice morto introdotto dalle
modifiche; (d) logica di Fase 1 anticipata.

Cinque lenti indipendenti hanno esaminato documento e codice, e ogni rilievo è
stato sottoposto a un revisore incaricato di **confutarlo**. Esito:

| Controllo | Esito |
|---|---|
| §6 campo per campo contro `shadow-core/src/model/` | nessuna discrepanza |
| Le 9 decisioni recepite sia nel testo sia nel codice | **4 recepimenti parziali trovati e corretti**: Fase 5 descriveva ancora l'auth come binaria; Fase 2 non nominava la regola di propagazione; §2 chiamava la BSL un "orientamento"; §13 non citava Fase 2 e Fase 5 |
| Nessuna logica di Fase 1 o successive | nessuna: nessun parser, nessun trait `Collector`, nessun flag operativo, nessuna lettura di file |
| §P7 — modello dati solo in `shadow-core` | confermato, invariato |
| §P8 — codice morto | **5 item speculativi trovati e rimossi** (vedi sotto) |
| Coerenza di `PROGRESS.md`, `LICENSE`, fixture | **3 imprecisioni corrette**: fixture di Fase 3 mancante, conta delle decisioni, verso delle dipendenze descritto per difetto |

Codice morto rimosso, ciascuno verificato irraggiungibile prima della rimozione
e con build verde dopo: `RulesetVersion::new` (serviva solo a rileggere manifest
passati, cioè Fase 5), `Display` per `EndpointId`, `Display` per
`Classification`, `Display` per `DigestAlgorithm` (tutti duplicati inutilizzati
di `as_str`), e la `[dev-dependencies] chrono` ridondante di `shadow-core`.
`Display` per `RulesetVersion` resta: lo usa `shadow-cli`.

Dopo le correzioni: `cargo build --workspace --all-targets` verde, `cargo test
--workspace` 6/6 verde, `cargo doc` senza warning, cancello di cablaggio
invariato (versione, aiuto, `0` senza argomenti, `2` su argomento non valido).

### Allineamento alla revisione v1.2 — 2026-08-29

Verifica eseguita dopo il recepimento, non a sensazione:

| Controllo | Comando | Esito |
|---|---|---|
| §6 è rimasto congelato | diff di §6 prima/dopo + conteggio dei campi per record | unica riga cambiata: gli **esempi** di evidenza, tradotti in inglese. Campi invariati: `ObservedRequest` 7, `EndpointPattern` 7, `DeclaredEndpoint` 3, `Finding` 6, `RunManifest` 5 |
| Nessuna stringa italiana verso l'utente | ispezione di `ABOUT`, `AFTER_HELP`, esempi di evidenza, valori di `output_format` | nessuna residua |
| Nessun codice di Fase 1 | `grep` su parser, trait `Collector`, flag operativi, calcolo di hash | nessuno: `--fail-on-undetermined` e il digest sono **scritti nel blueprint**, non nel codice |
| Build e test | `cargo build --workspace --all-targets`, `cargo test --workspace`, `cargo doc` | verdi, 6/6, zero warning |
| Cancello di cablaggio | esecuzione del binario | invariato e ora conforme al §7 riformulato |

### Fase 1 — 2026-08-30

I casi peggiori dell'ingestione (§9), ciascuno con una fixture che lo presidia e
resta verde a ogni `cargo test`:

| Rischio | Prova eseguita | Esito |
|---|---|---|
| **File da gigabyte**, memoria che cresce col file | `/usr/bin/time -l` sul binario release, stesso log a 11 MB e a 211 MB (la fixture ripetuta 20.000 e 400.000 volte) | RSS di picco **identico**: 6.520.832 byte in entrambi i casi. La memoria segue le richieste distinte, non la dimensione del file |
| **Riga corrotta a metà file** | `corrupted-midfile.log`: 6 righe, 3 valide, 3 rotte in tre modi diversi | 3 parsate, 3 scartate **con il perché**, la riga dopo quella rotta analizzata regolarmente |
| **`log_format` non riconosciuto** — il pericolo mortale: campi letti dalla posizione sbagliata | `unrecognised-format.log`, un log strutturato di altro tipo | Rifiuto con exit `1`, **stdout vuoto** (nessun manifest, nessun verdetto), messaggio che dice il formato atteso e come dichiararne un altro |
| **Encoding sporco** | `dirty-encoding.log`, byte non UTF-8 in mezzo | Riga scartata come `invalid-utf8`, nessun crash, e il digest resta quello del **file intero** — anche i byte scartati passano per l'hash |
| **Riga senza ritorno a capo** lunga 3× il limite (rischio non elencato in §9, trovato implementando) | File generato dal test | Scartata come `line-too-long`, la riga successiva analizzata: §P10 regge anche a un input costruito per aggirarlo |
| **File vuoto** | File a zero byte | Analisi legittima, manifest emesso, **avviso su stderr**: quasi sempre è il file sbagliato (§P1) |
| **Digest corretto** | Confronto con `shasum -a 256`, strumento indipendente | Identico su tutte le fixture |
| **Determinismo** (§P4) | Due run sullo stesso input, confronto del report riga per riga | Identici a parte il `run timestamp`, che §P4 ammette come unica differenza |
| **§P7 invariato** | Ispezione dei tipi definiti in `shadow-collectors` | `Collector`, `DiscardReason`, `IngestOutcome`, `IngestError` descrivono l'**ingestione**, non richieste o endpoint. Nessun record del modello duplicato |
| **§P2 strutturale** | Lettura della grammatica in `nginx.rs` | Ogni campo è letto dalla posizione che gli spetta o la riga cade: non esiste un ramo che restituisce una `ObservedRequest` con campi indovinati |

### Fase 2 — 2026-08-30

Il fallimento da presidiare qui non si vede: un endpoint fuso dentro un gruppo
non lascia traccia nell'output. Quasi tutte le prove servono a dimostrare che
qualcosa **non** è stato aggregato.

| Rischio | Prova eseguita | Esito |
|---|---|---|
| **Falso raggruppamento** — `/api/users/admin` che sparisce dentro `/api/users/{id}` | 5.000 identificatori numerici attorno a `admin`, `me`, `current` | I 5.000 collassano in un pattern; le tre parole restano **tre endpoint distinti e visibili**. Verificato anche end-to-end sul binario, con la fixture `admin-hidden.log` |
| Euristica troppo timida | `/api/users/1|2|3` e `/api/users/1/orders/91|92|93` | Collassano, anche annidati: `/api/users/{id}/orders/{id}` |
| Aggregazione sotto evidenza debole | Due soli identificatori distinti | **Non** si aggrega: §P3, nel dubbio si resta separati |
| Parole esadecimali corte (`cafe`, `dead`, `face`) | Tre segmenti esadecimali validi che sono parole | Restano tre endpoint: il catalogo delle forme tipo-ID non include le lunghezze brevi |
| Encoding: doppioni della stessa cosa | `/api/../secret` e `/api/%2e%2e/secret` | Un endpoint solo, due osservazioni |
| Encoding: segnale perso | `/api/%252e%252e/secret` | Resta **distinto**: decodificare a ripetizione cancellerebbe proprio ciò che si vuole vedere |
| Encoding: falso raggruppamento | `/api/a%2fb` contro `/api/a/b` | Restano due endpoint: uno ha un segmento, l'altro due |
| **Identificatore instabile** (§P4) | Due inventari costruiti da traffico diverso sullo stesso endpoint; e lo stesso endpoint con un metodo in più | Stesso identificatore in tutti i casi: dipende solo dal pattern |
| Auth propagata male | Pattern senza richieste osservabili; pattern con 2 osservabili su 5.000 | `NotObservable` nel primo caso (mai "senza auth"); nel secondo il verdetto è calcolato sulle sole osservabili e la base — 2 su 5.000 — è mostrata accanto |
| **§P10 dopo la Fase 2** | `/usr/bin/time -l`, stesso log a 11 MB e 211 MB (righe ripetute) | RSS di picco **identico**: 6.619.136 byte |
| §P10, il rovescio | Log di soli path distinti: 50k / 100k / 200k | RSS 72 / 133 / 260 MB, **lineare nei path distinti**. Il limite è annotato sopra fra le questioni aperte |
| §P7 invariato | Ispezione dei tipi definiti fuori da `shadow-core` | I tipi di `shadow-collectors` descrivono l'ingestione; la normalizzazione e l'inventario stanno nel core, dove §8 li vuole |

### Fase 3 — 2026-08-30

Il fallimento da presidiare qui è il `Known` sbagliato: un endpoint che il tool
dichiara a posto e che da quel momento nessuno guarda più.

| Rischio | Prova eseguita | Esito |
|---|---|---|
| **OpenAPI stantio o bugiardo** — un endpoint pericoloso sotto un prefisso dichiarato che passa per `Known` | `admin-hidden.log` contro `stale.json`, che dichiara solo `/api/users/{userId}` | `/api/users/admin` e `/api/users/me` escono **`Undetermined`**, con l'evidenza che dice perché non sono `Known`. Verificato sia in unità sia eseguendo il binario |
| Match troppo permissivo | Confronto per segmenti con tre esiti | Un valore fisso osservato dove la specifica ha una variabile **non** è un match: è il caso di mezzo, e resta `Undetermined` |
| Operazione non documentata nascosta in un endpoint documentato | Spec che dichiara `GET`, traffico che mostra anche `DELETE` | Non `Known`: `Undetermined` con i metodi non dichiarati nell'evidenza |
| **`Zombie` detto senza basi** | Finestra di 1 giorno contro staleness di 30 | Non si dice `Zombie`: si dice che la finestra è troppo corta. Con 90 giorni di traffico, invece, `Zombie` con esito `3` |
| **Rumore euristico** | Log tranquillo senza specifica; log con due path sospetti su quattro endpoint | Zero finding nel primo caso, **due** nel secondo: chi non ha niente da dire non produce righe |
| Euristica sull'auth sistematicamente sbagliata | Inventario con auth `NotObservable` | Nessun finding: l'euristica è spenta, come §9 impone |
| Euristica utile quando l'informazione c'è | Endpoint senza auth fra fratelli che ce l'hanno | Segnalato, severità alta, con l'evidenza che nomina i fratelli |
| Specifica illeggibile che produce verdetti a metà | YAML passato a `--openapi-spec` | Exit `1`, **stdout vuoto**, messaggio che dice come convertire. Nessun manifest, nessun verdetto |
| `Undetermined` che fa fallire una pipeline | Log con soli sospetti, con e senza `--fail-on-undetermined` | `0` senza il flag, `3` con il flag, **stesso report**: il flag decide se fallire, non cosa segnalare |
| Verdetto non ricostruibile | Manifest di un run con specifica | Due input, due digest: log e specifica, ciascuno col proprio algoritmo |
| §P4 | Classificazione ripetuta sullo stesso input | Finding identici, stesso ordine |

### Fase 4 — 2026-08-30

Il fallimento da presidiare qui è il report che diventa esso stesso una falla:
un file prodotto per essere condiviso, e che quindi verrà condiviso.

| Rischio | Prova eseguita | Esito |
|---|---|---|
| **Il report che perde segreti** | `secrets.log`: un JWT in un path, un'email con il dominio di un cliente, una chiave opaca, una password in querystring | Nessuno dei quattro compare nell'output di default, in nessun formato. Il report resta azionabile: gli endpoint si vedono, i valori no |
| Percorsi che rivelano cliente e struttura | Manifest, avvisi e **messaggi d'errore** | `/home/.../clienti/bancaXYZ/access.log` esce come `<path>/access.log`: quale file sia analizzato resta, dove stia no |
| Report grezzo indistinguibile da uno redatto | `--show-raw-values` | I valori escono in chiaro, e sia il report sia lo stderr lo dichiarano |
| Redazione così larga da rendere illeggibile il report | Path normali, evidenza, pattern con `{id}` | Restano leggibili: si maschera ciò che ha forma di segreto, non tutto |
| **Bomba di espansione YAML** | `bomb.yaml`: otto livelli di alias, ognuno che referenzia nove volte il precedente | Rifiutata **prima** del parser, exit `1`, stdout vuoto. La difesa non poteva stare dopo: quando il parser ha espanso, la memoria è già finita |
| Specifica enorme o troppo annidata | File oltre gli 8 MiB e oltre i 64 livelli | Rifiutati con l'indicazione del limite superato; la profondità è misurata **senza ricorsione** |
| **Allowlist che cancella l'evidenza** | Stesso run con e senza allowlist | Il report passa da 5 finding a 0, i conteggi del manifest restano identici: `Shadow 1`, `Known 1`, `Undetermined 3` |
| Regola di allowlist troppo ampia | `pattern /api/*` | Avviso su stderr con riga, regola e motivo: un solo segmento fisso prima del jolly |
| Allowlist senza versione | File senza intestazione | Rifiutato: un formato non versionato può essere reinterpretato in silenzio da una versione futura |
| **stdout inquinato** | `--output-format json` | stdout è JSON valido e nient'altro, verificato deserializzandolo; la diagnostica è su stderr |
| §P4 sul formato d'archivio | Due run, confronto del JSON con il timestamp azzerato | Identici |
| §6 congelato tranne la riapertura autorizzata | Conteggio dei campi per record | `ObservedRequest` 7, `EndpointPattern` 7, `DeclaredEndpoint` 3, `Finding` 6, `RunManifest` **6** — un campo in più, quello autorizzato dalla v1.6 |

### Taratura post-collaudo — 2026-09-05

Le fixture avversarie di questa taratura, tutte verdi, tutte in
`shadow-collectors/tests/log_format_dichiarato.rs` salvo dove indicato:

| Fixture avversaria | Cosa deve dimostrare | Esito |
|---|---|---|
| Dichiarazione con un campo **in più** rispetto al file | Il file non è quello: rifiuto, non lettura parziale (§P2) | verde |
| Dichiarazione con un campo **in meno** del file | Il resto della riga non viene ignorato in silenzio | verde |
| `$a$b` — due variabili adiacenti | Rifiuto **in compilazione**: non esiste modo di sapere dove finisce la prima | verde |
| Dichiarazione senza status / senza tempo / senza richiesta | Tutte e tre le mancanze dette insieme, non una per esecuzione | verde |
| Escape `\"` copiati da `nginx.conf` | Rifiuto con l'istruzione su cosa fare, invece di una grammatica che non combacia mai | verde |
| Dichiarazione **senza** `$remote_user` | Auth `NotObservable`, non `Absent` (§6) | verde |
| `$remote_user` valorizzato | Auth `Present` con schema non specificato, e il nome dell'utente **non** entra nel modello | verde |
| Le due grammatiche sulle stesse fixture | Stesso risultato campo per campo: non possono divergere in silenzio | verde |
| Le sette varianti senza dichiarazione | Ne passa ancora **una sola**: il default non si è allargato | verde |
| Copie delle fixture vs corpus del collaudo | Identiche byte per byte, o la fixture smette di presidiare la misura da cui nasce | verde |
| Dichiarazione ambigua passata alla CLI (`cancello.rs`) | Uscita `1`, **stdout vuoto**, nessun manifest e quindi nessun verdetto (§7) | verde |
| Combined dichiarato vs preimpostato, dalla CLI (`cancello.rs`) | Stesso report byte per byte, tranne timestamp e la voce `log_format` del manifest (§P4) | verde |
| Ordine della specifica invertito (`classification.rs`) | Stessa evidenza: la scelta del più vicino non dipende dall'ordine (§P4) | verde |
| Due dichiarati vicini uguali (`classification.rs`) | L'evidenza **ammette il pareggio** invece di sceglierne uno (§P9) | verde |

**Cancello di cablaggio, verificato eseguendo** (non a sensazione):

```
$ ./target/debug/shadow collaudo/corpus/varianti-log-format/03-combined-con-request-time.log
  exit=1, stdout=0 byte      <- prima e dopo la taratura, identico

$ ./target/debug/shadow …/03-combined-con-request-time.log --log-format '…$request_time'
  exit=0, observed endpoints (1), lines parsed 3, ruleset version 0.4.0
```

Il secondo comando è la prova di §P8: la grammatica dichiarata è raggiungibile
dall'entrypoint come la userebbe l'utente, non da un test soltanto.

### Due difetti pre-esistenti — 2026-09-05

Sei correzioni, tutte con la loro fixture. In `cancello.rs` quelle di
configurazione, in `redaction.rs` quelle di redazione.

| Fixture avversaria | Cosa deve dimostrare |
|---|---|
| `SHADOW_FAIL_ON_UNDETERMINED` con `yes`, `on`, `TRUE`, `True`, `1 `, `""` | Il run si ferma con `1` e stdout vuoto, invece di valere `false` in silenzio |
| Lo stesso gate con `true` e `false` dall'ambiente | Accende e spegne davvero, e il manifest registra la provenienza `environment` |
| `SHADOW_SHOW_RAW_VALUES` con un valore non riconosciuto | Vale per ogni chiave booleana, non solo per il gate |
| Una variabile d'ambiente non-UTF-8 | Ferma il run invece di scivolare sul default, e non stampa il valore |
| Segreto lungo con un escape in mezzo | L'escape non incolla due parti in un blob irriconoscibile |
| Path GitLab con `%2F`, e `%7Bcustomer_id%7D` | **Non** vengono mascherati: è la regressione da non rifare, misurata al 91,2% |
| Segreto in un path che contiene una parola sensibile | Non esce in chiaro come "nome" della coppia `chiave=valore` |
| Path citato dentro una frase, con la virgola attaccata | La punteggiatura non smentisce il valore |
| Nome del file che è un'email o un token | Non arriva intatto nel manifest; un nome ordinario sì |
| `token=abc123` dentro e fuori da un path | La coppia continua a funzionare: le correzioni non hanno rotto ciò che andava |
| **Due limiti noti**, che asseriscono un segreto in chiaro | Il giorno in cui il perimetro si decide, questi test falliscono: è il modo giusto di accorgersene |

**Verificato eseguendo:**

```
$ SHADOW_FAIL_ON_UNDETERMINED=yes ./target/debug/shadow …
  exit=1, stdout 0 byte
  shadow: SHADOW_FAIL_ON_UNDETERMINED must be one of: true, 1 (true) or false, 0 (false), got 'yes'.

$ SHADOW_LOG_FORMAT=<byte non UTF-8> ./target/debug/shadow …
  exit=1
  shadow: SHADOW_LOG_FORMAT is set but is not valid text, so its value cannot be read.

$ ./target/debug/shadow <log con un token base64 lungo e una barra codificata>
  /api/session/<redacted>

$ ./target/debug/shadow <log GitLab con 192 path %2F>
  0 pattern su 192 mascherati (con la correzione sbagliata: 186 su 204)
```
