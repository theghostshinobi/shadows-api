//! La vista come **pagina HTML autonoma**.
//!
//! # Perché autonoma davvero
//!
//! Nessun CDN, nessun font remoto, nessuno script esterno, nessuna immagine da
//! scaricare. Non è una preferenza di stile: una pagina che va a prendere
//! qualcosa all'apertura contraddice l'offline-first davanti agli occhi di chi
//! la guarda, e su uno strumento di sicurezza quella contraddizione è la cosa
//! che uno nota per prima.
//!
//! # L'HTML è un'uscita **ostile**
//!
//! Un browser esegue. I pattern di endpoint vengono dai dati analizzati, e un
//! path costruito ad arte che finisse nella pagina senza essere neutralizzato
//! diventerebbe codice nel browser di chi legge il report: **§P5 rovesciato** —
//! non un segreto che esce, un'iniezione che entra.
//!
//! Quindi qui dentro **nessuna stringa che venga dai dati raggiunge la pagina
//! senza passare da [`escape`]**, e non ci sono eccezioni «tanto questo campo è
//! nostro»: un campo nostro oggi è un campo dei dati domani.

use std::fmt::Write as _;
use std::io::Write;

use crate::Status;

/// Ogni quanto la pagina si rilegge da sola, quando lo si chiede.
///
/// Trenta secondi: il demone guarda ogni minuto, quindi una pagina che si
/// rilegge più spesso non mostrerebbe niente di nuovo, e una che si rilegge più
/// di rado farebbe aspettare.
pub const DEFAULT_REFRESH_SECONDS: u32 = 30;

/// Scrive la pagina.
///
/// `refresh` accende il `<meta http-equiv="refresh">`: è ciò che permette a un
/// browser lasciato aperto di seguire il demone **senza un socket**. Rilegge un
/// file da disco, e basta.
pub fn dashboard(out: &mut impl Write, status: &Status, refresh: Option<u32>) {
    let _ = out.write_all(page(status, refresh).as_bytes());
}

/// La pagina come stringa, per chi la deve servire invece di scriverla.
pub fn page(status: &Status, refresh: Option<u32>) -> String {
    let mut html = String::with_capacity(8 * 1024);
    let _ = write!(html, "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    let _ = write!(
        html,
        "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">"
    );
    if let Some(seconds) = refresh {
        let _ = write!(html, "<meta http-equiv=\"refresh\" content=\"{seconds}\">");
    }
    let _ = write!(html, "<title>Shadow — {}</title>", escape(&status.target));
    let _ = write!(html, "<style>{STYLE}</style></head><body>");

    header(&mut html, status);
    changed(&mut html, status);
    alerts(&mut html, status);
    inventory(&mut html, status);
    trend(&mut html, status);
    provenance(&mut html, status);

    let _ = write!(html, "</body></html>");
    html
}

fn header(html: &mut String, status: &Status) {
    let _ = write!(html, "<header><h1><span class=\"mark\">&#8801;</span> {}</h1>", escape(&status.target));
    match &status.last_run {
        Some(run) => {
            let _ = write!(
                html,
                "<p class=\"sub\">run {} · {} · {} endpoint(s) · {} line(s) analysed</p>",
                run.run_id,
                escape(&run.timestamp),
                run.endpoints,
                run.lines
            );
        }
        None => {
            let _ = write!(
                html,
                "<p class=\"sub\">nothing has been analysed yet for this target</p>"
            );
        }
    }
    let _ = write!(html, "</header>");
}

fn changed(html: &mut String, status: &Status) {
    let _ = write!(html, "<section><h2>What changed</h2>");
    if status.new_in_last_run.is_empty() {
        let _ = write!(
            html,
            "<p class=\"quiet\">Nothing new in the last run.</p></section>"
        );
        return;
    }
    let _ = write!(
        html,
        "<p><strong>{}</strong> endpoint(s) appeared for the first time in the last run.</p><ul class=\"list\">",
        status.new_in_last_run.len()
    );
    for endpoint in &status.new_in_last_run {
        let _ = write!(
            html,
            "<li><code>{}</code> <span class=\"tag\">{}</span></li>",
            escape(&endpoint.redacted_pattern),
            escape(endpoint.last_classification.as_deref().unwrap_or("no verdict"))
        );
    }
    let _ = write!(html, "</ul></section>");
}

fn alerts(html: &mut String, status: &Status) {
    let _ = write!(html, "<section><h2>Open alerts</h2>");
    if status.open_alerts.is_empty() {
        let _ = write!(
            html,
            "<p class=\"quiet\">No unacknowledged shadow endpoints.</p></section>"
        );
        return;
    }
    let _ = write!(html, "<ul class=\"list\">");
    for alert in &status.open_alerts {
        let _ = write!(
            html,
            "<li><code>{}</code> <span class=\"quiet\">first seen in run {}</span></li>",
            escape(&alert.redacted_pattern),
            alert.first_seen_run
        );
    }
    let _ = write!(
        html,
        "</ul><p class=\"quiet\">Acknowledge one with <code>shadow alerts --acknowledge &lt;id&gt;</code>.</p></section>"
    );
}

fn inventory(html: &mut String, status: &Status) {
    let _ = write!(
        html,
        "<section><h2>Inventory <span class=\"quiet\">{} endpoint(s)</span></h2><div class=\"chips\">",
        status.inventory.len()
    );
    for (name, count) in status.by_classification() {
        let _ = write!(
            html,
            "<span class=\"chip\"><b>{count}</b> {}</span>",
            escape(name)
        );
    }
    let _ = write!(
        html,
        "</div><div class=\"scroll\"><table><thead><tr><th>Endpoint</th><th>Methods</th><th>Requests</th><th>Class</th><th>Authentication</th><th>Observable</th></tr></thead><tbody>"
    );
    for row in &status.inventory {
        let _ = write!(
            html,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&row.redacted_pattern),
            escape(&row.methods.join(", ")),
            row.observation_count,
            escape(row.last_classification.as_deref().unwrap_or("no verdict")),
            escape(auth_label(row.auth_observed.as_deref())),
            row.auth_observable_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "not recorded".to_string()),
        );
    }
    let _ = write!(html, "</tbody></table></div></section>");
}

/// I quattro valori di §6, mai collassati, più il quinto dello storico.
fn auth_label(value: Option<&str>) -> &str {
    match value {
        Some("yes") => "yes",
        Some("no") => "no",
        Some("mixed") => "mixed",
        Some("notobservable") | Some("not observable") => "not observable",
        _ => "not recorded",
    }
}

fn trend(html: &mut String, status: &Status) {
    if status.trend.len() < 2 {
        return;
    }
    let peak = status
        .trend
        .iter()
        .map(|point| point.endpoints)
        .max()
        .unwrap_or(1)
        .max(1);
    let _ = write!(
        html,
        "<section><h2>Trend <span class=\"quiet\">last {} run(s)</span></h2><div class=\"bars\">",
        status.trend.len()
    );
    for point in &status.trend {
        // Un grafico fatto di divs: nessuna libreria, nessuno script, e si
        // stampa. La percentuale è calcolata qui e non è testo dei dati.
        let height = (point.endpoints * 100 / peak).max(2);
        let _ = write!(
            html,
            "<span class=\"bar\" title=\"run {} — {} endpoint(s)\"><i style=\"height:{}%\"></i></span>",
            point.run_id, point.endpoints, height
        );
    }
    let _ = write!(
        html,
        "</div><p class=\"quiet\">Runs are ordered by run number, not by clock: a clock that goes backwards must not reorder history.</p></section>"
    );
}

fn provenance(html: &mut String, status: &Status) {
    let _ = write!(
        html,
        "<footer><p>shadow {} · ruleset {}</p><p><strong>Shadow is a discovery aid, not a security certification.</strong> This page covers only endpoints that appeared in the analysed logs. The authentication column carries four distinct values and never collapses them into present/absent: <em>not observable</em> means the log format does not carry the information, and is not evidence that authentication was missing.</p></footer>",
        escape(&status.shadow_version),
        escape(&status.ruleset_version)
    );
}

/// Neutralizza una stringa destinata alla pagina.
///
/// **Nessuna stringa che venga dai dati raggiunge la pagina senza passare da
/// qui.** Si neutralizzano anche `'` e `"` — non solo i tre caratteri che
/// «bastano» nel corpo del documento — perché il giorno in cui una di queste
/// stringhe finisse dentro un attributo, il buco esisterebbe già e nessuno se
/// ne accorgerebbe.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Lo stile, incorporato. Chiaro e scuro secondo la preferenza di sistema, e
/// leggibile stampato.
const STYLE: &str = "\
:root{--bg:#fbfbfa;--fg:#1a1a18;--dim:#6b6b66;--line:#e2e2dd;--card:#fff;--accent:#8a3b2f}\
@media(prefers-color-scheme:dark){:root{--bg:#16161a;--fg:#e8e8e3;--dim:#9a9a94;--line:#2c2c32;--card:#1d1d22;--accent:#e07c6b}}\
*{box-sizing:border-box}\
body{margin:0;padding:2rem 1.5rem;background:var(--bg);color:var(--fg);\
font:15px/1.55 ui-sans-serif,-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;max-width:60rem;margin-inline:auto}\
h1{font-size:1.5rem;margin:0;font-weight:600;letter-spacing:-.01em}\
.mark{color:var(--accent);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:.1em}\
h2{font-size:.8rem;text-transform:uppercase;letter-spacing:.09em;color:var(--dim);margin:2.5rem 0 .75rem;font-weight:600}\
.sub{color:var(--dim);margin:.35rem 0 0;font-size:.9rem}\
.quiet{color:var(--dim)}\
code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.86em;word-break:break-all}\
.list{list-style:none;padding:0;margin:0;border-top:1px solid var(--line)}\
.list li{padding:.5rem 0;border-bottom:1px solid var(--line)}\
.tag{font-size:.75rem;color:var(--dim);border:1px solid var(--line);border-radius:99px;padding:.1rem .5rem;margin-left:.4rem}\
.chips{display:flex;flex-wrap:wrap;gap:.5rem;margin-bottom:1rem}\
.chip{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:.35rem .6rem;font-size:.82rem;color:var(--dim)}\
.chip b{color:var(--fg);font-variant-numeric:tabular-nums}\
.scroll{overflow-x:auto;border:1px solid var(--line);border-radius:8px;background:var(--card)}\
table{border-collapse:collapse;width:100%;font-size:.88rem}\
th{text-align:left;font-weight:600;color:var(--dim);font-size:.75rem;text-transform:uppercase;letter-spacing:.05em;padding:.6rem .7rem;border-bottom:1px solid var(--line);white-space:nowrap}\
td{padding:.5rem .7rem;border-bottom:1px solid var(--line);vertical-align:top}\
tbody tr:last-child td{border-bottom:0}\
.bars{display:flex;align-items:flex-end;gap:3px;height:80px;padding:.5rem;background:var(--card);border:1px solid var(--line);border-radius:8px}\
.bar{flex:1;display:flex;align-items:flex-end;height:100%;min-width:4px}\
.bar i{display:block;width:100%;background:var(--accent);opacity:.75;border-radius:2px 2px 0 0}\
footer{margin-top:3rem;padding-top:1rem;border-top:1px solid var(--line);color:var(--dim);font-size:.82rem}\
footer p{margin:.4rem 0}\
";
