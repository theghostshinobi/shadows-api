# L'interfaccia di supervisione — studio, prima di intervenire

Richiesto dal fondatore il 2026-09-06: *«1, 2, 3, gestiamo anche l'opzione di
utilizzo, magari facciamo tramite npm oppure un qualcosa che da terminale ti dà
più opzioni? studiamocela prima di intervenire»*.

Questo documento **non implementa niente**. Mette sul tavolo cosa costa ciascuna
forma, dove tocca i pilastri, e quali decisioni restano tue.

---

## Cosa c'è oggi

Quattro comandi, e nessuno di loro apre un socket:

| Comando | Cosa dà |
|---|---|
| `shadow <log>` | Analisi singola, report su stdout in terminale / JSON / Markdown |
| `shadow daemon <log>` | Guarda nel tempo, scrive nello storico, dice su stderr cosa è nuovo |
| `shadow alerts` | Gli endpoint shadow comparsi e non ancora presi in carico |
| `shadow compliance` | L'inventario per un auditor, in CSV / Markdown / JSON |

**Quello che manca non è un dato: è una vista.** Lo storico contiene già tutto —
inventario, classificazione, stato dell'auth con la base di calcolo, alert
pendenti, storia dei run — e nessuna delle tre forme che hai scelto deve
calcolare niente di nuovo. Sono tre modi di guardare la stessa tabella.

Questo è il punto che rende la scelta meno drammatica di quanto sembri: **si può
fare una forma alla volta senza pentirsene**, perché nessuna delle tre pregiudica
le altre.

---

## Le tre forme, e cosa costano davvero

### 1. HTML statico su file — `shadow dashboard --out stato.html`

Un file autonomo: apri con doppio clic, nessun server, nessuna porta, nessuna
dipendenza da installare. Dentro c'è quello che oggi leggi in tre comandi, più
l'andamento nel tempo che oggi non si vede da nessuna parte.

- **Costo:** basso. Generare HTML da dati che già abbiamo, senza librerie
  esterne. Il grosso del lavoro è il disegno, non il codice.
- **Cosa dà di unico:** si **condivide**. Lo mandi al responsabile, lo alleghi a
  un ticket, lo archivi con la data. È la forma che regge un audit.
- **Cosa non dà:** non si aggiorna da solo. È una fotografia con una data
  sopra — che per un audit è un pregio, per una supervisione dal vivo un limite.
- **Pilastri:** nessuna tensione. Zero socket, e il file passa dal `Redactor`
  come ogni altro output.
- **Attenzione, e non è piccola:** un HTML si apre in un browser, e un browser
  esegue. Se un pattern di endpoint finisse dentro la pagina senza essere
  neutralizzato, un path costruito ad arte diventerebbe **codice che gira nel
  browser di chi legge il report** — cioè §P5 rovesciato: non un segreto che
  esce, ma un'iniezione che entra. Serve la stessa disciplina del `table_cell`
  del Markdown, un piano più su, e una fixture avversaria con un path che prova
  a chiudere un tag.

### 2. TUI — `shadow watch`

Una schermata che si aggiorna: alert in evidenza, inventario navigabile,
l'ultimo giro del demone in tempo reale.

- **Costo:** medio, e con una dipendenza nuova (`ratatui` o simile) che oggi non
  abbiamo. Su uno strumento di sicurezza ogni dipendenza è superficie: va pesata,
  non aggiunta di slancio.
- **Cosa dà di unico:** è **viva** senza aprire niente. È la forma giusta per
  «sto guardando adesso mentre il demone gira».
- **Cosa non dà:** non si condivide, non si archivia, non si guarda da un'altra
  macchina.
- **Pilastri:** nessuna tensione con P6.

### 3. Server locale su `127.0.0.1` — `shadow serve`

L'unica forma che dà davvero una UI web che si aggiorna e che si guarda da un
browser qualsiasi.

- **Costo:** alto, e non per le pagine. Un server è **stato condiviso**,
  concorrenza, e soprattutto una **superficie di rete**.
- **Cosa dà di unico:** più persone guardano la stessa cosa insieme, da postazioni
  diverse, senza copiare file.
- **La tensione, per intero.** Ieri, decisione 36, hai scelto l'alert senza
  egress con questa motivazione: *«la frase "questo binario non apre socket"
  resta verificabile dall'esterno»*. Un server la rompe. Non per esfiltrazione —
  è in ascolto, non chiama nessuno — ma perché quella frase è **binaria**: o è
  vera, o diventa «non apre socket a meno che».

  Non è una questione di gusto. È il genere di frase che si mette in un README di
  uno strumento di sicurezza e su cui qualcuno decide di fidarsi.

  Se la vuoi, la mitigazione che la mantiene **quasi** vera esiste ed è nota:
  metterlo dietro una **feature di compilazione**, così esiste una build di cui
  si può ancora dire *non apre socket* in senso pieno, e chi la vuole senza
  compila l'altra. Con quella, la frase diventa «la build predefinita non apre
  socket», che è ancora verificabile.

  Restano poi tre decisioni che non prendo io: se legare solo a `127.0.0.1` o
  permettere altro; se serva un'autenticazione (e su una macchina condivisa
  serve, perché `127.0.0.1` non è una barriera fra utenti dello stesso host); e
  se il server debba poter **scrivere** — prendere in carico un alert da una
  pagina web è comodo, ed è anche il primo passo verso un'API di scrittura su uno
  strumento offline.

---

## La domanda sull'opzione di utilizzo: npm, o altro

Qui la tua domanda ne conteneva due, e conviene separarle.

### «Tramite npm» — è una domanda di **distribuzione**, non di interfaccia

Shadow è un binario Rust. Oggi si distribuirebbe con `cargo install`, un
pacchetto per sistema operativo, o un file da scaricare. npm è una quarta strada,
e funziona: `esbuild`, `swc` e altri la usano — si pubblicano i binari per
piattaforma e un pacchetto JS che al momento dell'installazione sceglie quello
giusto.

**Cosa risolve davvero:** `npx shadow ...` senza installare niente, e
l'inserimento naturale in una pipeline CI che ha già Node. Per un tool che vuole
stare in una CI, non è poco.

**Cosa costa, e su questo prodotto pesa più che altrove:**

- il modo classico di fare questo pacchetto è uno **script di postinstall che
  scarica un binario dalla rete**. È *esattamente* il pattern che i registri
  pubblici hanno visto usare per compromettere catene di fornitura, ed è ciò da
  cui §P6 e l'offline-first tengono Shadow lontano. Il modo pulito è pubblicare i
  binari **dentro** i pacchetti per piattaforma con `optionalDependencies`, senza
  postinstall e senza scaricare niente — si può fare, va fatto così, e va
  detto nel README perché è una promessa verificabile;
- diventa una **seconda catena di rilascio** da tenere allineata alla prima,
  e un disallineamento fra `cargo install shadow` e `npx shadow` sarebbe un
  disastro d'audit: due versioni con lo stesso nome che danno verdetti diversi;
- il nome sul registro è pubblico, e §3 dice che **«Shadow» è un nome di
  lavoro** con un possibile conflitto ancora da verificare. Pubblicare su npm
  significa prendere quel nome davvero. Questa è la ragione per cui la
  distribuzione **non si decide prima del nome**.

### «Qualcosa che da terminale ti dà più opzioni»

Questa la leggo come: *i quattro comandi non bastano a orientarsi*. Se è così, la
risposta non è npm ed è più economica di una UI:

- **`shadow status`** — una schermata sola, non interattiva: quanti endpoint,
  quanti alert aperti, quando è stato l'ultimo giro, cosa è cambiato. È il
  comando che oggi manca davvero, e costa poco.
- **`shadow watch`** — la forma 2, cioè `status` che si aggiorna.
- Un aiuto che **guidi** invece di elencare: oggi `--help` mostra le opzioni, non
  i percorsi d'uso («analizza una volta», «sorveglia», «esporta per l'audit»).

---

## Come le metterei in fila, se decidessi io — ma non decido io

1. **`shadow status`.** Piccolo, utile subito, e diventa il contenuto delle altre
   due forme: quello che scrive `status` è ciò che la TUI aggiorna e ciò che
   l'HTML impagina. Farlo per primo evita di scrivere tre volte la stessa vista.
2. **HTML statico.** Copre «vedere e supervisionare» e in più si condivide e si
   archivia, che è ciò che un audit chiede. Nessuna tensione con i pilastri,
   purché si tratti l'HTML come output ostile in uscita.
3. **TUI.** Quando serve guardare mentre il demone gira.
4. **Server locale.** Solo se serve davvero guardare da un'altra macchina, e solo
   con la decisione esplicita su P6 — dietro feature di compilazione, così la
   frase resta verificabile per la build predefinita.

**La distribuzione (npm o altro) viene dopo il nome**, non prima: §3 tiene
«Shadow» come nome di lavoro, e un pacchetto pubblico lo renderebbe definitivo di
fatto.

---

## Cosa mi serve da te per procedere

1. **In quale ordine**, o se ne vuoi una sola.
2. **Sul server locale (forma 3):** la feature di compilazione ti basta come
   mitigazione, o vuoi che «non apre socket» resti vero senza eccezioni?
3. **`shadow status` prima delle viste**, sì o no — è la scelta che decide se le
   tre forme condividono una vista sola o ne scrivono tre.
4. **La distribuzione**, e con essa il **nome**: sono la stessa decisione, e §3
   la tiene aperta da undici revisioni.

---

# La conclusione dello studio: quale interfaccia grafica

**HTML statico, riscritto dal demone a ogni giro.** Non per prudenza su P6 —
quello viene dopo — ma per due ragioni che stanno scritte nei tuoi documenti.

## La prima ragione: il ritmo del prodotto

Un server compra **liveness**, e la liveness non è ciò di cui questo prodotto ha
bisogno. Nessuno fissa un cruscotto di endpoint shadow in tempo reale: gli
endpoint compaiono nell'arco di giorni, e la decisione che ne segue — *questo
lo guardo, questo lo metto in allowlist* — ha cadenza quotidiana o settimanale.
Pagare una superficie di rete per un aggiornamento al secondo su un fenomeno che
si muove in giorni è comprare la cosa sbagliata.

## La seconda ragione: §1, e non è un'opinione

> *«Non è "il prossimo Akamai/Salt". Niente monitoraggio continuo multi-team come
> requisito del prodotto base.»*

Una console web viva, guardata da più persone insieme da postazioni diverse, **è**
monitoraggio continuo multi-team. Non è un effetto collaterale del server: è la
sola cosa che il server dia in più rispetto a un file. Costruirla significherebbe
costruire verso un non-obiettivo dichiarato.

§2 prevede una *dashboard* in Fase 5, ed è coerente: una dashboard che si guarda
non è una console che si presidia.

## Il pezzo che chiude il divario, e che non mi aspettavo

Un file **non è** una fotografia morta, se qualcuno lo riscrive.

Il demone gira già a intervalli. Se a ogni giro riscrive il file, e la pagina
porta un `<meta http-equiv="refresh">`, un browser lasciato aperto **segue da
solo**. Nessun socket, nessun JavaScript, nessuna dipendenza: il browser rilegge
un file da disco.

Questo copre quasi tutto ciò che volevi da un server — *vedere e supervisionare*,
in una finestra che resta aperta — e lascia intatta la frase della decisione 36.
Su una cartella condivisa lo vede anche chi sta su un'altra macchina.

Quello che **non** copre, e va detto: più persone che agiscono insieme (prendere
in carico un alert dalla pagina), e una sola vista su venti bersagli sorvegliati
contemporaneamente. Sono i due casi in cui il server vince davvero — ed è il
multi-team di §1.

## Che forma ha la pagina

Una pagina, un bersaglio, quattro blocchi in quest'ordine — che è l'ordine in cui
uno se li chiede:

```
┌───────────────────────────────────────────────────────────────┐
│  produzione            ultimo giro: 2026-09-06 14:32 (run 412) │
│                                                                │
│  COSA È CAMBIATO          3 endpoint nuovi dall'ultimo giro    │
│  ──────────────────       1 alert aperto da 4 giorni           │
│                                                                │
│  ALERT APERTI                                                  │
│  /api/admin/reset     Shadow · visto la prima volta al run 380 │
│  ...                                                           │
│                                                                │
│  INVENTARIO      118 endpoint                                  │
│  path            metodi  osservazioni  classe   auth (base)    │
│  ...                                     4 valori, mai binario │
│                                                                │
│  ANDAMENTO       endpoint e alert per run, ultimi N giri       │
│                                                                │
│  PROVENIENZA     shadow 0.0.0 · ruleset 0.7.0 · input <path>/… │
└───────────────────────────────────────────────────────────────┘
```

Il primo blocco è quello per cui la pagina esiste: **cosa è cambiato**. Oggi non
c'è da nessuna parte — né in `alerts`, né in `compliance` — e il demone lo dice
solo su stderr, dove scorre via.

L'ultimo blocco è quello che la rende un documento invece di una schermata: chi
l'ha prodotta, con quale ruleset, su quale input. Le stesse cose del
`RunManifest`, per la stessa ragione.

## Le regole che questa pagina deve rispettare, e non sono le solite

- **Autonoma davvero:** nessun CDN, nessun font remoto, nessuno script esterno.
  Non è una preferenza di stile: una pagina che scarica qualcosa all'apertura
  contraddice l'offline-first davanti agli occhi di chi la guarda.
- **L'HTML è un'uscita ostile.** Un browser esegue. Un pattern di endpoint viene
  dai dati analizzati, e se finisse nella pagina senza essere neutralizzato, un
  path costruito ad arte diventerebbe codice nel browser di chi legge il report:
  §P5 rovesciato — non un segreto che esce, un'iniezione che entra. Serve la
  disciplina del `table_cell` del Markdown, un piano più su, e una fixture
  avversaria con un path che prova a chiudere un tag.
- **Redatta come tutto il resto**, e `--show-raw-values` vale anche qui.
- **Deterministica** (§P4): stesso storico, stesso file — salvo il momento in cui
  è stata generata.

## Cosa **non** faccio, e perché

**Il server locale: no, per ora.** Non perché sia impossibile — la mitigazione
esiste, una feature di compilazione — ma perché l'unica cosa che aggiunge è il
multi-team di §1. Il giorno in cui quel non-obiettivo cambia, il server si
aggiunge senza buttare niente: la pagina che scrive il file è la stessa che
servirebbe il server.

**La TUI: dopo, e forse mai.** Quello che darebbe è `shadow status` che si
aggiorna. Se `status` esiste, la TUI è un'aggiunta di poche righe con una
dipendenza nuova; se non esiste, la TUI è il modo caro di ottenerlo. Prima
`status`.

## Cosa mi serve da te

1. **Confermi l'HTML statico** come la forma, con il demone che lo riscrive.
2. **Il blocco «cosa è cambiato»**: rispetto all'ultimo giro, o rispetto
   all'ultima volta che *tu* hai guardato? La seconda è più utile e richiede di
   ricordare una presa visione — cioè una colonna in più nello storico, e una
   decisione tua.
3. **Un bersaglio per pagina o tutti insieme?** Con la separazione dei bersagli
   che hai scelto (decisione 37), un file per bersaglio è la lettura coerente.
4. **`shadow status` prima**, sì o no.
