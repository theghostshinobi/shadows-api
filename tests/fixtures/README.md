# Corpus di test dorato

Coppie *(log di input + risultato atteso)* usate per verificare il motore
(§8, §10 del blueprint). Cresce a ogni fase, e vale la regola: **nessun
cancello di fase è verde senza le sue fixture verdi** (§10).

## Stato

**Fase 0: nessuna fixture, e correttamente** — non esisteva output di analisi di
cui fissare l'atteso. Le verifiche appropriate a quella fase sono i test sui
contratti in `shadow-core/tests/` (§11).

**Fase 1: presenti** in `nginx-combined/`, esercitate da
`shadow-collectors/tests/ingest_nginx.rs` (contenuto e conteggi) e da
`shadow-cli/tests/cancello_fase1.rs` (il cancello, eseguendo il binario).

| File | Cosa presidia |
|---|---|
| `nginx-combined/valid.log` | La fixture dorata: 5 righe valide, query multi-valore, un `$remote_user` valorizzato, un fuso `+0200` da portare a UTC |
| `nginx-combined/corrupted-midfile.log` | Riga corrotta a metà file, status impossibile, data impossibile: contate con il perché, l'analisi prosegue |
| `nginx-combined/unrecognised-format.log` | Un log strutturato di altro tipo: il tool si rifiuta invece di indovinare (§P2) |
| `nginx-combined/dirty-encoding.log` | Byte non UTF-8 in mezzo: riga scartata, nessun crash, digest comunque del file intero |
| `nginx-combined/admin-hidden.log` | **Fase 2**: 50 identificatori numerici con dentro `/api/users/admin` e `/api/users/me`. Gli id collassano, le due parole restano visibili |
| `nginx-combined/encoded-paths.log` | **Fase 2**: `..` e `%2e%2e` sono lo stesso endpoint; la doppia codifica resta distinta; `%2f` non si fonde con un `/` reale |
| `nginx-combined/long-window.log` | **Fase 3**: traffico su 64 giorni, l'unica finestra abbastanza lunga per poter dire `Zombie` |
| `nginx-combined/suspicious-paths.log` | **Fase 3**: due path sospetti e due innocui — senza specifica devono uscire due sospetti, non quattro |
| `openapi/current.json` | **Fase 3**: la specifica allineata al traffico |
| `openapi/stale.json` | **Fase 3**: la specifica **stantia**, che dichiara `/api/users/{userId}` mentre nel traffico c'è `/api/users/admin` |
| `openapi/spec.yaml` | **Fase 4**: una specifica in YAML, il formato in cui è scritta la maggior parte degli OpenAPI reali |
| `openapi/bomb.yaml` | **Fase 4**: una **bomba di espansione YAML** — poche righe che espanse occupano gigabyte. Va rifiutata *prima* del parser |
| `nginx-combined/secrets.log` | **Fase 4**: un JWT, un'email, una chiave opaca e una password in querystring. Nessuno deve comparire nell'output di default |
| `allowlist/known-noise.allow` | **Fase 4**: una regola precisa e una deliberatamente troppo ampia, per verificare che la seconda faccia scattare l'avviso |
| `log-format-variants/` (sette file) | **Taratura post-collaudo**: le sette varianti reali di `log_format` misurate dal collaudo. Prima ne passava una; ora passano tutte **dichiarandole**, e senza dichiarazione ne passa ancora una sola. Copia byte-identica di `collaudo/corpus/varianti-log-format/`, e un test lo verifica |

Alcune fixture sono **generate dal test**, non versionate, perché sarebbero file
enormi o degeneri: il file grande (la stessa fixture ripetuta 20.000 volte), la
riga senza ritorno a capo lunga tre volte il limite, e la versione a **5.000
identificatori** dell'ago nel pagliaio — la scala che §9 descrive con "migliaia
di ID".

## Cosa dovrà contenere, fase per fase (§10)

Oltre alle fixture "normali", il blueprint rende **obbligatorie** queste fixture
avversarie:

| Fixture avversaria (✅ = già verde) | Cosa deve dimostrare | Fase |
|---|---|---|
| ✅ Riga malformata in mezzo al file | La riga viene saltata e contata, il run prosegue | 1 |
| ✅ File molto grande | Memoria piatta a prescindere dalla dimensione (§P10) | 1 |
| ✅ `log_format` non riconosciuto | Errore chiaro, mai estrazione best-effort silenziosa (§P2) | 1 |
| ✅ Encoding sporco / byte non validi | Trattati come riga malformata, nessun crash | 1 |
| ✅ Endpoint-admin nascosto fra migliaia di ID | `/api/users/admin` **non** viene fuso in `/api/users/{id}` (§P3) | 2 |
| ✅ Path con encoding (`%2e%2e`, doppio encoding) | Normalizzazione consistente senza perdere il segnale | 2 |
| ✅ OpenAPI stantio / bugiardo | Match incerto → `Undetermined`, mai `Known` (§P9) | 3 |
| ✅ Log il cui formato non trasporta l'autenticazione (nginx `combined`) | Il pattern risulta `NotObservable`, **nessuna** euristica sull'assenza di auth si attiva, e all'utente viene detto che l'informazione mancava (§6, Fase 3) | 3 |
| ✅ Log contenente segreti (token in URL, email in querystring) | Redazione attiva di default in ogni output e file persistito (§P5) | 4 |
| ✅ Run con soli `Undetermined` | Exit code `0`, ma gli `Undetermined` visibili nel report e nei conteggi del manifest (§7) | 4 |
| ✅ Run con almeno un `Shadow`/`Zombie` | Exit code `3`, così una pipeline CI può fallire di proposito (§7) | 4 |
| ✅ Bomba di espansione YAML | Rifiutata prima del parser: gli alias non vengono espansi (§7) | 4 |
| ✅ Allowlist con regola troppo ampia | Avviso su stderr; i finding silenziati restano nei conteggi del manifest | 4 |
| ✅ Variante reale di `log_format` non prevista | Rifiutata senza dichiarazione, leggibile dichiarandola, mai letta a campi spostati (§P2) | taratura |
| ✅ Dichiarazione di `log_format` ambigua o incompleta | Rifiutata **prima** di aprire il file: nessun manifest, nessun verdetto (§7) | taratura |
| ✅ Più match parziali sullo stesso osservato | L'evidenza nomina il più vicino, e ammette il pareggio quando c'è (§P4, §P9) | taratura |
| Stesso input, due esecuzioni | Verdetto identico, manifest a parte il timestamp (§P4) | 4 |
| Finding in allowlist | Assente dal report, ma ancora presente nei conteggi del manifest | 4 |
