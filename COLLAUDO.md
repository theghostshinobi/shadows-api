# Collaudo delle Fasi 0–4

§9 dice di non iniziare la Fase 5 finché le Fasi 0–4 non sono complete **e
collaudate**. Sono complete e verdi su 106 verifiche automatiche. Questo
documento è il piano per stabilire se sono anche collaudate.

## Perché le fixture non bastano, e non è un difetto delle fixture

Le fixture le ho scritte io sapendo cosa dovevano trovare. Ognuna presidia un
fallimento che **avevamo già immaginato**: l'endpoint amministrativo fra gli
identificatori, l'OpenAPI stantio, la bomba di espansione. Fanno bene il loro
lavoro — impediscono che un fallimento noto torni — ma per costruzione non
possono dire nulla su ciò che non abbiamo immaginato.

Un log vero non è d'accordo con nessuno. Contiene forme di URL che nessuno ha
previsto, varianti di formato che nessuno ha dichiarato, e volumi che rendono
visibile ciò che su cinque righe non si vede.

C'è un precedente in questo progetto che vale la pena ricordare: la fixture
della bomba YAML l'ho scritta convinto che la mia difesa la fermasse, e a
fermarla è stata la protezione interna della libreria. La mia scansione degli
alias cercava `*` solo dopo uno spazio, mentre le bombe si scrivono `[*a,*a]`.
Senza quella fixture avrei consegnato una difesa che non difendeva. **Il
collaudo è la stessa cosa, un livello più su.**

---

## Le cinque domande

Ognuna ha una proprietà osservabile e un modo di misurarla. Dove la misura non è
automatizzabile lo dico, invece di inventare un numero che sembra oggettivo.

### 1. Rumore — la domanda principale

Se su traffico reale Shadow produce cento `Undetermined` di cui novanta
innocui, l'utente smette di leggere alla terza esecuzione, e da quel momento il
tool non segnala più niente. Costruirci sopra la Fase 5 significherebbe
vendere un allarme che nessuno ascolta.

**Cosa si misura, dal report JSON:**

| Misura | Perché |
|---|---|
| finding per categoria, in assoluto | il volume grezzo |
| **finding non-`Known` ogni 10 endpoint** | il rumore non è il numero, è il numero **per unità di attenzione**: 12 finding su 400 endpoint vanno bene, 35 su 40 no |
| distribuzione per **motivo** (prima riga di evidenza) | dice *quale* euristica genera il volume, non solo che ce n'è |
| finding identici fra giorni consecutivi | il rumore ricorrente lo gestisce l'allowlist; quello che cambia a ogni run è molto peggio |

**Cosa non si misura da solo:** se un finding sia *utile*. Serve il passo umano
descritto sotto — ed è la ragione per cui questo collaudo non lo posso fare da
solo.

> **La soglia del 30% è un innesco, non un voto.** Se più del 30% degli
> `Undetermined` viene giudicato rumore, la conversazione che si apre è *"come
> lo tariamo"*, non *"il tool è rotto"*. Superarla non è una bocciatura: è il
> segnale che le euristiche vanno tarate **prima** di costruirci sopra la Fase 5.
> Vale lo stesso per le altre soglie di questo documento: nessuna è un criterio
> di successo o fallimento, tutte sono inneschi di una discussione.

### 2. Leggibilità sotto redazione

Il rischio è simmetrico a quello che la redazione risolve: un report che non
perde segreti ma che nessuno riesce a leggere è inutile quanto uno che li perde.

| Misura | Soglia proposta |
|---|---|
| quota di pattern di endpoint che contengono `<redacted>` | oltre il 15% il `Redactor` è troppo aggressivo |
| quota di righe di evidenza che contengono `<redacted>` | idem |
| lettura umana di 20 finding: *saprei cosa fare?* | non automatizzabile |

Il perimetro del `Redactor` è deliberatamente rimasto aperto: è una cosa che si
vede guardando un report vero, non ragionandoci sopra.

### 3. Qualità della normalizzazione — ciò che le fixture non possono dire

I due errori non sono simmetrici e non si misurano allo stesso modo.

**Sotto-aggregazione** (una forma di identificatore non riconosciuta) è
**visibile e misurabile**. Sintomo: un prefisso con molti figli letterali che si
somigliano. Slug (`/posts/il-mio-titolo`), date (`/reports/2024-01-15`), ULID,
esadecimali corti, codici di lingua: il catalogo delle forme tipo-ID riconosce
solo numerico, UUID ed esadecimale di quattro lunghezze, e su traffico reale è
probabile che manchi qualcosa.

> Misura: raggruppare i pattern per prefisso padre e segnalare i prefissi con
> più di N figli letterali. Ogni prefisso segnalato è un candidato "forma di ID
> non riconosciuta", da guardare a occhio.

**Sovra-aggregazione** (una parola inghiottita dentro `{id}`) è **invisibile per
costruzione**: se è successo, dal report non si vede. Due cose si possono fare,
e nessuna è una misura:

- l'argomento di disegno — un valore diventa `{id}` solo se ha forma di
  identificatore, e `admin` non ce l'ha, qualunque sia la cardinalità. Regge
  finché il catalogo delle forme resta stretto; se lo allarghiamo per risolvere
  la sotto-aggregazione, **questo argomento si indebolisce**, ed è il vero
  rischio del punto 3;
- una controprova sul log grezzo: estrarre i segmenti non-tipo-ID che compaiono
  sotto un prefisso collassato e verificare che **ognuno** compaia ancora nel
  report. È automatizzabile, e la include lo script.

### 4. Robustezza del parsing sulle varianti reali

**Ipotesi da falsificare, e me l'aspetto vera:** molte installazioni nginx
aggiungono campi al formato `combined` — `$request_time`,
`$upstream_response_time`, `$http_x_forwarded_for`. La grammatica attuale è
stretta: rifiuta la riga se dopo lo user agent c'è altro. Su un log così Shadow
rifiuterà l'intero file con "formato non riconosciuto".

È il comportamento corretto secondo §P2 — meglio rifiutare che leggere campi
dalla posizione sbagliata — ma se capita sulla maggioranza delle installazioni
reali, allora serve un modo per **dichiarare** il proprio `log_format`, ed è una
cosa da sapere prima della Fase 5, non dopo.

| Misura | Soglia proposta |
|---|---|
| righe scartate su totale, per corpus | oltre il 2% su un `combined` genuino, la grammatica è troppo stretta |
| distribuzione dei motivi di scarto | dice *cosa* non combacia |
| numero di corpus rifiutati in blocco | dice quanto è diffusa la variante |

### 5. Scala e memoria su volume reale

Sul sintetico la memoria è piatta: 6 MB con un milione di path distinti. Da
confermare su un log vero, dove i path distinti sono *diversi* fra loro e non
generati da un ciclo.

| Misura | Soglia proposta |
|---|---|
| RSS di picco su un log da almeno 1 GB | non deve crescere con la dimensione del file |
| tempo per gigabyte | indicativo, non un criterio |

---

## I corpus: dove procurarsi log veri

In ordine di **quanto rispondono alla domanda principale**, non di comodità.

### A. Log di produzione tuoi, o di un team che te li presta

Il solo corpus che risponde davvero alla domanda sul rumore: traffico vero, di
un dominio vero, con le forme di URL che quel dominio usa davvero. Se esiste
anche l'OpenAPI corrispondente, esercita **tutta** la classificazione — con la
specifica, non solo le euristiche.

Sulla privacy, due cose:

- eseguirlo **sulla macchina che quei log già li ha**, senza copiarli altrove:
  Shadow è offline e non manda niente da nessuna parte (§P6), quindi non c'è
  motivo di spostare i dati;
- il report è già redatto di default, e questo rende il collaudo **anche un
  test di §P5**: se dopo la redazione ci trovi ancora qualcosa che non
  condivideresti, è un difetto del `Redactor` e va corretto.

### B. Un'applicazione open source reale dietro nginx

Il secondo per valore, il primo per riproducibilità. Si prende un progetto che
pubblica un OpenAPI, lo si mette dietro nginx, lo si guida con la sua stessa
suite di test o con un generatore di carico, e si cattura il log.

Dà **forme di URL vere** e una **specifica vera** senza toccare dati di terzi,
ed è ripetibile: se fra sei mesi cambiamo un'euristica, si rifà identico e si
confrontano i numeri. È il candidato naturale per diventare una regressione
permanente.

### C. Dataset pubblici storici

NASA-HTTP (1995), Calgary e Saskatchewan HTTP, EPA-HTTP; la raccolta *Loghub*
raccoglie log di sistemi reali per la ricerca. Sono pubblici, in formato
Common/Combined, e grandi abbastanza da dire qualcosa sulla scala.

**Cosa dicono:** robustezza del parser, comportamento su path molto vari,
memoria su volume vero.
**Cosa non dicono:** sono siti web degli anni '90, non API. Niente identificatori
moderni, niente OpenAPI, niente autenticazione osservabile. Usarli per giudicare
il rumore delle euristiche darebbe un numero rassicurante e **falso**.

### D. Traffico generato con profili realistici

Posso scrivere un generatore. Ma il limite va detto: **un log che genero io
verifica ciò che ho immaginato**, che è esattamente il difetto delle fixture, in
scala maggiore. Serve per carico e memoria, non per la sorpresa — e la sorpresa è
il motivo per cui stiamo facendo il collaudo.

---

## Il metodo: come non ingannarsi

Osservare e cambiare nello stesso passaggio è il modo di convincersi che
qualcosa funziona. L'ordine è vincolante.

1. **Congelare la versione.** Il manifest registra già versione del tool,
   ruleset, hash degli input e configurazione con la provenienza: ogni risultato
   è ricostruibile senza appunti.
2. **Fissare le soglie di accettazione *prima*** di guardare i risultati. Sono
   proposte sopra; vanno confermate o corrette adesso, non dopo. Deciderle dopo
   significa scegliere il numero che dà il risultato che si sperava.
3. **Raccogliere.** Eseguire su ogni corpus, salvare i report JSON. Nessuna
   modifica a euristiche, soglie o catalogo in questa fase.
4. **Passo umano, in cieco.** Un campione casuale di 30 finding, etichettati
   uno per uno *"vale la pena guardarlo"* / *"rumore"*, **prima** di vedere le
   statistiche aggregate. È l'unica misura possibile dell'utilità, e vederle
   prima la condizionerebbe. Questo passo lo puoi fare solo tu: io non so quali
   endpoint del tuo dominio meritino attenzione.
5. **Solo dopo, decidere insieme.** Con i numeri e le etichette davanti.

---

## Le due domande rimandate apposta

Si riaprono qui, con i report reali davanti.

**Catalogo dei path sospetti** (`test`, `debug`, `old`, `internal`). Le
domande: quante volte scatta su traffico vero? Quando scatta, è utile? E ci sono
segmenti frequenti nel tuo dominio che dovrebbero starci e non ci sono
(`staging`, `preview`, `v0`, `tmp`)? Il rischio è in due direzioni, e solo un
report reale dice in quale delle due siamo.

**Granularità per-metodo.** Oggi un metodo osservato e non dichiarato dà
`Undetermined` con la severità alzata e i metodi nominati nell'evidenza. La
domanda: quando succede davvero, quel finding si nota abbastanza? Se in un
report vero un `DELETE` non documentato si perde in mezzo agli altri
`Undetermined`, allora l'evidenza non basta e la questione torna su §6.

---

## Lo strumento di misura

`collaudo/misura.py` legge i report JSON prodotti da Shadow e calcola tutto ciò
che è calcolabile in questo documento. **Non tocca il tool**: usa solo il suo
output pubblico, `shadow-report/1`.

```bash
shadow /var/log/nginx/access.log --openapi-spec spec.json --output-format json > report.json
python3 collaudo/misura.py report.json --log /var/log/nginx/access.log
```

Con `--log` esegue anche la controprova sulla sovra-aggregazione: estrae dal log
grezzo i segmenti non-tipo-ID e verifica che ognuno compaia ancora nel report.

---

## Come è stato costruito il corpus (esecuzione del 2026-08-30)

Due applicazioni, scelte secondo il criterio: API-first, OpenAPI che **non può**
divergere dal codice perché è servito dalla stessa build che serve il traffico,
molto usate, e installabili in locale senza infrastruttura.

| | **Gitea 1.27.3** | **Vikunja 2.5.0** |
|---|---|---|
| specifica | Swagger 2.0 servita dall'istanza, 308 path, `basePath: /api/v1` | Swagger 2.0 servita dall'istanza, 126 path, `basePath: /api/v1` |
| forma dei parametri di path | **nomi**: `{owner}`, `{repo}` | **numeri**: `{id}`, `{taskID}`, `{projectID}` |
| perché questa | forge, la classe di API più diffusa con path basati su nomi | il contrasto: stessa architettura, parametri numerici |

Le due sono state prese apposta con **forme di parametro opposte**: una sola non
avrebbe permesso di distinguere una caratteristica del prodotto da
un'idiosincrasia dell'applicazione.

**Il log è vero.** Davanti a ciascuna applicazione c'è un reverse proxy Apache
2.4 che scrive in formato `combined`. Il `combined` di Apache e quello di nginx
sono la stessa cosa campo per campo — la direttiva di nginx è modellata su
questa — quindi il log è prodotto da un server reale, non generato da uno script.

**Le forme degli URL sono vere.** Le decide l'applicazione: nomi utente, nomi di
repository, indici di issue, SHA di commit, percorsi di file, identificativi di
progetto e di attività sono quelli che Gitea e Vikunja usano davvero.

### Cosa questo corpus **non** dice

Va detto, o un corpus debole passerebbe per buono:

- **il volume è piccolo** (poco più di mille richieste in tutto). Basta a mostrare
  le forme, non a dire nulla di statistico sul traffico di un servizio vero;
- **il mix di richieste lo decide il mio script**, non utenti reali. Ciò che è
  reale sono le forme degli URL e le specifiche, non le proporzioni fra endpoint;
- **la finestra temporale è di minuti**, quindi la categoria `Zombie` non è mai
  esercitata: serve un log che copra più della finestra di staleness;
- **la crescita dei path distinti non è testata.** Ripetendo il corpus reale fino
  a 420.000 righe la memoria resta piatta a 6 MB, il che conferma lo streaming ma
  non dice nulla su un log con centinaia di migliaia di path *diversi*.

## I risultati che non influenzano il giudizio umano

Le statistiche sui finding restano in `collaudo/misure/` e **non vanno lette
prima** di aver etichettato `collaudo/DA-GIUDICARE.md`: vederle prima
significherebbe confermarle invece che misurarle. Quello che segue non tocca
quel giudizio.

### La previsione su `$request_time`: confermata

Sette varianti di `log_format`, ricostruite dai formati documentati — non
raccolte da server di terzi — in `collaudo/corpus/varianti-log-format/`:

| Variante | Esito |
|---|---|
| `combined` di nginx, così com'è | **accettata** |
| `main` dell'esempio di nginx (aggiunge `$http_x_forwarded_for`) | rifiutata, `malformed-line` |
| `combined` + `$request_time` | rifiutata, `malformed-line` |
| `combined` + `$request_time` + `$upstream_response_time` | rifiutata, `malformed-line` |
| default di ingress-nginx (aggiunge nove campi) | rifiutata, `malformed-line` |
| `common` di Apache (senza referer e user agent) | rifiutata, `malformed-line` |
| `$request_time` in testa alla riga | rifiutata, `malformed-line` |

**Una sola variante su sette passa**, ed è quella senza aggiunte. Il
comportamento è corretto secondo §P2 — meglio rifiutare che leggere i campi
dalla posizione sbagliata — e allo stesso tempo rende il tool inservibile sulla
maggioranza delle installazioni reali, dove almeno `$request_time` c'è quasi
sempre. **La soluzione non è stata anticipata**, come da istruzione: è una
decisione di prodotto da prendere con questi numeri davanti.

### Parsing sui corpus reali

Zero righe scartate su entrambi, perché il proxy scrive `combined` puro. La
soglia del 2% non è stata avvicinata — ma il merito è del proxy configurato da
me, non della grammatica: la tabella qui sopra dice cosa sarebbe successo con
una configurazione di produzione.

### Leggibilità sotto redazione

Nessun pattern e nessuna riga di evidenza contiene `<redacted>` su entrambi i
corpus: il `Redactor` non ha mascherato nulla, e quindi non ha reso illeggibile
nulla. È un risultato **debole**, non un successo: questo traffico non contiene
segreti negli URL. La domanda vera resta aperta finché non passa un log che ne
contiene.

### Un difetto trovato leggendo i finding

Quando un endpoint osservato combacia solo parzialmente con più path dichiarati,
l'evidenza ne nomina **uno arbitrario** invece del più vicino. Esempio reale:
per `/api/v1/repos/collaudo/billing-service/issues/{id}` l'evidenza indica
`/api/v1/repos/{owner}/{repo}/issues/comments`, mentre il vicino ovvio è
`/issues/{index}`; sono entrambi match parziali e viene scelto il primo in
ordine. Il **verdetto è corretto**, la **spiegazione è fuorviante** — e in un
report la spiegazione è ciò su cui l'utente decide. Non corretto in questa
sessione: durante il collaudo si raccoglie, non si tara.

## Cosa mi serve da te

1. **Un corpus di tipo A o B.** È la cosa che sblocca tutto il resto. Se hai log
   di produzione tuoi, anche di un solo servizio e di un solo giorno, sono più
   utili di qualsiasi dataset pubblico. Se preferisci non usarli, dimmi quale
   applicazione open source usare per il tipo B e la preparo io.
2. **L'OpenAPI corrispondente**, se esiste. Senza, il collaudo misura solo le
   euristiche e lascia fuori metà del prodotto.
3. **Conferma delle soglie di accettazione**, o le tue. Vanno fissate prima.
4. **Il passo umano**: trenta finding da etichettare. È mezz'ora, ed è l'unico
   numero di questo collaudo che io non posso produrre.
5. **Una decisione**: se un log reale viene rifiutato per una variante di
   `log_format`, il collaudo si ferma lì o proseguo sui corpus che passano? La
   mia proposta è proseguire e riportare il rifiuto come risultato, perché è
   informazione utile quanto un numero.

---

# Esito del passo umano (2026-09-06)

**Etichettato dall'agente su autorizzazione esplicita del fondatore**, non dal
fondatore. La riserva sta scritta per esteso in testa a `DA-GIUDICARE.md` e va
ripetuta qui: questo documento aveva riservato il passo al fondatore perché *«io
non so quali endpoint del tuo dominio meritino attenzione»*, e su Gitea e
Vikunja quell'obiezione è più debole — non sono il dominio di nessuno — ma non
sparisce, perché **è il tool che giudica il proprio output**. In più l'agente
aveva già letto singoli finding dei due report, correggendo un difetto, quindi
non era cieco ai dati come lo sarebbe stato il fondatore. Non ha aperto
`collaudo/misure/` prima di etichettare, che è la condizione vincolante.

Il numero è **un innesco, non un voto**.

## Le trenta etichette

| | quanti |
|---|---|
| **utile** | **1** |
| **rumore** | 23 |
| **non so** (tutti `Known`: la domanda non si applica a un endpoint che combacia) | 6 |

Sui soli `Undetermined` del campione — che è la misura che questo documento
mette a soglia — **16 su 17 sono rumore: il 94%**. La soglia era il 30%. È
superata di tre volte, e quindi la conversazione che si apre è *come lo
tariamo*, esattamente come previsto.

## Le quattro famiglie che producono il volume

Le trenta etichette non sono trenta giudizi indipendenti: cadono in **quattro
famiglie**, e le statistiche aggregate — aperte solo dopo — dicono quanto pesa
ciascuna sul totale vero.

| Famiglia | Nel campione | Nel volume reale | Giudizio |
|---|---|---|---|
| «dichiarato ma mai osservato, e la finestra è troppo corta per dire zombie» | 10/30 | **305 su 423** (Gitea) e **115 su 127** (Vikunja) | rumore |
| Match parziale su un parametro di path **letterale** (nomi utente, nomi di repository) | 6/30 | 54 su 423 (Gitea), 1 su 127 (Vikunja) | rumore, tranne uno |
| Pagine della **UI web** segnalate come endpoint API non documentati | 4/30 | parte dei 61 `Shadow` di Gitea | rumore |
| Parametro dichiarato che **attraversa le barre** (`{filepath}`) e non può mai combaciare | 3/30 | parte dei 61 `Shadow` di Gitea | rumore |

Ogni finding non-`Known` dei due report cade in una di queste quattro. Estesa
per famiglia, l'etichettatura dice: **su 536 finding non-`Known`, uno solo vale
la pena di guardarlo.**

## L'unico finding utile, e perché è quello

`/api/v1/tasks/all` di Vikunja: `all` è una **parola letterale** dove la
specifica ha un identificatore. È il caso per cui §P3 esiste — la stessa forma
di `/api/users/admin` — ed è l'unico su cui aprirei davvero il codice.

Non l'ho scelto sapendolo: le misure, lette dopo, dicono che è **l'unico**
finding di tutto il report di Vikunja che non appartenga alla famiglia della
finestra troppo corta. Il campione stratificato e il volume reale indicano lo
stesso finding.

## Le due domande rimandate apposta: hanno una risposta

**Catalogo dei path sospetti** (`test`, `debug`, `old`, `internal`). Il run di
Gitea **senza specifica** produce `finding non-Known ogni 10 endpoint: 0.0` —
cioè **zero finding su 118 endpoint reali**. Il catalogo non è mai scattato.
Non produce rumore, e non produce nemmeno segnale: senza inventario dichiarato,
su questo traffico, Shadow non dice niente. La domanda «serve?» ha ora un dato,
e la risposta non è «è tarato bene».

**Granularità per-metodo.** Nessun finding dei due corpus nomina un metodo
osservato e non dichiarato: l'euristica non è mai scattata, quindi la domanda
«un `DELETE` non documentato si nota abbastanza?» resta **non esercitata**.
Serve traffico che contenga il caso.

## Le due cose che invece hanno retto

**Nessuna parola inghiottita dentro `{id}`.** La controprova sul log grezzo:
0 segmenti non-tipo-ID su Gitea e 12 su Vikunja, **tutti ancora presenti nel
report**. La sovra-aggregazione è il fallimento invisibile che questo documento
temeva di più, e su traffico vero non è avvenuto.

**Parsing.** Zero righe scartate su entrambi — ma il merito resta del proxy
configurato per il collaudo, non della grammatica, come già annotato.

## Cosa si apre adesso, e non lo decide l'agente

Quattro conversazioni, una per famiglia. Nessuna è una taratura di soglia:
sono tutte decisioni di prodotto.

**Tutte e quattro corrette il 2026-09-06**, su richiesta esplicita del
fondatore. Le prime tre sono qui sotto con l'esito accanto; la quarta ha già la
sua nota.

1. **La finestra troppo corta.** Sette finding su dieci dicono la stessa cosa —
   *non lo so, il log è troppo breve* — una volta per ogni endpoint dichiarato
   non colpito. Il tool sa già che la finestra non basta: potrebbe dirlo **una
   volta sola**, con il conteggio, invece di 305 volte. Non è nascondere
   informazione (i conteggi del manifest restano interi): è deciderne la forma.
2. **I parametri di path letterali.** Su un'API basata su nomi, il match
   parziale scatta su ogni utente e ogni repository. È §P3 che funziona, e
   costa. Va deciso se un match parziale che si ripete identico su N valori
   diversi dello stesso posto sia N finding o uno.

   **Fatto, dalla stessa correzione.** Diciassette `/api/v1/users/<nome>` con la
   stessa spiegazione parola per parola diventano un gruppo solo. Il
   raggruppamento è **strutturale, non testuale**: la prima riga di evidenza è
   identica byte per byte fra i membri di una famiglia, perché nomina il path
   *dichiarato* e non quello osservato. Nessuna stringa viene analizzata per
   indovinare la famiglia.
3. **La UI web dentro il log.** Un log che contiene sia pagine sia API fa
   risultare ogni pagina un endpoint non documentato. Serve un modo di dire
   quale parte del traffico è l'API? È una chiave di configurazione nuova, e
   non la invento.

   **Fatto: `path_prefix`.** Opt-in, e il preimpostato resta «analizza tutto»,
   perché scegliere da soli quale traffico sia l'API significherebbe decidere di
   non guardare qualcosa senza dirlo. Le richieste fuori portata **non
   spariscono**: sono contate, e il conteggio esce su stderr, nel report e nel
   JSON. Un prefisso che non comincia con `/` è rifiutato, perché ridurrebbe
   l'analisi a zero in silenzio. `RulesetVersion` 0.7.0: cambia cosa viene
   analizzato, quindi sale.
4. **`{filepath}` che attraversa le barre.** Questo non era rumore da tarare: era
   un **buco nel confronto**. Un parametro dichiarato che contiene barre non
   poteva combaciare con niente, quindi ogni richiesta a quel path usciva come
   `Shadow` conclamato — con confidenza alta, e facendo uscire `3` in CI. Era il
   falso positivo più caro dei quattro, ed era l'unico che si correggeva nel
   codice invece che in una decisione.

   **Corretto il 2026-09-06** (`RulesetVersion` 0.6.0). Erano **18 dei 61
   `Shadow`** del report di Gitea, cioè il 30% dei conclamati. Ora un dichiarato
   più corto che finisce con una variabile produce `Undetermined`, con
   un'evidenza che dice **quanti segmenti** quella variabile dovrebbe
   inghiottire e che OpenAPI non ha modo di dichiararlo.

   La proprietà che rende accettabile la correzione, presidiata da una fixture:
   **un endpoint davvero non documentato resta `Shadow` conclamato.**
   `/api/v1/repos/collaudo/docs-portal/segreti/dump` esce ancora con uscita `3`.

   **Il prezzo, scritto perché sia rivedibile:** una sottorisorsa non
   documentata sotto una collezione dichiarata che finisce con una variabile —
   `/api/users/1/comments/5` contro `/api/users/{id}` — diventa `Undetermined`
   invece di `Shadow`, quindi non fa più fallire una pipeline. Resta segnalata,
   e l'evidenza dice che servirebbe un `{id}` lungo tre segmenti. È il prezzo
   che il blueprint paga già ovunque: `Shadow` significa «assente
   dall'inventario dichiarato», e finché un dichiarato potrebbe coprirlo
   quell'assenza non è verificabile. Se fosse troppo alto, la leva è limitare
   quanti segmenti una variabile può inghiottire — ed è una decisione del
   fondatore, non dell'agente. Una fixture chiamata `prezzo_noto_...` lo
   asserisce, così il giorno in cui si rivede quel test fallisce.
