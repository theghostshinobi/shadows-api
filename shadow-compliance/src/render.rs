//! I tre formati del report d'audit.
//!
//! Tre e non uno perché servono a tre persone diverse: il **CSV** è ciò che un
//! auditor apre davvero — si filtra, si ordina, si allega a un workpaper; il
//! **Markdown** è la versione che si legge, con le premesse e i limiti in
//! testa; il **JSON** è per chi ci costruisce sopra un'automazione, ed è l'unico
//! dei tre a essere un **contratto versionato**.
//!
//! Le tre viste partono dagli stessi dati e dalle stesse etichette: se
//! divergessero, il documento che si consegna direbbe una cosa diversa da
//! quello che si automatizza.

use std::io::Write;

use crate::{auth_label, classification_label, ComplianceReport, DISCLAIMER};

/// Identificatore dello schema JSON del report d'audit.
///
/// È un contratto pubblico versionato allo stesso titolo di `shadow-report/1`
/// (§7): aggiungere campi è consentito, rinominarli o rimuoverli richiede
/// `shadow-compliance/2`. È un identificatore **distinto**, perché è un
/// documento diverso con un pubblico diverso.
pub const SCHEMA: &str = "shadow-compliance/1";

/// Il report in CSV.
///
/// Una riga per endpoint, e le colonne dell'autenticazione sono **due**: lo
/// stato e la sua base di calcolo. Separarle è ciò che impedisce a un foglio di
/// calcolo di trasformare quattro valori in una casella spuntata o no.
pub fn csv(out: &mut impl Write, report: &ComplianceReport<'_>) {
    let _ = writeln!(
        out,
        "endpoint_id,path_pattern,methods,observations,classification,authentication,observable_requests,first_seen_run,last_seen_run,pending_alert"
    );
    for row in report.inventory {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{}",
            field(&row.id),
            field(&row.redacted_pattern),
            field(&row.methods.join(" ")),
            row.observation_count,
            field(classification_label(row)),
            field(auth_label(row)),
            row.auth_observable_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "not recorded".to_string()),
            row.first_seen_run,
            row.last_seen_run,
            if row.pending_alert { "yes" } else { "no" },
        );
    }
}

/// Protegge un campo CSV: virgolette raddoppiate e campo quotato quando serve.
///
/// Un pattern di endpoint può contenere una virgola — è un carattere legale in
/// un path — e senza questa funzione spezzerebbe la riga in due colonne dentro
/// il foglio di calcolo di un auditor, che non se ne accorgerebbe.
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Il report in Markdown: la versione che si legge.
pub fn markdown(out: &mut impl Write, report: &ComplianceReport<'_>) {
    let _ = writeln!(out, "# Endpoint inventory — {}\n", report.target);
    let _ = writeln!(
        out,
        "Produced by shadow {} with ruleset {}.",
        report.shadow_version, report.ruleset_version
    );
    match (report.last_run_id, report.last_run_timestamp) {
        (Some(id), Some(when)) => {
            let _ = writeln!(out, "Latest analysis: run {id} at {when}.\n");
        }
        _ => {
            let _ = writeln!(out, "No analysis has been recorded for this target yet.\n");
        }
    }

    let _ = writeln!(out, "## What this document is, and is not\n");
    for line in DISCLAIMER {
        let _ = writeln!(out, "- {line}");
    }

    let _ = writeln!(out, "\n## Summary\n");
    let _ = writeln!(out, "| Classification | Endpoints |");
    let _ = writeln!(out, "|---|---|");
    for (name, count) in report.classification_summary() {
        let _ = writeln!(out, "| {name} | {count} |");
    }
    let _ = writeln!(out, "\n| Authentication | Endpoints |");
    let _ = writeln!(out, "|---|---|");
    for (name, count) in report.auth_summary() {
        let _ = writeln!(out, "| {name} | {count} |");
    }

    let _ = writeln!(out, "\n## Inventory\n");
    let _ = writeln!(
        out,
        "| Endpoint | Methods | Requests | Classification | Authentication | Observable requests | Alert |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|");
    for row in report.inventory {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            cell(&row.redacted_pattern),
            cell(&row.methods.join(", ")),
            row.observation_count,
            classification_label(row),
            auth_label(row),
            row.auth_observable_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "not recorded".to_string()),
            if row.pending_alert { "open" } else { "—" },
        );
    }
    if report.inventory.is_empty() {
        let _ = writeln!(out, "\nNo endpoints have been observed for this target yet.");
    }
}

/// Protegge una barra verticale dentro una cella di tabella Markdown: un
/// pattern viene dai dati analizzati, e una barra spezzerebbe la riga.
fn cell(value: &str) -> String {
    value.replace('|', "\\|")
}

/// Il report in JSON: contratto pubblico versionato [`SCHEMA`].
pub fn json(out: &mut impl Write, report: &ComplianceReport<'_>) {
    let endpoints: Vec<serde_json::Value> = report
        .inventory
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "path_pattern": row.redacted_pattern,
                "methods": row.methods,
                "observations": row.observation_count,
                "classification": classification_label(row),
                "authentication": {
                    // Lo stato e la sua base restano **due campi distinti**: un
                    // consumatore che volesse un booleano deve almeno doverlo
                    // fabbricare lui, e vedere che sta buttando via qualcosa.
                    "observed": auth_label(row),
                    "observable_requests": row.auth_observable_count,
                },
                "first_seen_run": row.first_seen_run,
                "last_seen_run": row.last_seen_run,
                "pending_alert": row.pending_alert,
            })
        })
        .collect();

    let document = serde_json::json!({
        "schema": SCHEMA,
        "target": report.target,
        "shadow_version": report.shadow_version,
        "ruleset_version": report.ruleset_version,
        "latest_run": report.last_run_id.map(|id| serde_json::json!({
            "id": id,
            "timestamp": report.last_run_timestamp,
        })),
        "disclaimer": DISCLAIMER,
        "authentication_states": crate::AUTH_STATES,
        "summary": {
            "by_classification": report.classification_summary()
                .into_iter()
                .collect::<std::collections::BTreeMap<&str, usize>>(),
            "by_authentication": report.auth_summary()
                .into_iter()
                .collect::<std::collections::BTreeMap<&str, usize>>(),
        },
        "endpoints": endpoints,
    });

    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_string())
    );
}
