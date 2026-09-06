Te lo do qui in chat come preferisci per le cose destinate a Claude Code. Una premessa onesta, visto che me l'hai chiesto esplicitamente: ci ho messo dentro anche cose che nella nostra conversazione non erano emerse ma che per un blueprint serio sono fondamentali (strategia di test, determinismo, contratto stdout/stderr, riproducibilità del report, no-telemetria, budget di memoria). Sono segnate come tali dove serve.

Il documento è pensato con due ossessioni in testa: (1) l'agente non deve mai costruire pezzi scollegati — per questo ogni fase ha un **cancello di cablaggio** con una prova concreta; (2) l'agente non deve perdersi — per questo c'è un vocabolario canonico e un meccanismo di stato da tenere aggiornato.

---

# SHADOW — Blueprint di progetto (v1.12)

## 0. Come Claude Code deve usare questo documento

Leggi **tutto** questo blueprint prima di scrivere una sola riga. Poi:

- Le **Fasi** si completano **in ordine**. Non iniziare una fase se il cancello di cablaggio della fase precedente non è verde.
- I contratti della **Fase 0** (modello dati, vocabolario, invarianti) sono **immutabili** salvo revisione esplicita del fondatore. Non reinterpretarli, non duplicarli, non creare varianti locali.
- Regola del **walking skeleton**: la Fase 1 deve produrre un percorso end-to-end funzionante anche se stupido (un log entra → attraversa tutta la pipeline → esce un risultato stampato). Le fasi successive **arricchiscono una pipeline già connessa**, non aggiungono moduli isolati.
- Regola **no dead code**: nessuna fase è completa se produce codice non raggiungibile dall'entrypoint della CLI. Un modulo che nessuno chiama = fase non finita.
- Mantieni un file **`PROGRESS.md`** con la checklist dei cancelli (vedi §11). All'inizio di ogni sessione di lavoro, **rileggi il vocabolario (§5) e `PROGRESS.md`** prima di procedere.
- Quando sei incerto su una decisione non coperta qui: **FERMATI e chiedi**. Non inventare in silenzio. Un'assunzione sbagliata su un tool di sicurezza è più costosa di una domanda.

---

## 1. Missione e non-obiettivi

**Missione (una frase):** dati i log di un web server o gateway, Shadow dice quali endpoint API stanno effettivamente rispondendo che non risultano documentati (**shadow**), e quali sono documentati ma non rispondono più (**zombie**).

**Non-obiettivi (cosa Shadow NON è — non costruire verso questo):**

- Non è protezione runtime in tempo reale. Non blocca traffico, non è un WAF, non è un IPS.
- Non è "il prossimo Akamai/Salt". Niente monitoraggio continuo multi-team come requisito del prodotto base.
- Non è una certificazione di sicurezza. È uno strumento di supporto alla discovery. Questo va scritto nella doc e riflesso nell'onestà dell'output.
- Non è un servizio cloud. Non chiama casa, non manda dati da nessuna parte (vedi P6).
- Non fa scanning attivo del target. Lavora su dati già prodotti (log), non genera traffico verso i sistemi analizzati (nella fase base).

---

## 2. Decisioni già prese (immutabili salvo revisione esplicita)

- **Linguaggio:** Rust. Struttura a **Cargo workspace** multi-crate.
- **Modello di licenza:** gratuito per uso individuale, a pagamento per uso aziendale/commerciale. La licenza è la **BSL (Business Source License) 1.1**, con finestra di conversione a 4 anni (v1.1); i parametri ancora da compilare sono elencati in §3. Il confine tecnico tra free e paid è: analisi single-shot vs servizio persistente + reportistica di compliance (vedi Fase 5).
- **Cinque funzioni core:** ingestione log → inventario osservato → confronto con dichiarato → euristiche senza dichiarato → output/report.
- **Direttiva prima (vedi P1):** sempre favorire il **falso positivo rumoroso** rispetto al **falso negativo silenzioso**.
- **Offline-first, zero telemetria.**
- **CLI-first.** Il demone + dashboard è posteriore (Fase 5).

---

## 3. Decisioni ancora aperte (da risolvere col fondatore, NON inventare)

- **Nome "Shadow":** possibile conflitto con prodotti esistenti. Da verificare prima di consolidare branding/README. Fino a decisione, "Shadow" è nome di lavoro.
- **Parametri BSL:** la finestra di conversione a licenza libera è **decisa: 4 anni** — che è anche il tetto massimo consentito dalla licenza stessa, la quale fa comunque scattare la conversione al quarto anniversario della prima distribuzione pubblica di ogni versione. **Restano aperti** e vanno definiti nel testo di licenza: la definizione precisa di "uso aziendale", il *Licensor*, la *Change License* (deve essere GPL v2.0 o successiva, o compatibile) e il contatto per le licenze commerciali.
- **Collector oltre nginx:** quale formato aggiungere per secondo (Apache, JSON di API gateway tipo Kong/Traefik, log cloud). Non deciso.

---

## 4. Principi / Invarianti globali (i pilastri — non violabili)

- **P1 — Direttiva prima:** in ogni punto ambiguo, preferisci segnalare in eccesso piuttosto che tacere. Un falso negativo silenzioso è il fallimento peggiore del prodotto.
- **P2 — Fallisci rumorosamente, mai "sbagliato in silenzio":** meglio un errore chiaro che un risultato plausibile ma inventato. Vale soprattutto per il parsing (P2 + Fase 1).
- **P3 — Conservativo di default:** non aggregare path nel dubbio; non classificare come "noto" nel dubbio. La cautela è la posizione di riposo.
- **P4 — Determinismo:** stesso input → stesso output. Due esecuzioni sullo stesso log producono lo stesso verdetto (a parte il timestamp del run). Requisito per test e per audit.
- **P5 — Redazione di default:** i valori che sembrano segreti/dati personali sono mascherati in **ogni** output e in **ogni** file persistito, salvo flag esplicito. Il report non deve mai diventare esso stesso una falla. **Nel perimetro rientrano anche i percorsi di filesystem** che finiscono nel report, nel manifest e nell'allowlist: un percorso come `/home/utente/clienti/bancaXYZ/logs/access.log` rivela il nome del cliente e la struttura interna dell'organizzazione, e viaggia dritto nel file che si consegna all'auditor. Un percorso è un dato sensibile quanto un token.

  **Un'eccezione mirata, e una sola:** i percorsi che l'utente ha scritto **sulla riga di comando in questa stessa invocazione** compaiono in chiaro nei messaggi diagnostici, perché mostrarglieli non gli rivela nulla che non abbia appena digitato — e un errore che dice `cannot read <path>/spec.json` non aiuta chi ha appena sbagliato a scrivere il percorso. Tutto il resto resta mascherato: percorsi derivati, percorsi letti da un file di config o da una variabile d'ambiente, e **qualunque cosa provenga dai dati analizzati**. L'eccezione vale per i **messaggi** (stderr), mai per il report e per il manifest: quelli sono i file che viaggiano.
- **P6 — Offline e nessuna esfiltrazione:** Shadow non apre connessioni di rete non richieste. Nessuna telemetria, nessun "phone home". Per un tool di sicurezza è un requisito di fiducia, non un dettaglio.
- **P7 — Singola fonte di verità per il modello dati:** i record canonici (§6) sono definiti **una volta** nel crate core e importati ovunque. Nessun modulo definisce la propria versione di `ObservedRequest`.
- **P8 — Nessun codice morto:** ogni componente costruito è raggiungibile dall'entrypoint. Se non è cablato, non è finito.
- **P9 — Onestà nell'output:** un caso incerto è etichettato come incerto (`Undetermined`), mai spacciato per verdetto sicuro.
- **P10 — Streaming a memoria limitata:** i log si leggono a flusso, riga per riga. La memoria usata non deve crescere con la dimensione del file.

---

## 5. Vocabolario canonico (termini + identificatori globali)

Questi sono i "nomi globali" del progetto. Usali **identici** in codice, commenti, output e doc. Coerenza terminologica = l'agente non si perde.

| Termine (identificatore canonico) | Significato |
|---|---|
| **Shadow** | Endpoint osservato nel traffico ma assente dall'inventario dichiarato |
| **Zombie** | Endpoint presente nel dichiarato ma non più osservato da oltre la finestra di staleness |
| **Known** | Endpoint osservato che combacia con uno dichiarato |
| **Undetermined** | Caso ambiguo: match parziale o incerto. **Categoria di prima classe**, mai "Known di default" |
| **AuthObservation** | Verdetto sull'autenticazione di un `EndpointPattern`: sì / no / mista / **non osservabile** |
| **NotObservable** ("non osservabile") | Il formato di log non trasporta l'informazione di autenticazione. **Non** equivale ad "assente": è l'assenza del dato, non l'assenza dell'auth |
| **ObservedRequest** | Unità atomica: una singola richiesta estratta da una riga di log |
| **EndpointPattern** | Endpoint logico dopo normalizzazione (es. `/api/users/{id}`) |
| **ObservedInventory** | Insieme degli `EndpointPattern` ricavati dal traffico |
| **DeclaredInventory** | Insieme degli endpoint dichiarati (da OpenAPI/Swagger fornito dall'utente) |
| **Finding** | Un risultato classificato con evidenza, severità, confidenza |
| **Classification** | Enum: `Shadow` / `Zombie` / `Known` / `Undetermined` |
| **Collector** | Modulo che traduce un formato di log specifico in `ObservedRequest` |
| **Redactor** | Componente che maschera segreti/PII prima di output e persistenza |
| **Allowlist** | Elenco locale di finding noti-innocui da silenziare (non cancellare) |
| **RunManifest** | Metadati del singolo run: versione tool, versione ruleset, hash input, conteggi, timestamp |
| **RulesetVersion** | Versione dell'insieme di regole/euristiche usato in un run |

**Convenzione di lingua:** la regola vale per gli **identificatori pubblici** e per il **testo rivolto all'utente**, che sono in **inglese** — i termini canonici qui sopra lo sono già, e devono uscire scritti esattamente così. **Non** vale per i nomi dei test, che sono documentazione interna a tutti gli effetti, né per commenti e documentazione, che restano in **italiano** aderenti a questo blueprint.

È testo rivolto all'utente anche ogni stringa che nasce nel codice e finisce in un output: l'**evidenza** dei finding, i **motivi di scarto** registrati nel manifest, e i **valori canonici** delle chiavi di configurazione.

---

## 6. Modello dati centrale (la spina dorsale — definito una volta, §P7)

Descrizione concettuale dei campi. **Nessun codice qui**: l'agente scrive le strutture, ma il *contenuto* è questo e non cambia.

**ObservedRequest** — cosa contiene ogni richiesta estratta:
- metodo HTTP
- path grezzo (così com'era nel log)
- mappa dei parametri di query (chiavi e valori grezzi, che poi passano dal Redactor). Una chiave che compare più volte conserva **tutti** i suoi valori: nessun valore viene scartato in silenzio (§P2)
- presenza e tipo dell'header di autenticazione — **mai il valore** del token. Gli stati sono **tre**, non due: *presente* (con lo schema osservato), *assente*, e **non osservabile** — cioè il formato di log non trasporta affatto l'informazione, come nel `combined` di nginx. "Non osservabile" non è una prova di assenza: confondere i due casi produrrebbe un dato inventato che sembra vero, esattamente ciò che §P2 vieta. Quando l'autenticazione è **osservata** ma il formato non dice **quale schema** fosse, lo schema è registrato come *non specificato*: il fatto (l'autenticazione c'era) si conserva, l'inferenza (era Basic) non viene spacciata per osservazione. Il campo si chiama `schema`, non `schema probabile`
- status code di risposta
- timestamp
- riferimento alla provenienza (file + numero di riga), per tracciabilità

**EndpointPattern** — l'endpoint logico:
- pattern del path normalizzato (con segmenti variabili resi espliciti, es. `{id}`)
- insieme dei metodi HTTP osservati su quel pattern
- primo e ultimo timestamp di osservazione (serve per lo zombie)
- conteggio osservazioni
- flag "autenticazione osservata": sì / no / mista / **non osservabile**. Regola di propagazione dalle richieste al pattern:
  - se **nessuna** richiesta del pattern è osservabile → il pattern è **non osservabile**;
  - se **almeno una** lo è → il verdetto (sì / no / mista) si calcola **sulle sole richieste osservabili**, ignorando le altre.
- **numero di richieste osservabili su cui il verdetto di autenticazione si basa**: va mostrato all'utente accanto al conteggio osservazioni, perché un verdetto costruito su 3 richieste osservabili su 5000 è debole e chi legge deve poterlo vedere (§P9)
- **identificatore stabile** dell'endpoint (deve essere lo stesso tra run diversi sullo stesso endpoint — serve per riconoscere "già visto" e per l'allowlist)

**DeclaredEndpoint** — l'endpoint atteso, estratto dall'OpenAPI:
- pattern del path
- metodi dichiarati
- schemi di autenticazione dichiarati: un **insieme**, perché una specifica può dichiararne più d'uno sullo stesso endpoint e scartarne uno sarebbe una perdita silenziosa (§P2). Insieme vuoto = nessuno schema dichiarato, che è un'informazione, non un dato mancante

**Finding** — il risultato:
- riferimento all'`EndpointPattern`, tramite il suo **identificatore stabile**; per gli zombie, riferimento al `DeclaredEndpoint` tramite **pattern del path + metodi** — il dichiarato non ha un identificatore stabile, e non se ne inventa uno ora: se servirà, emergerà in Fase 3
- `Classification`
- livello di **confidenza** (alta/media/bassa)
- flag `is_ambiguous`
- **evidenza** (perché è stato classificato così). L'evidenza è testo che l'utente legge, quindi in inglese (§5): es. `"not present in the OpenAPI spec"`, `"no auth observed while sibling patterns require it"`, `"path suggests a test environment"`
- severità suggerita, su scala a **tre livelli: bassa / media / alta**. Nessun altro livello, né sopra né sotto. Come si compone: `Shadow` parte da media e sale ad **alta** se l'endpoint risponde **senza autenticazione osservata** (non documentato e senza auth è la combinazione peggiore); `Zombie` è **bassa**, perché è igiene e non esposizione; qualsiasi finding sale di un livello se sull'endpoint compaiono **metodi osservati e non dichiarati**

**RunManifest** — la carta d'identità del run (requisito non discusso prima, critico per audit e determinismo):
- versione di Shadow
- `RulesetVersion`
- hash dei file di input (log e OpenAPI): **SHA-256 calcolato sul contenuto del file**, con l'**algoritmo registrato accanto al digest** dentro il manifest — così un digest non può mai essere letto senza sapere come è stato prodotto
- conteggi: righe totali, righe parsate, **righe scartate e perché**, endpoint trovati, finding per categoria
- **configurazione effettiva del run**: ogni chiave di §7 con il valore **realmente usato** e, per ciascuna, **da dove è arrivato** — flag della riga di comando, variabile d'ambiente, file di config, oppure default. La provenienza conta quanto il valore: "90 giorni perché c'era un file di config che nessuno ricordava" è informazione d'audit, non un dettaglio. Senza questo campo, due report divergenti sullo stesso servizio non permettono di distinguere se è cambiato il traffico o la configurazione — lo stesso buco che l'hash degli input aveva chiuso per i file
- timestamp del run

---

## 7. Costanti e configurazione globali

**Chiavi di configurazione** (nomi canonici, usati identici in CLI/config/env):
- `log_format` (formato atteso; se non riconosciuto → errore chiaro, vedi Fase 1). Accetta **o** un nome canonico **o** la direttiva `log_format` del server che ha scritto il log, riconoscibile perché contiene `$` — nessun nome canonico ne contiene, quindi una sola chiave porta le due cose senza che un valore possa essere letto per l'altra. Una grammatica **dichiarata** è validata con la stessa strettezza di quella preimpostata: o la riga combacia per intero e ogni campo è letto dalla posizione che la dichiarazione gli assegna, o la riga è scartata con il proprio motivo. Una dichiarazione ambigua (due variabili senza separatore) o incompleta (senza richiesta, status o tempo) **ferma il run prima di aprire il file**: una grammatica che non sa dove finisce un campo non deve arrivare ai dati. **Ha un valore preimpostato, e lo può avere solo perché la validazione è stretta**: un formato che non combacia esce con `1` e stdout vuoto, quindi il default non può produrre un'analisi sbagliata in silenzio. È un vincolo condizionale: **se la validazione si allentasse, il default va rimosso nello stesso momento** — e la dichiarabilità **non** lo allenta, perché senza dichiarazione l'unica grammatica accettata resta quella preimpostata.

  Un limite resta, e va detto a chi dichiara: un valore che contiene il **carattere delimitatore** sposta la lettura dei campi successivi. Non è aggirabile — un formato posizionale non può rappresentare il proprio separatore dentro un valore, e nginx protegge solo `"` e `\` — e cercare il delimitatore "giusto" più avanti significherebbe indovinare dove il campo finisce, che è ciò che §P2 vieta. Vale identico per la grammatica preimpostata.

  Rendere dichiarabile la grammatica **non** decide *quale Collector aggiungere per secondo* (§3), che resta aperta: che il `common` e il `combined` di Apache risultino esprimibili in questa sintassi è una conseguenza — sono gli stessi campi nello stesso ordine — non la scelta di un secondo formato.
- `openapi_spec` (percorso del file dichiarato, opzionale)
- `zombie_staleness_days` (finestra oltre cui un endpoint dichiarato ma non visto diventa zombie)
- `redaction_enabled` (default: acceso)
- `allowlist_path`
- `output_format` (`terminal` / `json` / `markdown` — valori canonici in inglese, §5: è testo che l'utente digita)
- `fail_on_undetermined` (default: **spento**; se acceso, anche i soli `Undetermined` fanno uscire con il codice dei finding — vedi sotto)
- `history_path` (percorso dello storico persistente, opzionale; assente = nessuno storico, e Shadow si comporta esattamente come prima). **È un percorso, non un piccolo linguaggio:** un valore che comincia per `file:` viene rifiutato, perché SQLite lo leggerebbe come URI e potrebbe aprire un database che vive solo in memoria — il tool direbbe di ricordare senza ricordare niente, e ogni run troverebbe tutto nuovo
- `target` (il servizio a cui il run appartiene; gli storici di bersagli diversi non si mescolano mai. Default: `default`)
- `path_prefix` (quale parte del traffico è l'API; assente = tutto). Un log che porta sia pagine web sia chiamate API fa risultare **ogni pagina** un endpoint non documentato, perché la specifica descrive solo l'API. Le richieste fuori dal prefisso **non spariscono**: non vengono osservate, vengono **contate**, e il conteggio si dice — su stderr e nel report. Il valore preimpostato è «analizza tutto», perché scegliere da soli quale traffico sia l'API significherebbe decidere di non guardare qualcosa senza dirlo, e §P1 lo vieta. Un prefisso che non comincia con `/` è rifiutato: ridurrebbe l'analisi a zero in silenzio

**Valori canonici delle chiavi booleane** (`redaction_enabled`, `fail_on_undetermined`): `true` / `1` per acceso, `false` / `0` per spento, scritti esattamente così — **nessuna variante di maiuscole, nessun sinonimo**. Un valore diverso **ferma il run** con `1` e stdout vuoto; non vale `false`.

È la stessa disciplina di `output_format`, che non accetta `JSON`, e ha una ragione più forte: un valore che significasse `false` in silenzio spegnerebbe un interruttore mentre chi l'ha scritto crede di averlo acceso. Su `fail_on_undetermined` — che è un gate di CI — quello è esattamente il falso negativo silenzioso che §P1 chiama il fallimento peggiore del prodotto. Indovinare che `yes` volesse dire `true` sarebbe interpretare al posto dell'utente su una chiave di sicurezza.

Vale allo stesso modo per una variabile d'ambiente che **esista ma non sia testo valido**: si rifiuta, non si ignora. Scivolare sul default farebbe scrivere `default` nel manifest su una chiave che l'ambiente stava davvero cambiando, e il manifest è la carta d'identità del run (§6).

**Precedenza di configurazione:** flag CLI > variabile d'ambiente > file di config > default. Documentata e implementata così, senza eccezioni.

**Schema del report esportabile — contratto pubblico versionato.** Il report JSON si dichiara `shadow-report/1` nel campo `schema`. È costruito esplicitamente e **non** derivato dalla forma interna dei record: legare lo schema pubblico ai tipi del core significherebbe che una rifattorizzazione interna diventa un cambio di contratto per chi consuma il file.

Vale la stessa regola dei motivi di scarto: **aggiungere campi è consentito, rinominarli o rimuoverli richiede `shadow-report/2`.** Chi costruisce un'automazione sopra questo file deve poter sapere, dal file stesso, se le sue assunzioni reggono ancora.

Il layout **Markdown** non è invece un contratto: è presentazione per esseri umani, e può cambiare senza versione. Chi automatizza usa il JSON.

**Motivi di scarto delle righe — catalogo congelato.** §6 rende "righe scartate **e perché**" parte del contratto del `RunManifest`. Questi sono i motivi, e diventano chiavi dentro un file d'audit:

| Chiave | Quando |
|---|---|
| `blank-line` | Riga vuota o di soli spazi |
| `invalid-utf8` | La riga non è UTF-8 valido; non si tenta nessuna conversione tollerante |
| `line-too-long` | La riga supera la lunghezza massima (vedi soglie globali) |
| `malformed-line` | La riga non combacia con la struttura del formato |
| `invalid-request-line` | La richiesta fra virgolette non ha la forma `METODO PATH HTTP/versione` |
| `invalid-status-code` | Il campo dello status non è un codice HTTP |
| `invalid-timestamp` | Il campo del tempo non è una data valida nel formato atteso |

**Sono contratto congelato allo stesso titolo di §6**: aggiungere nuovi motivi è consentito, **rinominare o rimuovere quelli esistenti no** — invaliderebbe i manifest già prodotti, cioè renderebbe illeggibile un archivio d'audit.

**Contratto codici di uscita** (serve per integrazione in CI — valori fissati, non più negoziabili):
- `0` = **analisi eseguita e completata**, nessun finding `Shadow`/`Zombie`
- `1` = errore di esecuzione (input illeggibile, formato non riconosciuto)
- `2` = errore d'uso (argomenti della riga di comando non validi)
- `3` = analisi eseguita e completata **ma** trovati finding `Shadow`/`Zombie` (così una pipeline CI può fallire di proposito)

**Vincolo: nessun manifest, nessun verdetto.** Il `RunManifest` esiste **se e solo se** un'analisi è stata eseguita, e un codice di uscita con **semantica di finding** (`0` e `3`) è emesso **solo insieme a un manifest**. Un'invocazione che non analizza nulla — `--help`, `--version` — termina con successo ma **non** afferma "nessun finding": non avendo prodotto un manifest, non ha prodotto nessun verdetto. Senza questo vincolo una pipeline CI che invochi Shadow male leggerebbe "analisi completata, nessun finding", cioè il falso negativo silenzioso che §P1 chiama il fallimento peggiore del prodotto.

**Gli `Undetermined` non attivano il `3`.** Il `3` scatta solo su `Shadow`/`Zombie` **conclamati**, cioè su un confronto con un inventario dichiarato. Un run senza OpenAPI produce solo `Undetermined` (vedi Fase 3) e quindi non fa fallire nessuna pipeline: **non è un difetto, è la posizione corretta**. Un run senza dichiarato non ha basi per bocciare una build; chi vuole un gate vincolante in CI fornisce l'OpenAPI, ed è il prezzo giusto per avere un verdetto che vincola.

Via d'uscita esplicita per chi vuole comunque il gate sui sospetti: `fail_on_undetermined` (`--fail-on-undetermined`). **Opt-in, mai default.** L'utente lo accende sapendo che sta trasformando dei sospetti in una condizione di fallimento.

**Contratto stdout/stderr** — il criterio non è "machine-readable contro leggibile", è **chi ha chiesto cosa**:
- **stdout** = ciò che l'utente ha **chiesto**: il report (JSON/Markdown), ma anche l'aiuto di `--help` e la versione di `--version`. Sono output richiesti esplicitamente, non diagnostica.
- **stderr** = tutto ciò che l'utente **non** ha chiesto: progressi, avvisi, conteggi diagnostici, messaggi d'errore, e l'aiuto stampato di sua iniziativa dal tool quando l'invocazione è priva di argomenti.
- Motivo, in entrambe le direzioni: `shadow ... > report.json` deve produrre un file pulito, **e** `shadow --help | less` deve funzionare. Mandare l'aiuto richiesto su stderr romperebbe ogni uso normale di un tool a riga di comando.

**Soglie/pattern globali** (definiti in un unico posto, versionati come parte del `RulesetVersion`).

Il criterio per stare in questo elenco è uno: **se cambiarla cambia l'esito di un'analisi, è parte del ruleset**.

**Quando cambia la `RulesetVersion`.** Cambia quando cambia **qualsiasi cosa che possa alterare l'esito di un'analisi a parità di input**: soglie, catalogo delle forme tipo-ID, euristiche, motivi di scarto. **Non** cambia per rifattorizzazioni, documentazione, o modifiche alla sola presentazione dell'output — un'ottimizzazione che dà lo stesso verdetto usando meno memoria non è un cambio di ruleset. Schema: **minor** per aggiunte e modifiche di soglie, **major** solo se un cambiamento rende i report vecchi non più confrontabili con i nuovi.

**Precedente, perché non si ridiscuta ogni volta: un avviso non è un verdetto.** Le soglie che governano *avvisi* — quando segnalare che una regola di allowlist è troppo ampia, quando dire che un file è vuoto — **non** fanno parte del ruleset e non fanno salire la `RulesetVersion`. Cambiarle cambia ciò che l'utente legge, non ciò che il tool ha concluso: a parità di input i verdetti restano identici, e un archivio di report resta confrontabile. La regola parla di *esito dell'analisi*, e un avviso non lo è. Due run sullo stesso input devono poter divergere solo se il ruleset è cambiato, e il manifest deve permettere di ricostruire perché (§P4).

- **lunghezza massima di una riga di log** oltre cui la riga viene scartata come `line-too-long`: **64 KiB** (Fase 1). Non è un dettaglio di implementazione: decide cosa viene scartato, ed è ciò che impedisce a un file senza ritorni a capo di aggirare §P10.
- **righe non vuote esaminate per riconoscere il formato** prima di dichiararlo non riconosciuto: **100** (Fase 1).
- soglia di cardinalità oltre cui un segmento di path è considerato variabile: **almeno 3 valori distinti** conformi a un tipo tipo-ID **nella stessa posizione e sotto lo stesso prefisso** (Fase 2)
- **catalogo delle forme "tipo-ID"** riconosciute per la normalizzazione (Fase 2): interamente numerico; UUID; stringa esadecimale di lunghezza 24, 32, 40 o 64. Il catalogo è deliberatamente corto: ogni forma aggiunta è un modo in più di inghiottire una parola scambiandola per un identificatore (§P3)
- **limiti sulla lettura della specifica dichiarata** (Fase 3/4): dimensione massima del file e profondità massima di annidamento, oltre cui la specifica viene **rifiutata rumorosamente**. La specifica è input non fidato quanto un log, e il parser YAML in particolare è un vettore noto: le **bombe di espansione** (anchor e alias che si moltiplicano a ogni livello) fanno esplodere la memoria a partire da poche righe di file. Un tool di sicurezza che ci cade è un imbarazzo, quindi gli alias YAML sono **rifiutati**, non limitati: in OpenAPI il riuso si fa con `$ref`
- elenco dei nomi di campo considerati "sensibili" per la redazione (`token`, `key`, `session`, `password`, `auth`, ecc.), i pattern strutturali dei segreti, e la regola di mascheramento dei **percorsi di filesystem** richiesta da §P5 (Fase 4)
- pattern dei path "sospetti" per le euristiche (`/test/`, `/debug/`, `/old/`, `/internal/`, ecc.) (Fase 3)

---

## 8. Struttura del repository (Cargo workspace)

Regola di separazione netta, da rispettare per non finire in un monolite ingarbugliato:

- **`shadow-core`** — il motore. Contiene il modello dati (§6), la normalizzazione, la classificazione. **Zero I/O di rete, zero lettura file, zero dipendenze da CLI.** È una libreria pura e testabile in isolamento. È il cuore del valore: va tenuto il più curato.
- **`shadow-collectors`** — i moduli che leggono formati di log specifici e li traducono in `ObservedRequest`. Qui vive l'I/O di lettura del **traffico osservato**. Progettato a plugin dietro un'unica interfaccia (contratto Collector), così aggiungere un formato non tocca il core.
- **`shadow-spec`** — legge la specifica **dichiarata** (OpenAPI/Swagger fornito dall'utente) e produce `DeclaredEndpoint`. È il simmetrico di `shadow-collectors`, sull'altra metà del confronto. Ha un crate suo per tre ragioni, tutte e tre strutturali:
  - `shadow-core` non può fare I/O, e quel vincolo non si tocca;
  - `shadow-collectors` è definito sui **formati di log**, e un OpenAPI non è traffico osservato: è la fonte dell'inventario dichiarato, concettualmente l'opposto. Mescolarli confonderebbe proprio la distinzione osservato/dichiarato che è il cuore del prodotto;
  - `shadow-cli` è orchestrazione: metterci dentro il parsing di un formato complesso lo trasformerebbe in un contenitore di logica, che è il modo in cui i progetti degenerano.
- **`shadow-cli`** — l'orchestratore: legge la config, invoca il collector giusto, passa i dati al core, applica il Redactor, produce l'output. È l'entrypoint. Tutto deve essere raggiungibile da qui (§P8).
- **`shadow-history`** — lo **storico persistente** su SQLite (Fase 5): quali endpoint sono già stati visti, su quale bersaglio, in quale run. Non classifica e non corregge: conserva ciò che un run ha concluso e riconosce ciò che non c'era. Ha un crate suo perché non appartiene né alla CLI né al demone: **entrambi** lo scrivono, e metterlo nel demone farebbe dipendere la CLI dal demone. Lo schema evolve per **migrazioni versionate** in `PRAGMA user_version`: si aggiunge in coda, non si modifica una migrazione già rilasciata, e una versione che questo binario non conosce ferma l'apertura invece di essere interpretata a caso — un archivio d'audit già scritto non si riscrive, come i motivi di scarto e lo schema del report (§7).
- **`shadow-view`** — dallo storico alla **vista di supervisione**: cosa è cambiato, cosa è aperto, cosa è noto, come si è mosso. Terminale, pagina HTML, server e applicazione macOS mostrano **questa** struttura e non ne calcolano una propria: se ognuna calcolasse la sua, prima o poi direbbero numeri diversi sullo stesso storico. **Zero rete.** `[DA RATIFICARE]` — §8 non lo prevedeva.
- **`shadow-serve`** — **la sola cosa di questo progetto che apre un socket**, e sta da sola apposta: così la superficie di rete si legge tutta in un file. In **sola lettura** — prendere in carico un alert resta una cosa del terminale — in ascolto su `127.0.0.1` se non si dice altro, e lo dice a voce alta quando l'indirizzo è raggiungibile da fuori. Scritto sulla libreria standard: per servire una pagina non serve un framework, e ogni dipendenza è una catena di fornitura in più da difendere. `[DA RATIFICARE]` — §8 non lo prevedeva, **e cambia il senso della decisione 36** (vedi §13, v1.13).
- **`shadow-daemon`** — l'**esecuzione continua**: quando guardare, quante volte, quando fermarsi. Nient'altro: la pipeline resta quella delle Fasi 1–4 e l'orchestrazione di `shadow-cli`, perché un demone con dentro una copia della pipeline è il modo più rapido di far divergere i verdetti di `shadow` da quelli di `shadow daemon`.
- **`shadow-compliance`** — il **report d'audit** esportabile: l'inventario degli endpoint con classificazione e stato dell'autenticazione, in CSV, Markdown e JSON (`shadow-compliance/1`, identificatore distinto da `shadow-report/1` perché è un documento diverso con un pubblico diverso). **Si chiamava `shadow-report` fino al 2026-09-05**, quando il fondatore ha sciolto la collisione con l'identificatore dello schema JSON `shadow-report/1` di §7: lo schema è un contratto pubblico congelato con consumatori possibili, il nome del crate è interno e non ne aveva nessuno, quindi si è spostato il lato che non costava niente.
- **`tests/fixtures/`** — il corpus di test dorato (log di esempio + output atteso). Cresce a ogni fase (§10).

---

## 9. LE FASI

### Fase 0 — Fondamenta e contratti (nessuna logica di analisi)

**Obiettivo:** mettere in piedi lo scheletro del workspace e fissare i contratti globali, senza ancora analizzare nulla.

**Deliverable:**
- Workspace Cargo con i crate `shadow-core`, `shadow-collectors`, `shadow-cli` (vuoti ma compilanti).
- Definizione del modello dati canonico (§6) in `shadow-core`.
- Definizione del vocabolario (§5) come riferimento nel codice.
- Scheletro di `RunManifest`.
- File di licenza (BSL, con parametri segnaposto se non ancora decisi — vedi §3).
- `PROGRESS.md` inizializzato con la checklist dei cancelli.

**Cablaggio (cancello):** il workspace compila; la CLI si avvia e stampa versione + aiuto. Nessuna funzionalità, ma la spina dorsale del progetto esiste ed è coerente.

**Criteri di riuscita:**
- [ ] `cargo build` verde su tutto il workspace
- [ ] Il modello dati è definito **una sola volta** e in `shadow-core`
- [ ] `PROGRESS.md` esiste e riflette lo stato

**Verifica avversaria:** a questo stadio il rischio è definire modelli dati incoerenti o duplicati. Controllo: nessun crate oltre `shadow-core` definisce record propri per rappresentare richieste/endpoint.

---

### Fase 1 — Walking skeleton: ingestione nginx end-to-end

**Obiettivo:** far attraversare a un log l'**intera pipeline**, anche in modo minimale. Un log nginx entra, viene parsato in `ObservedRequest`, produce un `Finding` banale (anche solo "endpoint osservato"), e viene stampato. Da qui in poi la pipeline è **connessa** e le fasi dopo la ingrossano.

**Deliverable:**
- Lettore **a flusso** (mai tutto in memoria).
- Parser nginx (primo Collector) dietro l'interfaccia Collector.
- Validazione del formato all'avvio.
- Gestione righe malformate: skip + conteggio + registrazione nel `RunManifest`.
- **Digest SHA-256 degli input** (§6), calcolato **mentre il file viene letto a flusso** — non con una seconda passata, che violerebbe §P10 — e registrato nel `RunManifest` insieme al proprio algoritmo. Sta qui e non in Fase 4 perché un manifest che tace quale file ha analizzato viola la propria ragione d'essere: rimandarlo renderebbe non riproducibili proprio i primi report prodotti.
- Output minimale in terminale.

**Cablaggio (cancello — la cosa che dimostra che non hai lasciato pezzi scollegati):** eseguire la CLI su un file di log nginx reale produce, end-to-end, un elenco stampato di endpoint osservati **e** un `RunManifest` con i conteggi (righe totali/parsate/scartate) **e il digest SHA-256 del file di input**. Se questo comando gira e produce output coerente, la pipeline è cablata dall'inizio alla fine, e il primo report è già riproducibile.

**Criteri di riuscita:**
- [ ] Un log nginx valido produce endpoint osservati + manifest
- [ ] Il manifest riporta il digest SHA-256 dell'input, con l'algoritmo accanto (§6)
- [ ] Il digest è calcolato nella stessa passata della lettura: memoria piatta, nessuna seconda lettura del file (§P10)
- [ ] Memoria stabile a prescindere dalla dimensione del file (§P10)
- [ ] Le righe malformate vengono contate, non fanno crashare il run

**Verifica avversaria (i casi peggiori della ingestione):**
- **File da gigabyte:** deve reggere con memoria piatta. Prova: dai in pasto un file molto grande (anche generato ripetendo righe) e verifica che la RAM non salga con la dimensione.
- **Righe corrotte a metà file:** una riga bacata non deve far perdere l'analisi delle altre. Skip + conteggio, sempre avanti.
- **`log_format` personalizzato:** il pericolo mortale è estrarre **silenziosamente** i campi dalla posizione sbagliata (es. leggere lo status code dove c'è altro) e produrre numeri inventati che *sembrano* veri. Difesa (§P2): o il formato è riconosciuto, o il tool **si rifiuta con un errore chiaro** / richiede che l'utente dichiari il formato. Mai "best effort" silenzioso su dati di sicurezza.
- **Encoding sporco / byte non validi:** trattati come riga malformata, non crash.

---

### Fase 2 — Inventario osservato + normalizzazione dei path

**Obiettivo:** raggruppare le `ObservedRequest` in `EndpointPattern`, riconoscendo i segmenti variabili (ID) senza fondere cose diverse.

**Deliverable:**
- Logica di normalizzazione conservativa dei path.
- Costruzione dell'`ObservedInventory`.
- Assegnazione dell'**identificatore stabile** all'endpoint.

**Come si decide, e quando.** La regola chiede *almeno* N valori distinti tipo-ID: **oltre l'N-esimo la decisione non cambia più**. Quindi la decisione si prende **durante la lettura**, non in una seconda passata sull'insieme completo: ogni posizione conserva al massimo i valori tipo-ID visti finora e, appena arrivano alla soglia, li fonde in un unico ramo variabile dimenticandoli. Chi arriva dopo entra direttamente nel ramo fuso.

Non è un'euristica diversa: è la stessa senza accumulare dati che non servono al verdetto. Perché il risultato resti **indipendente dall'ordine** di arrivo (§P4), la fusione è **retroattiva**: i rami già creati per i valori precedenti — con tutti i loro sottoalberi — confluiscono nel ramo variabile, non restano indietro come pattern separati.

Il motivo per cui questo conta: accumulare tutti i valori distinti costerebbe memoria proporzionale alla cardinalità, cioè tanta **proprio sui log dove la normalizzazione serve di più**. Un tool che esaurisce la memoria sul caso d'uso per cui esiste ha un difetto di progetto, non un limite accettabile.
- Applicazione della **regola di propagazione dell'autenticazione** dalle `ObservedRequest` all'`EndpointPattern` (§6), con registrazione del numero di richieste osservabili su cui il verdetto si basa.

**Cablaggio (cancello):** l'output della Fase 1 alimenta direttamente la normalizzazione; eseguendo la CLI ora si ottiene un inventario di `EndpointPattern` (non più righe grezze). Il collegamento Fase 1 → Fase 2 è dimostrato dal fatto che l'output è cambiato da "richieste" a "endpoint logici".

**Criteri di riuscita:**
- [ ] `/api/users/1`, `/api/users/2`, `/api/users/3` collassano in un unico `/api/users/{id}`
- [ ] Endpoint realmente diversi restano separati
- [ ] Lo stesso endpoint riceve lo stesso identificatore in run diversi (§P4)
- [ ] Un pattern le cui richieste non sono osservabili sul fronte auth risulta "non osservabile", non "senza auth"; un pattern con poche richieste osservabili porta con sé quante fossero (§6)

**Verifica avversaria (il fallimento più insidioso, perché è invisibile):**
- **Falso raggruppamento:** se l'euristica è troppo aggressiva, `/api/users/admin` viene fuso in `/api/users/{id}`, e un endpoint amministrativo (magari senza auth) **sparisce** dentro un gruppo "normale" e non viene mai segnalato. Difesa (§P3): un segmento diventa variabile solo con **evidenza forte** (alta cardinalità di valori distinti **e** conformità a un tipo tipo-ID). "admin" in mezzo a 5000 ID numerici **non** deve diventare `{id}`. Prova obbligatoria: una fixture con un endpoint-admin nascosto tra tanti ID; il tool **deve** mostrarlo separato. Il costo di un falso positivo (due righe che sono la stessa cosa, l'utente lo capisce) è basso; il costo di un falso raggruppamento (un endpoint reale sparisce) è esattamente ciò che il prodotto doveva evitare.
- **Encoding nei path** (`%2e%2e`, doppio encoding): normalizzare in modo consistente così la stessa cosa non diventa due endpoint, ma senza perdere il segnale.

---

### Fase 3 — Classificazione (con e senza OpenAPI)

**Obiettivo:** produrre i `Finding` classificati. Con OpenAPI fornito → `Shadow`/`Zombie`/`Known`/`Undetermined`. Senza OpenAPI → euristiche.

**Deliverable:**
- Parsing dell'OpenAPI/Swagger fornito → `DeclaredInventory`.
- Confronto stretto osservato vs dichiarato.
- Motore euristico per il caso senza dichiarato. **I finding euristici sono `Undetermined`**, mai `Shadow`: senza un inventario dichiarato non si può *sapere* che un endpoint è shadow, lo si può solo sospettare. La sfumatura la porta il livello di **confidenza**, non la classificazione. Un sospetto e un fatto verificabile non sono la stessa cosa e non devono finire nella stessa categoria (§P9).
- Conseguenza sul contratto CI (§7): il codice di uscita `3` scatta solo su `Shadow`/`Zombie` conclamati, quindi **un run senza OpenAPI non fa mai fallire una pipeline**. È corretto così. Per chi vuole comunque il gate sui sospetti esiste `fail_on_undetermined` (`--fail-on-undetermined`), **opt-in e mai default**.
- Popolamento di evidenza, confidenza, `is_ambiguous` su ogni `Finding`.
- **Metodi osservati e non dichiarati.** Un `Finding` ha per soggetto un `EndpointPattern`, che porta un *insieme* di metodi: il modello non prevede un verdetto per-metodo, e non se ne inventa uno. Il verdetto resta `Undetermined` — la timidezza è corretta — ma l'**evidenza nomina esplicitamente** i metodi osservati e assenti dal dichiarato, e la **severità sale di un livello**: un `DELETE` non documentato è più grave di un disallineamento di path.
- **Le euristiche basate sull'assenza di autenticazione non si attivano sugli endpoint il cui flag di auth è "non osservabile"** (§6). Se il formato di log non trasporta l'informazione, un'euristica del tipo "endpoint senza auth" non sbaglierebbe ogni tanto: sbaglierebbe **sistematicamente**, su tutto il file. Meglio spenta che sempre in errore. È un'**eccezione consapevole a §P1**: qui si tace di proposito perché il segnale non esiste, non perché sia scomodo — e il fatto che l'informazione manchi va comunque detto all'utente, non nascosto.

**Cablaggio (cancello):** pipeline completa log → inventario → classificazione; eseguendo la CLI con un OpenAPI si ottengono finding etichettati per categoria; senza OpenAPI si ottengono finding euristici. Il collegamento con le fasi precedenti è dimostrato dal fatto che i finding puntano agli `EndpointPattern` della Fase 2.

**Criteri di riuscita:**
- [ ] Un endpoint osservato assente dall'OpenAPI → `Shadow`
- [ ] Un endpoint dichiarato non più osservato oltre `zombie_staleness_days` → `Zombie`
- [ ] Un match incerto → `Undetermined`, **mai** `Known`
- [ ] Senza OpenAPI, il tool resta utile (euristiche) senza sommergere di rumore
- [ ] Senza OpenAPI **nessun** finding esce come `Shadow`: i finding euristici sono `Undetermined` con confidenza esplicita
- [ ] Un run senza OpenAPI esce con `0`; con `--fail-on-undetermined` acceso esce con il codice dei finding (§7)
- [ ] Su un log il cui formato non trasporta l'autenticazione, nessuna euristica basata sull'assenza di auth produce finding — e l'utente vede dichiarato che l'informazione non era osservabile

**Verifica avversaria:**
- **OpenAPI stantio o bugiardo:** il file c'è ma è vecchio/sbagliato, e il team si fida perché "Shadow non ha detto niente". Se il match è troppo permissivo (es. solo sul prefisso `/api/users`), un endpoint pericoloso sotto quel prefisso passa come `Known` mentre la doc parlava d'altro. Difesa (§P3, §P9): matching **stretto** di default; ogni match parziale/incerto → `Undetermined`, mai silenziosamente `Known`. Un falso negativo silenzioso qui è peggio di troppi falsi positivi.
- **Rumore euristico:** se senza OpenAPI segnali 200 "possibili shadow" e 190 sono innocui, l'utente smette di leggere alla terza esecuzione — un tool che grida sempre al lupo è spento. Difesa: euristiche tarate conservative, ogni finding euristico con confidenza esplicita e motivo. Meglio pochi segnali forti che molti deboli.
- **`Undetermined` come cittadino di prima classe:** verificare che esista davvero nel flusso e non venga collassato in `Known` per comodità.

---

### Fase 4 — Output, redazione, allowlist, manifest

**Obiettivo:** trasformare i `Finding` in un output usabile e **sicuro**, riproducibile e non rumoroso alla seconda esecuzione.

**Deliverable:**
- Output terminale leggibile (tabella chiara) + export JSON e Markdown.
- Il verdetto di autenticazione di ogni endpoint mostrato **insieme al numero di richieste osservabili su cui si basa** (§6): "nessuna auth osservata (su 3 richieste osservabili di 5000)" è un'informazione onesta, "nessuna auth osservata" da solo non lo è.
- **Redactor** applicato prima di ogni stampa e scrittura.
- **Allowlist**: sopprime dalla vista i finding noti-innocui. Formato **esplicito e versionato**, leggibile e rivedibile in una pull request: chi silenzia un finding deve poterlo giustificare a un collega.
- `RunManifest` completo scritto insieme al report.
- Etichettatura onesta dei finding incerti nel report.

**Cablaggio (cancello — end-to-end reale):** eseguire la CLI su un log reale con parametri sensibili al suo interno produce un report che è (a) azionabile, (b) **con i segreti mascherati**, (c) accompagnato da un manifest, (d) con i finding già in allowlist silenziati. Questo dimostra che tutti i pezzi delle quattro fasi sono connessi e che l'output è pronto all'uso.

**Criteri di riuscita:**
- [ ] Nessun token/segreto in chiaro nell'output di default
- [ ] `shadow ... > report.json` produce un file pulito (diagnostica su stderr, §7)
- [ ] Due run sullo stesso input danno lo stesso verdetto (manifest a parte il timestamp, §P4)
- [ ] Un finding in allowlist non compare nel report ma resta nei conteggi del manifest

**Verifica avversaria:**
- **Il report che perde segreti:** i log contengono spesso token in URL, email in querystring, a volte password mandate per errore via GET. Se il report riporta i path completi coi valori grezzi, il file stesso (magari incollato in un ticket Jira condiviso) diventa una falla. Difesa (§P5): redazione **di default**, ovunque si stampi o si persista (report, allowlist, manifest). Flag esplicito tipo `--show-raw-values` per chi sa cosa fa.
- **Allowlist avvelenata o troppo larga:** una entry con wildcard troppo ampia può silenziare interi rami per errore, ricreando il falso negativo silenzioso. Difesa: formato esplicito e versionato; avviso quando una regola è troppo ampia; l'allowlist **silenzia il rumore noto ma non cancella l'evidenza** dai conteggi del manifest.
- **Noise fatigue:** senza stabilità tra run, l'utente rivede lo stesso rumore ogni volta. Difesa: identificatore stabile dell'endpoint (Fase 2) + allowlist + confidenza, così "già visto e scartato" resta scartato.
- **Report non riproducibile:** senza `RunManifest` (versione tool, versione ruleset, hash input, conteggi) non c'è audit possibile. Difesa: manifest emesso **ogni volta che un'analisi avviene** (e mai quando non avviene, §7); verdetto stabile a input costante.

---

### Fase 5 — (Post-MVP, confine del tier a pagamento) Persistenza + demone + report compliance

**Non iniziare finché le Fasi 0–4 non sono complete e collaudate.** Questa è la parte "verticale/enterprise" e coincide col confine di ciò che si paga.

Cosa aggiunge (non funzioni di analisi nuove, ma modalità d'uso nuove):
- Esecuzione **continua** come demone che ingerisce log nel tempo, invece che a comando singolo.
- Storico in un database (SQLite è sufficiente all'inizio — non serve Postgres).
- Alert quando compare un **nuovo** endpoint shadow.
- **Report di compliance** formattati per l'audit (orientati a requisiti tipo inventario endpoint per PCI DSS 4.0): elenco endpoint, classificazione, stato dell'autenticazione — con i **quattro** valori di §6, "non osservabile" compreso, mai collassati in un binario presenza/assenza — in formato esportabile. Un report d'audit che dichiara "senza auth" dove il dato non era osservabile è peggio di un report che tace.
- Multi-tenancy / multi-utente.

Questo è ciò che l'azienda paga e che il singolo non userebbe comunque. Le funzioni di analisi restano quelle delle Fasi 1–4, condivise tra free e paid.

---

## 10. Strategia di test (trasversale, cresce a ogni fase)

Non discusso prima esplicitamente, ma è il **solo** meccanismo che cattura i falsi negativi silenziosi. Senza corpus di test, non puoi verificare che il motore abbia ragione.

- **Fixture dorate:** coppie (log di input + finding attesi). Ogni fase aggiunge le sue.
- **Test di determinismo:** stesso input → output identico (manifest a parte il timestamp).
- **Fixture avversarie obbligatorie:** riga malformata in mezzo, file grande, log con segreti (per verificare la redazione), endpoint-admin nascosto tra ID (per verificare la non-aggregazione), OpenAPI stantio (per verificare `Undetermined`), log il cui formato non trasporta l'autenticazione (per verificare che il pattern risulti "non osservabile" e che nessuna euristica sull'assenza di auth si attivi — §6, Fase 3).
- **Regola:** nessun cancello di fase è verde senza le sue verifiche automatiche verdi — fixture dorate e avversarie dalla Fase 1 in poi, test sui contratti in Fase 0 (§11).

---

## 11. Meccanismo anti-deriva (come l'agente non si perde)

- **`PROGRESS.md`** con la checklist dei cancelli di ogni fase, aggiornato man mano.
- **Riorientamento a ogni sessione:** prima di lavorare, rileggi §5 (vocabolario) e `PROGRESS.md`. Sappi sempre: quale fase, cosa esisteva prima, cosa aggiunge questa fase.
- **Trigger di STOP:** se una decisione non è coperta da questo blueprint, fermati e chiedi (§3 elenca le decisioni aperte note).
- **Definizione di "fatto" per fase:** cancello di cablaggio verde **+** **verifiche automatiche verdi appropriate alla fase** **+** nessun codice morto. Tutti e tre, o la fase non è finita.
  - In **Fase 0** le verifiche appropriate sono i **test sui contratti**: non esiste output di analisi di cui fissare l'atteso, ma esiste un modello dati da esercitare. Non è un'eccezione alla regola, è la regola applicata a una fase che non produce ancora analisi.
  - **Dalla Fase 1 in poi** sono le **fixture dorate** (log di input → risultato atteso) **più le fixture avversarie obbligatorie** di §10.
  - Il principio è uno solo e non ha buchi: *nessuna fase è dimostrabilmente fatta senza una verifica automatica che lo dimostri*.

---

## 12. Note del fondatore (a parole tue)

*Spazio lasciato apposta.* Se vuoi aggiungere a Claude Code istruzioni tue in linguaggio libero — priorità, vincoli personali, cosa ti sta più a cuore che venga fatto bene per primo — scrivile qui prima di consegnargli il documento.

---

Due cose restano appese e le decidi tu, non l'agente: il **conflitto sul nome "Shadow"** e i **parametri esatti della licenza BSL**. Sono in §3 apposta, così l'agente non le inventa. Se vuoi, ti dettaglio a fondo una singola fase (tipo la Fase 2, la normalizzazione, che è quella tecnicamente più insidiosa) prima di darlo in pasto a Claude Code.
---

## 13. Registro delle revisioni

§0 stabilisce che i contratti della Fase 0 sono immutabili **salvo revisione esplicita del fondatore**. Quando una revisione avviene, va registrata qui: senza traccia, fra tre sessioni nessuno sa più se una differenza fra documento e codice è una decisione o una deriva.

### v1.1 — 2026-08-29 — revisione esplicita del fondatore, a valle della Fase 0

Nove decisioni chiuse dopo che la Fase 0 ne aveva sollevate le ambiguità. Le prime sei toccano i contratti di §6 e §7 e sono state recepite nel testo.

| # | Decisione | Dove è recepita |
|---|---|---|
| 1 | Severità su scala a tre livelli (bassa / media / alta), nessun altro livello | §6, `Finding` |
| 2 | `Auth` "non osservabile" diventa la **quarta** variante anche a livello di `EndpointPattern`, con regola di propagazione e con il numero di richieste osservabili su cui il verdetto si basa reso visibile | §6 `ObservedRequest` e `EndpointPattern`; Fase 2 (propagazione); Fase 3 (euristiche); Fase 4 (output); Fase 5 (report di compliance) |
| 3 | Codici di uscita: `0` nessun finding, `1` errore di esecuzione, `2` errore d'uso, `3` finding `Shadow`/`Zombie`. Gli `Undetermined` da soli non attivano il `3` | §7 |
| 4 | Hash degli input: SHA-256 sul contenuto del file, algoritmo registrato accanto al digest nel manifest | §6, `RunManifest` |
| 5 | I `Finding` riferiscono il `DeclaredEndpoint` tramite pattern del path + metodi; nessun identificatore stabile per il dichiarato finché non serve davvero | §6, `Finding` |
| 6 | Confermati i due allargamenti proposti in Fase 0: valori multipli per chiave di query, schemi di autenticazione dichiarati al plurale | §6, `ObservedRequest` e `DeclaredEndpoint` |
| 7 | `ObservedInventory` e `DeclaredInventory` restano termini di vocabolario: i tipi concreti nascono in Fase 2 e Fase 3 | §5 (già corretto così) |
| 8 | Lingua: identificatori del codice di produzione e testo utente in inglese, documentazione interna e nomi dei test in italiano | §5, convenzione di lingua |
| 9 | BSL: finestra di conversione a **4 anni**; la definizione di "uso aziendale" resta aperta | §3 |

**Il modello dati di §6 è da qui in avanti congelato.** Ogni ulteriore modifica richiede una nuova revisione esplicita, registrata in questa tabella.

### Modifiche fatte nel recepire la v1.1 — alcune da ratificare

Recependo le nove decisioni sono stati toccati punti che le decisioni non nominavano ma che le contraddicevano. Sono elencati qui perché il documento non cambi mai senza lasciare traccia di chi l'ha cambiato e perché. **Le voci marcate [DA RATIFICARE] sono inferenze dell'agente, non parole del fondatore:** finché non sono confermate, valgono ma restano segnalate.

| Punto | Modifica | Natura |
|---|---|---|
| §2 | "Orientamento verso BSL" → "la licenza è la BSL 1.1, finestra 4 anni" | **RATIFICATA** il 2026-08-29: era una svista in §2, la licenza è scelta |
| §5 | Convenzione di lingua: precisato che i **nomi dei test** restano in italiano | **RATIFICATA** il 2026-08-29: la convenzione vale per identificatori pubblici e testo utente, non per i nomi dei test, che sono documentazione interna |
| §5 | Aggiunti `AuthObservation` e "non osservabile" alla tabella del vocabolario | conseguenza diretta della decisione 2 |
| §9 Fase 2 | Deliverable e criterio per la regola di propagazione dell'auth | conseguenza diretta della decisione 2 |
| §9 Fase 5 | Report di compliance: stato dell'auth a quattro valori, mai binario | conseguenza diretta della decisione 2 |
| §10 | Aggiunta la fixture avversaria obbligatoria per l'auth non osservabile | conseguenza diretta della decisione 2 |
| §13 | Riga 2 estesa a Fase 2 e Fase 5; riga 8 allineata a §5 | coerenza interna del registro |

Restano inoltre **aperte** alcune contraddizioni emerse dalla verifica avversaria del recepimento: sono elencate in `PROGRESS.md`, non risolte qui, perché nessuna è coperta dalle nove decisioni (§0: non inventare, chiedere).

### v1.2 — 2026-08-29 — revisione esplicita del fondatore, a valle della verifica avversaria

Quattro contraddizioni chiuse. Su una di esse — il contratto stdout/stderr — **il blueprint aveva torto**: era formulato in modo troppo assoluto, ed è stato riscritto, non aggirato.

| # | Decisione | Dove è recepita |
|---|---|---|
| 10 | **Contratto stdout/stderr riformulato.** Il criterio non è "machine-readable contro leggibile" ma **chi ha chiesto cosa**: stdout porta ciò che l'utente ha chiesto (report, `--help`, `--version`), stderr tutto ciò che non ha chiesto. Mandare l'aiuto richiesto su stderr romperebbe `shadow --help \| less` | §7 |
| 11 | **Nessun manifest, nessun verdetto.** `0` significa *analisi eseguita e completata* con zero `Shadow`/`Zombie`; un codice con semantica di finding è emesso solo insieme a un `RunManifest`. Un'invocazione che non analizza nulla non afferma mai "nessun finding" | §7; Fase 4 (verifica avversaria) |
| 12 | **Il digest SHA-256 è deliverable della Fase 1**, non della Fase 4, calcolato nella stessa passata di lettura. Un manifest che tace quale file ha analizzato viola la propria ragione d'essere, e rimandarlo renderebbe non riproducibili i primi report | §9 Fase 1 (deliverable, cancello, criteri) |
| 13 | **I finding euristici sono `Undetermined`**, mai `Shadow`: senza dichiarato c'è un sospetto, non un fatto. Il codice `3` scatta solo su `Shadow`/`Zombie` conclamati, quindi mai su un run senza spec — e questo è corretto, non un difetto. Via d'uscita opt-in: `fail_on_undetermined` | §7; §9 Fase 3 |

Anticipate nella stessa revisione, perché discendono da principi già fissati e non da decisioni future:

| Punto | Modifica | Natura |
|---|---|---|
| §4 P5 | La redazione copre esplicitamente i **percorsi di filesystem**: rivelano cliente e struttura interna e finiscono nel manifest che si consegna all'auditor | estensione esplicita di §P5, decisa dal fondatore |
| §5, §6, §7 | Tradotte in inglese le stringhe che escono verso l'utente: esempi di evidenza in §6, valori canonici di `output_format` in §7; §5 dichiara che la regola copre anche evidenza, motivi di scarto e valori di configurazione | applicazione della decisione 8, già chiusa |

**Nota su §6:** l'unica modifica al testo di §6 è la traduzione degli *esempi* di evidenza. Nessun campo è stato aggiunto, rimosso o ridefinito: il modello dati resta congelato come stabilito in v1.1.

### v1.3 — 2026-08-29 — revisione esplicita del fondatore, prima della Fase 1

| # | Decisione | Dove è recepita |
|---|---|---|
| 14 | **Lettura di §P8 ratificata**: in Fase 0 il pilastro vieta i componenti speculativi (costruttori, accessori e `impl` che nessuna decisione richiede e che nessuno raggiunge), e dalla Fase 1 in poi vale nella forma piena, perché la pipeline raggiunge il modello dall'entrypoint. Ratificate anche le rimozioni già fatte | §P8 invariato; lettura registrata in `PROGRESS.md` |
| 15 | **Nessuna esenzione dalle fixture di Fase 0: si corregge §11.** La definizione di "fatto" chiede **verifiche automatiche verdi appropriate alla fase** — in Fase 0 i test sui contratti, dalla Fase 1 in poi le fixture dorate più quelle avversarie. Il principio non ha più un buco al primo passo, e la Fase 0 è fatta secondo §11 senza eccezioni | §11; §10 (stessa regola, allineata) |

Con questa revisione la **Fase 0 è chiusa**: cancello verde, verifiche automatiche verdi appropriate alla fase, nessun codice morto.

### v1.4 — 2026-08-30 — revisione esplicita del fondatore, a valle della Fase 1

Tre scelte che la Fase 1 aveva dovuto fare per non fermarsi, chiuse dopo averle viste funzionare.

| # | Decisione | Dove è recepita |
|---|---|---|
| 16 | **Schema di autenticazione neutro quando non è determinabile.** Su `$remote_user` valorizzato il fatto è "c'era autenticazione", l'inferenza è "era Basic": il campo si chiama `schema`, non `schema probabile`, e "quasi sempre giusto" su un dato che finisce in un report d'audit è il dato inventato-che-sembra-vero vietato da §P2. `NotObservable` sarebbe l'errore opposto, butterebbe via un fatto reale | §6; `nginx.rs`, `observed_request.rs` |
| 17 | **I sette motivi di scarto sono contratto congelato**, allo stesso titolo di §6: aggiungerne è consentito, rinominarne o rimuoverne no, perché sono chiavi dentro manifest già prodotti | §7 (catalogo) |
| 18 | **Le soglie di ingestione entrano nel `RulesetVersion`.** `MAX_LINE_BYTES` (64 KiB) e `FORMAT_SAMPLE_LINES` (100) decidono cosa viene scartato, quindi l'esito dell'analisi: senza versionamento due run sullo stesso input potrebbero divergere senza che nessuno possa ricostruire perché. Il criterio generale è ora scritto in §7: *se cambiarla cambia l'esito di un'analisi, è parte del ruleset* | §7; `shadow-core/src/ruleset.rs` |
| 19 | **Il default di `log_format` resta, ma condizionato.** Vale solo perché la validazione è stretta e il fallimento è rumoroso; se la validazione si allentasse, il default va rimosso nello stesso momento | §7 |

Conseguenza operativa della 18: il ruleset non è più vuoto, quindi la `RulesetVersion` non è più `0.0.0`.

### v1.5 — 2026-08-30 — revisione esplicita del fondatore, a valle della Fase 2

| # | Decisione | Dove è recepita |
|---|---|---|
| 20 | **La normalizzazione decide durante la lettura.** Oltre la soglia di valori distinti la decisione non cambia più: tenerne cinquantamila non aggiunge nulla al verdetto e costa memoria proprio sui log ad alta cardinalità, cioè quelli dove la normalizzazione serve di più. Non è un'euristica diversa, è la stessa senza accumulare dati inutili — e la fusione dei rami è **retroattiva**, così il risultato resta indipendente dall'ordine di arrivo | §9 Fase 2; `inventory/variability.rs` |
| 21 | **Regola scritta per la `RulesetVersion`**: cambia quando cambia qualsiasi cosa che possa alterare l'esito a parità di input; non cambia per rifattorizzazioni, documentazione o sola presentazione. Minor per soglie, major se i report vecchi non sono più confrontabili | §7; `ruleset.rs` |
| 22 | **Identificatore stabile basato sul solo pattern: confermato.** Se dipendesse anche dai metodi, un endpoint che domani riceve un `POST` uscirebbe dall'allowlist e riapparirebbe come rumore — il *noise fatigue* della Fase 4 che si sarebbe manifestato tre fasi dopo | §6; `inventory/mod.rs` |
| 23 | **Il parsing della specifica dichiarata ha un crate suo, `shadow-spec`.** Non `shadow-core` (niente I/O), non `shadow-collectors` (è definito sui formati di log, e un OpenAPI non è traffico osservato ma il suo opposto concettuale), non `shadow-cli` (è orchestrazione, e riempirla di logica è il modo in cui i progetti degenerano) | §8; crate `shadow-spec` |

Nella stessa revisione, due conseguenze registrate perché il documento resti vero:

| Punto | Modifica | Natura |
|---|---|---|
| `ruleset.rs` | `RulesetVersion` a `0.2.0`: le euristiche di Fase 3 e la ri-codifica delle graffe letterali nei path cambiano l'esito a parità di input, quindi la regola 21 impone il minor | applicazione della regola 21 |
| `inventory/path.rs` | Le graffe letterali in un path osservato vengono ri-codificate (`%7Bid%7D`): senza, un client che manda il template invece del valore finirebbe nello stesso pattern degli identificatori veri — un falso raggruppamento | conseguenza di §9 Fase 2 |

### v1.6 — 2026-08-30 — revisione esplicita del fondatore, a valle della Fase 3

| # | Decisione | Dove è recepita |
|---|---|---|
| 24 | **§6 riaperto per un solo campo: la configurazione effettiva del run entra nel `RunManifest`**, con la provenienza di ogni valore. È lo stesso buco d'audit che la decisione 4 aveva chiuso per gli input: un report che dice `Undetermined` senza dire con quale `zombie_staleness_days` è stato prodotto non è ricostruibile, e chi confronta due report divergenti non può sapere se è cambiato il traffico o la configurazione. **§6 è richiuso subito dopo questa modifica** | §6; `run_manifest.rs` |
| 25 | **YAML supportato**, con `serde_norway` — fork mantenuto di `serde_yaml`, API già nota, minor superficie di rischio. Il rifiuto conservativo era giusto come stato temporaneo, non finale: la maggioranza delle specifiche reali è in YAML e un tool che le rifiuta viene chiuso al primo tentativo. **Vincolo:** limiti espliciti su dimensione e profondità, e alias YAML rifiutati, perché le bombe di espansione sono un vettore reale | §7; `shadow-spec` |
| 26 | **Granularità per-metodo: nessun secondo cantiere su §6.** L'informazione c'è già nell'evidenza: il verdetto resta `Undetermined`, l'evidenza nomina i metodi non dichiarati, la severità sale di un livello. Da riaprire in Fase 4 solo se i report reali lo mostrano insufficiente | §6, §9 Fase 3; `classify/` |
| 27 | **`zombie_staleness_days` a 30 giorni: confermato.** Copre un ciclo di rilascio mensile senza rendere inutile la categoria, e ora chi lo cambia lascia traccia nel manifest | §7; `ruleset.rs` |
| 28 | **Composizione della severità: confermata**, più la regola della decisione 26 | §6; `classify/` |

Il catalogo dei quattro path sospetti resta com'è: si rivede in Fase 4, su report prodotti da log veri.

### v1.7 — 2026-08-30 — revisione esplicita del fondatore, prima del collaudo

| # | Decisione | Dove è recepita |
|---|---|---|
| 29 | **La redazione vale anche su stderr, con un'eccezione mirata**: i percorsi che l'utente ha scritto sulla riga di comando in questa stessa invocazione compaiono in chiaro, perché non gli rivelano nulla che non abbia appena digitato. Tutto il resto — percorsi derivati, percorsi da config o ambiente, e qualunque cosa provenga dai dati analizzati — resta mascherato, e report e manifest non hanno eccezioni | §P5; `main.rs` |
| 30 | **Lo schema del report JSON è chiuso: `shadow-report/1`**, contratto pubblico versionato allo stesso titolo dei motivi di scarto. Aggiungere campi è consentito, rinominarli o rimuoverli richiede `shadow-report/2`. La voce corrispondente esce da §3 | §7; §3 |
| 31 | **Un avviso non è un verdetto**: le soglie che governano avvisi non fanno parte del ruleset e non fanno salire la `RulesetVersion`. Scritto come precedente, perché non si ridiscuta a ogni fase | §7 |
| 32 | **I finding silenziati dall'allowlist non contano per il codice di uscita.** È il senso stesso dell'allowlist: un finding già valutato e scartato che continuasse a far fallire la pipeline la renderebbe inutile. I conteggi del manifest restano completi, quindi la tracciabilità c'è | §7, Fase 4; `main.rs` |

Nessuna di queste fa salire la `RulesetVersion`, e il perché è la decisione 31 applicata a se stessa: cambiano ciò che si legge, non ciò che il tool conclude. A parità di input i verdetti sono identici e l'archivio dei report resta confrontabile.

**Formato dell'allowlist e perimetro del `Redactor` restano deliberatamente aperti:** sono le due cose che il collaudo su log veri risolve da solo. Il rischio "report illeggibile" si vede guardando un report vero, non ragionandoci sopra.

### v1.8 — 2026-09-05 — revisione esplicita del fondatore, a valle delle correzioni post-collaudo

Una sola decisione, ratificata dopo che il codice l'aveva già implementata e il `PROGRESS.md` l'aveva segnalata come inferenza dell'agente in attesa di conferma.

| # | Decisione | Dove è recepita |
|---|---|---|
| 33 | **Valori canonici delle chiavi booleane: `true`/`1` e `false`/`0`, senza varianti di maiuscole e senza sinonimi.** Un valore diverso ferma il run invece di valere `false`; lo stesso vale per una variabile d'ambiente che esista ma non sia testo valido. Prima, qualunque valore diverso da `1`/`true` significava `false` in silenzio, e `SHADOW_FAIL_ON_UNDETERMINED=yes` spegneva il gate mentre chi l'aveva scritto credeva di averlo acceso | §7; `config.rs`, `cancello.rs` |

Non fa salire la `RulesetVersion`: sta nella lettura della configurazione, che l'elenco di §7 non nomina fra le cose che stanno nel ruleset, e a configurazione valida non cambia nessun verdetto.

**Resta invece in attesa di ratifica l'emendamento a §7 su `log_format`** — la grammatica dichiarabile, implementata e descritta in `PROGRESS.md`. Questa revisione **non** la copre: porta una decisione sola, e il documento non deve poter far credere il contrario.

### v1.9 — 2026-09-05 — revisione esplicita del fondatore, a valle della taratura post-collaudo

La decisione che la v1.8 aveva lasciato fuori di proposito.

| # | Decisione | Dove è recepita |
|---|---|---|
| 34 | **La grammatica di log è dichiarabile.** `log_format` accetta o un nome canonico o la direttiva `log_format` del proprio server, distinta perché contiene `$`. La grammatica dichiarata è validata con la stessa strettezza di quella preimpostata, e una dichiarazione ambigua o incompleta ferma il run prima di aprire il file. Il valore preimpostato resta, perché il vincolo condizionale che lo giustificava non è cambiato | §7; `log_format.rs`, `nginx.rs`, `lib.rs`, `main.rs` |

**Perché la decisione è stata presa con dei numeri davanti, non per comodità.** Il collaudo su corpus reale (2026-08-30) aveva misurato che **una variante di `log_format` su sette** veniva accettata — il `combined` nudo — e che le altre sei sono configurazioni ordinarie: `$request_time`, `$upstream_response_time`, `$http_x_forwarded_for`, il default di ingress-nginx, il `common` di Apache. Rifiutarle era corretto secondo §P2 e allo stesso tempo rendeva il tool inservibile dove serve. La via d'uscita non è allentare la grammatica: è dichiararla. Dopo la taratura le varianti leggibili sono **sette su sette dichiarandole**, e **una sola senza dichiarazione** — cioè il default non si è allargato di un millimetro, e un test lo presidia.

Questa revisione **non** fa salire la `RulesetVersion` di suo: la taratura che la implementa l'aveva già portata a `0.4.0` quando è stata scritta, ed è a `0.5.0` dalle correzioni sulla redazione. Ratificare un comportamento già implementato non cambia nessun verdetto.

**Restano aperti**, e non li tocca nessuna delle due revisioni di oggi: il perimetro del `Redactor` (con due limiti noti, ciascuno con un test che asserisce un segreto in chiaro), il formato dell'allowlist, e le decisioni di §3.

### v1.10 — 2026-09-05 — revisione esplicita del fondatore, all'apertura della Fase 5

Le quattro decisioni che la Fase 5 non poteva cominciare senza. §9 le elencava come «da risolvere col fondatore, NON inventare», ed è così che sono state risolte.

| # | Decisione | Dove è recepita |
|---|---|---|
| 35 | **Il crate si chiama `shadow-compliance`, lo schema resta `shadow-report/1`.** La collisione fra §8 e §7 si scioglie spostando il nome del crate, che è interno e non ha consumatori, invece dell'identificatore dello schema, che è un contratto pubblico congelato | §8 |
| 36 | **L'alert è senza egress.** Le forme ammesse sono: una riga marcata «nuovo / non preso in carico» nello storico, il sottocomando `shadow alerts` che la interroga, il codice di uscita del run che l'ha scoperto. Niente webhook, niente SMTP, niente esecuzione di comandi. La frase «questo binario non apre socket» resta verificabile dall'esterno, e per uno strumento di sicurezza quella frase è il prodotto (§P6) | §9 Fase 5; `history.rs`, `main.rs` |
| 37 | **«Multi-tenancy» significa separazione dei bersagli d'analisi.** Un demone sorveglia più servizi, ciascuno con log, specifica, allowlist e storico propri, e i due inventari non si mescolano mai. **Non** significa account, ruoli o login: quelli implicherebbero un server e contraddirebbero §2 | §7 (`target`); `shadow-history` |
| 38 | **Lo storico conserva i run e lo stato per endpoint**: manifest e finding di ogni esecuzione, più prima/ultima volta visto e «preso in carico» per ciascun endpoint. Serve entrambe le cose che §9 chiede alla Fase 5, l'alert sul nuovo shadow e il report d'audit | `shadow-history/src/schema.rs` |

**Tre conseguenze scritte qui perché nessuna è stata decisa a voce.**

**§P4 con il tempo di mezzo.** Un demone introduce orologio, ordine di arrivo e stato accumulato, e ognuna delle tre cose è un modo di far dipendere un verdetto da qualcosa che non è l'input. La separazione è strutturale: `shadow-core` classifica senza sapere che lo storico esista, e la novità — *questo endpoint non c'era* — è un'**annotazione accanto al finding**, mai un ingrediente. A input costante il report è identico con e senza storico, e un test lo verifica confrontandoli carattere per carattere.

**L'ordine non lo decide l'orologio.** I run sono ordinati dal proprio identificatore progressivo. Un orologio che va all'indietro viene **detto**, non aggiustato: il timestamp resta un dato, e quando contraddice l'ordine di registrazione lo si segnala.

**Nessun manifest, nessun verdetto, anche qui.** `shadow alerts` legge una memoria e non analizza niente: non emette un `RunManifest` e non afferma «nessun finding». Esce con successo nello stesso senso in cui lo fanno `--help` e `--version` (§7). E se lo storico non si lascia scrivere, il run si ferma **prima** di stampare: un report stampato con la memoria non aggiornata farebbe risultare tutto nuovo all'esecuzione dopo.

**Restano aperte** e non le tocca questa revisione: la **politica di retention** (lo schema garantisce già che una cancellazione non possa portarsi via evidenza ancora citata, ma *quando* e *se* cancellare non è deciso), il **formato del report di compliance**, e se il **confine free/paid** sia imposto tecnicamente o solo dalla licenza.

### v1.11 — 2026-09-05 — revisione esplicita del fondatore, a valle della prima fetta della Fase 5

I due emendamenti che la v1.10 aveva lasciato segnalati come inferenze dell'agente.

| # | Decisione | Dove è recepita |
|---|---|---|
| 39 | **Il crate `shadow-history` entra in §8.** Lo storico non è né la CLI né il demone: **entrambi** lo scrivono, e metterlo nel demone farebbe dipendere la CLI dal demone — l'arco inverso che §8 vieta. Il suo schema evolve per migrazioni versionate, e una versione sconosciuta ferma l'apertura | §8 |
| 40 | **`history_path` e `target` entrano fra le chiavi di configurazione di §7**, con la precedenza e la registrazione nel manifest di tutte le altre. `history_path` è un percorso e non un URI: un valore che comincia per `file:` è rifiutato | §7 |

Nessuna delle due fa salire la `RulesetVersion`, che resta `0.5.0`: una chiave di configurazione in più non è una regola che decide l'esito di un'analisi, e a configurazione invariata non cambia nessun verdetto. Il codice le implementava già: ratificare un comportamento esistente non cambia niente di ciò che il tool conclude.

**Con questa, non restano inferenze dell'agente in attesa di conferma.** Codice e blueprint tornano allineati: non c'è nessun comportamento implementato che il documento non conosca.

**Restano aperte**, e non le tocca questa revisione: la **politica di retention**, il **formato del report di compliance**, il **confine free/paid**, le voci di §3 (nome «Shadow», parametri BSL, secondo Collector) e il **perimetro del `Redactor`**, con i suoi due limiti noti presidiati da altrettanti test.

### v1.12 — 2026-09-06 — revisione esplicita del fondatore, a valle del passo umano del collaudo

| # | Decisione | Dove è recepita |
|---|---|---|
| 41 | **`path_prefix` entra fra le chiavi di configurazione di §7**: quale parte del traffico è l'API. **Opt-in**, e il valore preimpostato resta «analizza tutto» — dedurre il prefisso dalla specifica sarebbe stato facile, `shadow-spec` conosce già il `basePath`, e sarebbe stato sbagliato: un'API non documentata su un altro path sparirebbe senza che nessuno lo sappia. Le richieste fuori portata non spariscono, vengono contate, e il conteggio si dice | §7; `config.rs`, `main.rs`, `report.rs` |

**Perché la decisione è stata presa con dei numeri davanti.** Il passo umano del collaudo (2026-09-06) ha misurato che su 536 finding non-`Known` **uno solo** valeva la pena di guardarlo, e ha identificato quattro famiglie che producevano tutto il resto. Le pagine della UI web dentro il log erano una delle quattro: la specifica descrive solo l'API, quindi ogni pagina risultava un endpoint non documentato, con confidenza alta e uscita `3`.

**Cosa è cambiato insieme a questa, senza essere una revisione.** Vanno annotate qui perché il registro serva a distinguere una decisione da una deriva, anche quando la modifica non tocca il documento:

- **Il buco di `{filepath}`** — un parametro dichiarato che attraversa le barre non poteva combaciare con niente, quindi usciva `Shadow` conclamato. Ora è `Undetermined`. Non è una regola nuova: è la definizione di `Shadow` di §5 applicata alla lettera, *«assente dall'inventario dichiarato»*, e un'assenza non è verificabile finché un dichiarato potrebbe coprirla. `RulesetVersion` 0.6.0.
- **Il raggruppamento dei finding nel report** — la ragione si scrive una volta e i soggetti si elencano tutti. È presentazione, che §7 dichiara non essere un contratto, e infatti il JSON è rimasto identico: **non** fa salire la `RulesetVersion`.

`RulesetVersion` è quindi passata a **0.7.0**, per il buco di `{filepath}` e per `path_prefix`.

**Restano aperte**: la politica di **retention**, il **formato del report di compliance**, il **confine free/paid**, le voci di §3, il **perimetro del `Redactor`** con i suoi due limiti noti, e le due domande che il passo umano ha lasciato senza risposta — il **catalogo dei path sospetti**, che su traffico vero non è mai scattato, e la **granularità per-metodo**, mai esercitata.
