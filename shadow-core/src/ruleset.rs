//! Il **ruleset**: soglie ed elenchi globali che decidono l'esito di un'analisi
//! (§7), versionati insieme come [`RULESET_VERSION`].
//!
//! Il criterio per stare qui è uno solo: **se cambiarla cambia l'esito di
//! un'analisi, è parte del ruleset**. Due run sullo stesso input devono poter
//! divergere solo se il ruleset è cambiato, e il `RunManifest` deve permettere
//! di ricostruire perché (§P4). Una costante che decide quali righe vengono
//! scartate non è un dettaglio di implementazione, per quanto lo sembri.
//!
//! Vivono in `shadow-core` perché §7 chiede che siano "definiti in un unico
//! posto": i crate che le applicano — `shadow-collectors` per l'ingestione —
//! le importano da qui invece di tenersene una copia.

/// Versione corrente del ruleset (§5, §7).
///
/// Cambia quando cambia **qualsiasi cosa che possa alterare l'esito di
/// un'analisi a parità di input** (§7): soglie, catalogo delle forme tipo-ID,
/// euristiche, motivi di scarto. **Non** cambia per rifattorizzazioni,
/// documentazione o modifiche alla sola presentazione — un'ottimizzazione che
/// dà lo stesso verdetto usando meno memoria non è un cambio di ruleset.
///
/// Schema: **minor** per aggiunte e modifiche di soglie, **major** solo se un
/// cambiamento rende i report vecchi non più confrontabili con i nuovi.
///
/// Storia: `0.0.0` in Fase 0, quando nessuna regola esisteva; `0.1.0` con le
/// soglie di ingestione e normalizzazione; `0.2.0` con le euristiche di Fase 3
/// e la ri-codifica delle graffe letterali nei path; `0.3.0` con i limiti di
/// lettura della specifica dichiarata e la severità che sale sui metodi non
/// dichiarati; `0.4.0` con le due tarature decise dopo il collaudo su corpus
/// reale — la grammatica di log **dichiarabile** e la scelta del path
/// dichiarato più vicino nell'evidenza; `0.5.0` con il riconoscimento delle
/// stringhe opache che guarda **dentro** gli escape percentuali prodotti dalla
/// normalizzazione, ignora la punteggiatura della frase attorno al valore, e si
/// applica anche al nome del file negli input del manifest; `0.6.0` con il
/// riconoscimento della variabile dichiarata che dovrebbe attraversare le barre,
/// che non è più `Shadow` conclamato ma `Undetermined`; `0.7.0` con
/// `path_prefix`, che delimita la parte di traffico analizzata.
///
/// Il raggruppamento dei finding nel report **non** ha fatto salire il numero:
/// cambia quante volte si legge la stessa frase, non ciò che il tool ha
/// concluso, ed è la lettera di §7 sulle modifiche alla sola presentazione.
///
/// Perché `0.4.0` e non un numero invariato, visto che nessuna soglia di questo
/// file è cambiata: la regola di §7 parla di *esito di un'analisi a parità di
/// input*, e un file che prima veniva rifiutato in blocco e ora viene analizzato
/// è un esito diverso sullo stesso input. Vale anche per l'evidenza: nomina un
/// path diverso, e un archivio di report va potuto confrontare sapendo perché.
/// Il precedente della v1.7 — *un avviso non è un verdetto* — non copre questo
/// caso: parla degli avvisi, e l'evidenza sta dentro il `Finding`.
///
/// Perché `0.5.0` per un cambio di **redazione**, che sembra presentazione: perché
/// §7 elenca *i pattern strutturali dei segreti* fra le cose che stanno nel
/// ruleset, per nome. Non è un'interpretazione, è la lettera dell'elenco, e
/// applicarla è ciò che permette a un archivio di report di dire perché due
/// esecuzioni mostrano cose diverse. Il rifiuto dei valori booleani non
/// riconosciuti **non** contribuisce: sta nella lettura della configurazione,
/// che l'elenco non nomina, e a configurazione valida non cambia niente.
pub const RULESET_VERSION: &str = "0.8.0";

/// Lunghezza massima di una riga di log tenuta in memoria: oltre, la riga viene
/// scartata come `line-too-long` (§7, Fase 1).
///
/// Decide cosa viene scartato, quindi è parte del ruleset. È anche ciò che
/// impedisce a un file **senza ritorni a capo** di trasformarsi in
/// un'allocazione grande quanto il file, cioè a §P10 di essere aggirato da un
/// input malformato.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Quante righe non vuote si esaminano prima di dichiarare che il formato non è
/// quello atteso (§7, Fase 1).
///
/// Serve a fallire **presto**: senza, un file nel formato sbagliato verrebbe
/// letto per intero prima di scoprire che non se n'è capito niente.
pub const FORMAT_SAMPLE_LINES: u64 = 100;

/// Quanti valori distinti conformi a un tipo tipo-ID devono comparire **nella
/// stessa posizione e sotto lo stesso prefisso** perché quella posizione sia
/// considerata variabile (§7, Fase 2).
///
/// È metà della "evidenza forte" richiesta da §P3; l'altra metà è la conformità
/// del singolo valore a una forma tipo-ID. Servono **entrambe**: la cardinalità
/// da sola fonderebbe `/api/users/admin` dentro `/api/users/{id}`.
pub const MIN_DISTINCT_VALUES_FOR_VARIABLE: usize = 3;

/// Segnaposto con cui un segmento variabile compare in un `EndpointPattern`
/// (§6: es. `/api/users/{id}`).
pub const ID_PLACEHOLDER: &str = "{id}";

/// Lunghezze di stringa esadecimale riconosciute come forma tipo-ID (§7).
///
/// Sono le forme opache diffuse: ObjectId (24), MD5 (32), SHA-1 (40),
/// SHA-256 (64). L'elenco è deliberatamente corto e **non** include le
/// lunghezze brevi: `cafe`, `dead`, `face`, `added` sono esadecimali validi e
/// sono parole. Ogni forma aggiunta qui è un modo in più di inghiottire una
/// parola scambiandola per un identificatore (§P3).
pub const HEX_ID_LENGTHS: &[usize] = &[24, 32, 40, 64];

/// Giorni oltre i quali un endpoint dichiarato ma non osservato è considerato
/// `Zombie`, quando l'utente non specifica `zombie_staleness_days` (§7).
///
/// È una regola che cambia i verdetti, quindi vive qui: due run sullo stesso
/// log possono dare `Zombie` o `Undetermined` a seconda di questo numero.
pub const DEFAULT_ZOMBIE_STALENESS_DAYS: i64 = 30;

/// Segmenti di path che fanno sospettare una superficie non di produzione
/// (§7, Fase 3).
///
/// L'elenco è corto e viene direttamente dagli esempi di §7. Ogni voce in più è
/// un modo in più di produrre rumore: un tool che segnala duecento cose di cui
/// centonovanta innocue viene spento alla terza esecuzione, e a quel punto non
/// segnala più niente. Meglio pochi segnali forti.
pub const SUSPICIOUS_PATH_SEGMENTS: &[&str] = &["test", "debug", "old", "internal"];

/// Dimensione massima di una specifica dichiarata, oltre cui il file viene
/// rifiutato (§7, Fase 4).
///
/// La specifica è input non fidato quanto un log. Le specifiche reali stanno
/// abbondantemente sotto questo limite; ciò che lo supera o è un errore
/// dell'utente o è un file costruito apposta.
pub const MAX_SPEC_BYTES: usize = 8 * 1024 * 1024;

/// Profondità massima di annidamento in una specifica dichiarata (§7, Fase 4).
///
/// Un documento annidato più a fondo di così non descrive un'API: descrive un
/// tentativo di far ricorrere qualcosa finché non si rompe.
pub const MAX_SPEC_DEPTH: usize = 64;

/// Dice se un carattere deve essere **ri-codificato** in `%XX` dentro un
/// segmento canonico di path (§7, Fase 2).
///
/// # Le cinque famiglie, e perché sono qui e non altrove
///
/// 1. `/` — o un segmento si spaccerebbe per due.
/// 2. `%` — o la doppia codifica collasserebbe sulla singola.
/// 3. `{` e `}` — o un path che contiene letteralmente `{id}` si confonderebbe
///    con il segnaposto dei segmenti variabili.
/// 4. **Caratteri di controllo**, C0 e **C1**: illeggibili in un report.
/// 5. **Caratteri invisibili e di direzione**: spazio a larghezza zero, unione
///    a larghezza zero, spazi esotici, marcatori bidirezionali, indicatore di
///    ordine dei byte.
///
/// La quinta famiglia è arrivata dopo, da una revisione avversaria, e non è un
/// allargamento della regola: è la sua ultima riga applicata alla lettera —
/// *«i caratteri di controllo (illeggibili in un report, o peggio)»*. **Il «o
/// peggio» è questo.**
///
/// Due esempi misurati sul binario, prima della correzione:
///
/// - `/api/admin` e `/api/ad<U+200B>min` uscivano **tipograficamente
///   identici** nel report, nella pagina e nel documento d'audit, pur essendo
///   due endpoint diversi. Un endpoint che sparisce alla vista di chi decide è
///   il fallimento che §P1 chiama il peggiore del prodotto, e qui spariva senza
///   che nessuna regola risultasse violata;
/// - `<U+202E>` rovescia il testo che segue: `/api/<U+202E>txt.exe` si legge
///   `/api/exe.txt`. Un path può quindi **presentarsi come qualcosa che non
///   è**, in un documento che va in mano a un auditor.
///
/// Ri-codificare non nasconde e non fonde niente: rende **visibile** ciò che
/// era invisibile, e la trasformazione è reversibile. È l'opposto di
/// un'aggregazione, ed è per questo che non tocca §P3.
///
/// **Ciò che non entra qui:** lettere accentate, ideogrammi, alfabeti non
/// latini. Sono caratteri che si vedono, e ri-codificarli renderebbe
/// illeggibile un path perfettamente normale — cioè il difetto che questa
/// correzione esiste per non causare.
pub fn must_be_reencoded(ch: char) -> bool {
    if matches!(ch, '/' | '%' | '{' | '}') {
        return true;
    }
    // `is_control` copre C0, DEL **e C1** (`U+0080`-`U+009F`), che il controllo
    // per byte di prima si lasciava sfuggire.
    if ch.is_control() {
        return true;
    }
    matches!(ch,
        // Trattino morbido e marcatori di direzione isolati.
        '\u{00AD}' | '\u{061C}' | '\u{180E}'
        // Spazio a larghezza zero, unione/non-unione, marcatori sinistra-destra.
        | '\u{200B}'..='\u{200F}'
        // Incorporamento e **override** bidirezionale: il carattere che fa
        // leggere un path al contrario.
        | '\u{202A}'..='\u{202E}'
        // Giuntore di parole e operatori invisibili.
        | '\u{2060}'..='\u{2064}'
        // Isolamenti bidirezionali.
        | '\u{2066}'..='\u{2069}'
        // Indicatore di ordine dei byte / spazio insecabile a larghezza zero.
        | '\u{FEFF}'
        // Annotazione interlineare.
        | '\u{FFF9}'..='\u{FFFB}'
        // Spazi che non si vedono o che non si distinguono da uno spazio
        // normale: due path che differiscono solo per questi sarebbero due
        // endpoint indistinguibili a occhio.
        | '\u{00A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}'
        | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}
