//! La vista in terminale e in JSON.
//!
//! Il JSON non è un contratto pubblico come `shadow-report/1`: è il modo in cui
//! l'applicazione macOS parla con il binario, e vive con lui. Se un giorno
//! qualcuno ci costruisce sopra un'automazione, allora diventerà un contratto e
//! prenderà un identificatore, come è successo al report.

use std::io::Write;

use crate::Status;

/// La vista in terminale: `shadow status`.
pub fn terminal(out: &mut impl Write, status: &Status) {
    let _ = writeln!(out, "target            {}", status.target);
    match &status.last_run {
        Some(run) => {
            let _ = writeln!(
                out,
                "last run          {} at {}",
                run.run_id, run.timestamp
            );
            let _ = writeln!(
                out,
                "                  {} endpoint(s), {} line(s) analysed",
                run.endpoints, run.lines
            );
        }
        None => {
            let _ = writeln!(out, "last run          none: nothing has been analysed yet");
        }
    }

    // Prima cosa dopo l'intestazione, perché è la sola domanda per cui si apre
    // un cruscotto.
    let _ = writeln!(out, "\nwhat changed      {} new in the last run", status.new_in_last_run.len());
    for endpoint in &status.new_in_last_run {
        let _ = writeln!(
            out,
            "  new  {}  {}  ({})",
            endpoint.id,
            endpoint.redacted_pattern,
            endpoint.last_classification.as_deref().unwrap_or("no verdict")
        );
    }

    let _ = writeln!(out, "\nopen alerts       {}", status.open_alerts.len());
    for alert in &status.open_alerts {
        let _ = writeln!(
            out,
            "  {}  {}  first seen in run {}",
            alert.id, alert.redacted_pattern, alert.first_seen_run
        );
    }

    let _ = writeln!(out, "\ninventory         {} endpoint(s)", status.inventory.len());
    for (name, count) in status.by_classification() {
        let _ = writeln!(out, "  {name:<14} {count}");
    }

    if status.trend.len() > 1 {
        let _ = writeln!(out, "\ntrend             last {} run(s)", status.trend.len());
        for point in &status.trend {
            let _ = writeln!(
                out,
                "  run {:<6} {:>6} endpoint(s)  {:>9} line(s)",
                point.run_id, point.endpoints, point.lines
            );
        }
    }

    let _ = writeln!(
        out,
        "\nprovenance        shadow {} · ruleset {}",
        status.shadow_version, status.ruleset_version
    );
    let _ = writeln!(
        out,
        "                  Shadow is a discovery aid, not a security certification."
    );
}

/// La vista in JSON, per l'applicazione macOS e per chi vuole leggerla a
/// macchina.
pub fn json(out: &mut impl Write, status: &Status) {
    let document = serde_json::json!({
        "target": status.target,
        "last_run": status.last_run.as_ref().map(|run| serde_json::json!({
            "id": run.run_id,
            "timestamp": run.timestamp,
            "endpoints": run.endpoints,
            "lines": run.lines,
        })),
        "shadow_version": status.shadow_version,
        "ruleset_version": status.ruleset_version,
        "new_in_last_run": status.new_in_last_run.iter().map(|e| serde_json::json!({
            "id": e.id,
            "path_pattern": e.redacted_pattern,
            "classification": e.last_classification,
        })).collect::<Vec<_>>(),
        "open_alerts": status.open_alerts.iter().map(|e| serde_json::json!({
            "id": e.id,
            "path_pattern": e.redacted_pattern,
            "first_seen_run": e.first_seen_run,
        })).collect::<Vec<_>>(),
        "inventory_size": status.inventory.len(),
        "by_classification": status.by_classification()
            .into_iter()
            .collect::<std::collections::BTreeMap<&str, usize>>(),
        "trend": status.trend.iter().map(|p| serde_json::json!({
            "run_id": p.run_id,
            "timestamp": p.timestamp,
            "endpoints": p.endpoints,
            "lines": p.lines,
        })).collect::<Vec<_>>(),
        "disclaimer": "Shadow is a discovery aid, not a security certification.",
    });
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_string())
    );
}
