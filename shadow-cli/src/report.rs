//! Il report: terminale, JSON, Markdown (§9, Fase 4).
//!
//! # Una regola sola attraversa tutto questo modulo
//!
//! **Niente esce senza passare dal [`Redactor`]** (§P5). Non è una raccomandazione:
//! è il motivo per cui ogni funzione qui riceve un `Redactor` e non un valore
//! già formattato. Un report finisce incollato in un ticket condiviso, ed è
//! esattamente lì che un token lasciato in chiaro smette di essere un dettaglio.

use std::io::Write;

use shadow_core::{
    AuthObservation, Classification, EndpointPattern, Finding, FindingSubject, ObservedInventory,
    Redactor, RunManifest,
};

/// Tutto ciò che un report deve poter mostrare.
pub struct Report<'a> {
    pub inventory: &'a ObservedInventory,
    /// Richieste rimaste fuori dal `path_prefix`, contate e mai nascoste.
    pub out_of_scope: u64,
    pub visible: Vec<&'a Finding>,
    pub silenced: usize,
    pub manifest: &'a RunManifest,
    pub redactor: Redactor,
}

impl Report<'_> {
    /// Rende leggibile il soggetto di un finding, già mascherato.
    fn subject(&self, subject: &FindingSubject) -> String {
        match subject {
            FindingSubject::ObservedEndpoint(id) => self
                .inventory
                .iter()
                .find(|endpoint| &endpoint.id == id)
                .map(|endpoint| self.redactor.text(&endpoint.path_pattern))
                .unwrap_or_else(|| id.as_str().to_string()),
            FindingSubject::DeclaredEndpoint { path_pattern, .. } => {
                self.redactor.text(path_pattern)
            }
        }
    }

    fn subject_id(&self, subject: &FindingSubject) -> String {
        match subject {
            FindingSubject::ObservedEndpoint(id) => id.as_str().to_string(),
            FindingSubject::DeclaredEndpoint { .. } => "-".to_string(),
        }
    }

    /// I finding raggruppati per **la ragione che li accomuna**.
    ///
    /// # Perché raggruppare, e perché non è nascondere
    ///
    /// Il passo umano del collaudo ha misurato che sette finding su dieci
    /// dicono la stessa identica cosa: *«dichiarato ma mai osservato, e la
    /// finestra è troppo corta per dire zombie»*, ripetuto una volta per ogni
    /// endpoint dichiarato non colpito — 305 volte su Gitea, 115 su Vikunja. E
    /// che su un'API basata su nomi il match parziale si ripete su ogni utente
    /// e ogni repository, con la stessa spiegazione parola per parola.
    ///
    /// Il tool non ha concluso 305 cose: ne ha conclusa una, su 305 soggetti.
    ///
    /// Quindi la ragione si scrive **una volta** e sotto si elencano i
    /// soggetti. **Nessun finding sparisce**: ci sono tutti, uno per riga, e nel
    /// JSON — che è il contratto — la lista resta esattamente com'era. Cambia
    /// ciò che l'utente legge, non ciò che il tool ha concluso, ed è per questo
    /// che non fa salire la `RulesetVersion` (§7).
    ///
    /// Il raggruppamento è **strutturale, non testuale**: la prima riga di
    /// evidenza di due finding della stessa famiglia è identica byte per byte,
    /// perché nomina il path *dichiarato*, non quello osservato. Non si analizza
    /// nessuna stringa per indovinare la famiglia.
    fn grouped(&self) -> Vec<FindingGroup<'_>> {
        let mut groups: Vec<FindingGroup<'_>> = Vec::new();
        for finding in self.ordered() {
            let key = finding.evidence.first().cloned().unwrap_or_default();
            match groups
                .iter_mut()
                .find(|g| g.key == key && g.classification == finding.classification)
            {
                Some(group) => group.members.push(finding),
                None => groups.push(FindingGroup {
                    key,
                    classification: finding.classification,
                    members: vec![finding],
                }),
            }
        }
        groups
    }

    fn ordered(&self) -> Vec<&&Finding> {
        let mut ordered: Vec<&&Finding> = self.visible.iter().collect();
        ordered.sort_by_key(|finding| {
            (
                urgency(finding.classification),
                self.subject(&finding.subject),
            )
        });
        ordered
    }
}

/// Chiavi di configurazione il cui valore è un **percorso di filesystem**.
///
/// Vanno mascherate come i percorsi degli input, non come testo generico: un
/// `openapi_spec` che punta a `/home/utente/clienti/bancaXYZ/spec.json` rivela
/// il cliente esattamente quanto ci riesce il percorso del log (§P5).
const PATH_VALUED_KEYS: &[&str] = &["openapi_spec", "allowlist_path", "history_path"];

/// Maschera il valore di una chiave di configurazione, sapendo se è un percorso.
fn config_value(redactor: &Redactor, key: &str, value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if PATH_VALUED_KEYS.contains(&key) {
        redactor.path(std::path::Path::new(value))
    } else {
        redactor.text(value)
    }
}

/// L'ordine in cui un essere umano vuole leggerli.
fn urgency(classification: Classification) -> u8 {
    match classification {
        Classification::Shadow => 0,
        Classification::Zombie => 1,
        Classification::Undetermined => 2,
        Classification::Known => 3,
    }
}

fn lower(value: impl std::fmt::Debug) -> String {
    format!("{value:?}").to_lowercase()
}

/// Come descrivere il verdetto di autenticazione, con la sua base numerica (§6).
fn auth(endpoint: &EndpointPattern) -> String {
    match endpoint.auth_observed {
        AuthObservation::NotObservable => {
            "not observable (this log format does not carry it)".to_string()
        }
        other => format!(
            "{} (based on {} of {} observable)",
            lower(other),
            endpoint.auth_observable_count,
            endpoint.observation_count
        ),
    }
}

/// Quanti finding con la stessa ragione si stampano per esteso prima di
/// passare all'elenco compatto.
///
/// Sotto questa soglia ripetere l'evidenza non costa attenzione a nessuno;
/// sopra, la ripetizione **è** il rumore.
const GROUP_DETAIL_LIMIT: usize = 3;

/// Finding che dicono la stessa cosa su soggetti diversi.
struct FindingGroup<'a> {
    /// La prima riga di evidenza, identica per tutti i membri.
    key: String,
    classification: Classification,
    members: Vec<&'a &'a Finding>,
}

// --- terminale --------------------------------------------------------------

/// Il report leggibile.
pub fn terminal(out: &mut impl Write, report: &Report<'_>) {
    let redactor = report.redactor;
    let _ = writeln!(out, "observed endpoints ({})", report.inventory.len());
    for endpoint in report.inventory.iter() {
        let methods: Vec<&str> = endpoint.methods.iter().map(String::as_str).collect();
        let _ = writeln!(
            out,
            "  {}  {:<7} {:<44} {:>8}  auth: {}",
            endpoint.id.as_str(),
            methods.join(","),
            redactor.text(&endpoint.path_pattern),
            endpoint.observation_count,
            auth(endpoint)
        );
    }

    if report.out_of_scope > 0 {
        let _ = writeln!(
            out,
            "  ({} request(s) outside the analysed scope; they were counted, not examined)",
            report.out_of_scope
        );
    }

    let _ = writeln!(out, "\nfindings ({})", report.visible.len());
    for group in report.grouped() {
        if group.members.len() <= GROUP_DETAIL_LIMIT {
            for finding in &group.members {
                let _ = writeln!(
                    out,
                    "  {:<13} {:<8} severity {:<6} {} {}",
                    finding.classification.as_str(),
                    lower(finding.confidence),
                    lower(finding.severity),
                    report.subject_id(&finding.subject),
                    report.subject(&finding.subject)
                );
                for reason in &finding.evidence {
                    let _ = writeln!(out, "      because {}", redactor.text(reason));
                }
            }
            continue;
        }

        // La ragione una volta, i soggetti tutti. Nessuno sparisce: cambia
        // quante volte si legge la stessa frase.
        let _ = writeln!(
            out,
            "  {:<13} {} endpoint(s), all for the same reason",
            group.classification.as_str(),
            group.members.len()
        );
        let _ = writeln!(out, "      because {}", redactor.text(&group.key));
        for finding in &group.members {
            let _ = writeln!(
                out,
                "        {} {}",
                report.subject_id(&finding.subject),
                report.subject(&finding.subject)
            );
        }
    }
    if report.silenced > 0 {
        let _ = writeln!(
            out,
            "  ({} finding(s) silenced by the allowlist; they are still counted in the manifest)",
            report.silenced
        );
    }

    manifest_terminal(out, report);
}

fn manifest_terminal(out: &mut impl Write, report: &Report<'_>) {
    let manifest = report.manifest;
    let counts = &manifest.counts;
    let redactor = report.redactor;

    let _ = writeln!(out, "\nrun manifest");
    let _ = writeln!(out, "  shadow version    {}", manifest.shadow_version);
    let _ = writeln!(out, "  ruleset version   {}", manifest.ruleset_version);
    let _ = writeln!(
        out,
        "  run timestamp     {}",
        manifest.run_timestamp.to_rfc3339()
    );
    if !redactor.is_enabled() {
        let _ = writeln!(out, "  redaction         OFF (--show-raw-values)");
    }
    for input in &manifest.inputs {
        let _ = writeln!(out, "  input             {}", redactor.path(&input.path));
        let _ = writeln!(
            out,
            "    {:<14}  {}",
            input.algorithm.as_str(),
            input.digest
        );
    }
    let _ = writeln!(out, "  lines total       {}", counts.total_lines);
    let _ = writeln!(out, "  lines parsed      {}", counts.parsed_lines);
    let _ = writeln!(out, "  lines discarded   {}", counts.discarded_lines);
    for (reason, count) in &counts.discarded_by_reason {
        let _ = writeln!(out, "    {reason:<14}  {count}");
    }
    let _ = writeln!(out, "  endpoints found   {}", counts.endpoints_found);
    for (classification, count) in &counts.findings_by_classification {
        let _ = writeln!(out, "    {:<14}  {}", classification.as_str(), count);
    }
    let _ = writeln!(out, "  configuration");
    for (key, setting) in &manifest.configuration.settings {
        let _ = writeln!(
            out,
            "    {:<22} {:<24} from {}",
            key,
            config_value(&redactor, key, &setting.value),
            setting.source.as_str()
        );
    }
}

// --- JSON -------------------------------------------------------------------

/// Il report in JSON: **unico contenuto di stdout**, così che
/// `shadow ... > report.json` dia un file pulito (§7).
///
/// Lo schema è costruito a mano invece che derivato dai tipi: §3 lascia aperto
/// il formato esatto del report esportabile, e legare lo schema pubblico alla
/// forma interna dei record significherebbe che una rifattorizzazione del
/// modello cambia un contratto verso l'esterno.
pub fn json(out: &mut impl Write, report: &Report<'_>) {
    let redactor = report.redactor;

    let endpoints: Vec<serde_json::Value> = report
        .inventory
        .iter()
        .map(|endpoint| {
            serde_json::json!({
                "id": endpoint.id.as_str(),
                "path_pattern": redactor.text(&endpoint.path_pattern),
                "methods": endpoint.methods.iter().collect::<Vec<&String>>(),
                "observation_count": endpoint.observation_count,
                "first_seen": endpoint.first_seen.to_rfc3339(),
                "last_seen": endpoint.last_seen.to_rfc3339(),
                "auth": {
                    "observed": lower(endpoint.auth_observed),
                    "observable_requests": endpoint.auth_observable_count,
                },
            })
        })
        .collect();

    let findings: Vec<serde_json::Value> = report
        .ordered()
        .iter()
        .map(|finding| {
            serde_json::json!({
                "classification": finding.classification.as_str(),
                "confidence": lower(finding.confidence),
                "severity": lower(finding.severity),
                "is_ambiguous": finding.is_ambiguous,
                "endpoint_id": report.subject_id(&finding.subject),
                "path_pattern": report.subject(&finding.subject),
                "evidence": finding
                    .evidence
                    .iter()
                    .map(|e| redactor.text(e))
                    .collect::<Vec<String>>(),
            })
        })
        .collect();

    let document = serde_json::json!({
        "schema": "shadow-report/1",
        "redaction_enabled": redactor.is_enabled(),
        "endpoints": endpoints,
        "findings": findings,
        "findings_silenced_by_allowlist": report.silenced,
        "requests_outside_scope": report.out_of_scope,
        "manifest": manifest_json(report),
    });

    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_string())
    );
}

/// Il manifest in JSON, **già redatto**.
///
/// È `pub(crate)` perché è la stessa forma che finisce nello storico: se lo
/// storico se la costruisse per conto suo, prima o poi persisterebbe un campo
/// che il report non mostra, o lo persisterebbe non redatto (§P5).
pub(crate) fn manifest_json(report: &Report<'_>) -> serde_json::Value {
    let manifest = report.manifest;
    let counts = &manifest.counts;
    let redactor = report.redactor;

    serde_json::json!({
        "shadow_version": manifest.shadow_version,
        "ruleset_version": manifest.ruleset_version.as_str(),
        "run_timestamp": manifest.run_timestamp.to_rfc3339(),
        "inputs": manifest.inputs.iter().map(|input| serde_json::json!({
            "path": redactor.path(&input.path),
            "role": lower(input.role),
            "algorithm": input.algorithm.as_str(),
            "digest": input.digest,
        })).collect::<Vec<serde_json::Value>>(),
        "counts": {
            "lines_total": counts.total_lines,
            "lines_parsed": counts.parsed_lines,
            "lines_discarded": counts.discarded_lines,
            "discarded_by_reason": counts.discarded_by_reason,
            "endpoints_found": counts.endpoints_found,
            "findings_by_classification": counts.findings_by_classification.iter()
                .map(|(k, v)| (k.as_str().to_string(), *v))
                .collect::<std::collections::BTreeMap<String, u64>>(),
        },
        "configuration": manifest.configuration.settings.iter().map(|(key, setting)| {
            (key.clone(), serde_json::json!({
                "value": config_value(&redactor, key, &setting.value),
                "source": setting.source.as_str(),
            }))
        }).collect::<std::collections::BTreeMap<String, serde_json::Value>>(),
    })
}

// --- Markdown ---------------------------------------------------------------

/// Il report in Markdown, per finire in un ticket o in una pagina di wiki —
/// che è esattamente il posto in cui la redazione conta di più.
pub fn markdown(out: &mut impl Write, report: &Report<'_>) {
    let redactor = report.redactor;
    let manifest = report.manifest;

    let _ = writeln!(out, "# Shadow report\n");
    if !redactor.is_enabled() {
        let _ = writeln!(
            out,
            "> **Redaction is off** (`--show-raw-values`): this document may contain secrets.\n"
        );
    }
    let _ = writeln!(
        out,
        "Produced by shadow {} with ruleset {} at {}.\n",
        manifest.shadow_version,
        manifest.ruleset_version,
        manifest.run_timestamp.to_rfc3339()
    );

    let _ = writeln!(out, "## Findings\n");
    if report.visible.is_empty() {
        let _ = writeln!(out, "Nothing to report.\n");
    } else {
        let _ = writeln!(out, "| Classification | Confidence | Severity | Endpoint | Why |");
        let _ = writeln!(out, "|---|---|---|---|---|");
        for finding in report.ordered() {
            let _ = writeln!(
                out,
                "| {} | {} | {} | `{}` | {} |",
                finding.classification.as_str(),
                lower(finding.confidence),
                lower(finding.severity),
                table_cell(&report.subject(&finding.subject)),
                table_cell(
                    &finding
                        .evidence
                        .iter()
                        .map(|e| redactor.text(e))
                        .collect::<Vec<String>>()
                        .join("; ")
                )
            );
        }
        let _ = writeln!(out);
    }
    if report.silenced > 0 {
        let _ = writeln!(
            out,
            "{} finding(s) were silenced by the allowlist and are still counted in the manifest.\n",
            report.silenced
        );
    }

    let _ = writeln!(out, "## Observed endpoints\n");
    let _ = writeln!(out, "| Endpoint | Methods | Requests | Auth |");
    let _ = writeln!(out, "|---|---|---|---|");
    for endpoint in report.inventory.iter() {
        let methods: Vec<&str> = endpoint.methods.iter().map(String::as_str).collect();
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            table_cell(&redactor.text(&endpoint.path_pattern)),
            table_cell(&methods.join(", ")),
            endpoint.observation_count,
            auth(endpoint)
        );
    }

    let _ = writeln!(out, "\n## Run manifest\n");
    for input in &manifest.inputs {
        let _ = writeln!(
            out,
            "- input `{}` ({} `{}`)",
            redactor.path(&input.path),
            input.algorithm.as_str(),
            input.digest
        );
    }
    let counts = &manifest.counts;
    let _ = writeln!(
        out,
        "- {} line(s) read, {} parsed, {} discarded",
        counts.total_lines, counts.parsed_lines, counts.discarded_lines
    );
    let _ = writeln!(out, "- {} endpoint(s) found", counts.endpoints_found);
    let _ = writeln!(out, "\n### Configuration\n");
    let _ = writeln!(out, "| Key | Value | From |");
    let _ = writeln!(out, "|---|---|---|");
    for (key, setting) in &manifest.configuration.settings {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} |",
            key,
            table_cell(&config_value(&redactor, key, &setting.value)),
            setting.source.as_str()
        );
    }
}

/// Protegge una barra verticale dentro una cella di tabella Markdown.
///
/// Si applica a **ogni** cella che porta testo non deciso da noi: il pattern di
/// un endpoint e i metodi vengono dai dati analizzati, l'evidenza li cita, e il
/// valore di `log_format` è una grammatica che scrive l'utente. Una barra in
/// uno qualunque di questi spezza la riga in due colonne, e chi legge il report
/// non vede che manca un pezzo — vede una tabella storta e si fida lo stesso.
/// Il Markdown non è un contratto (§7), ma un report illeggibile è un report
/// che non serve.
fn table_cell(value: &str) -> String {
    value.replace('|', "\\|")
}
