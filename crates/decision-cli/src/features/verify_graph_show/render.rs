//! CLI rendering helpers for `dec verify graph show` (FT-043).
//!
//! Two output formats:
//!   * `text` — multi-line human render with the header (id, verifies,
//!     environment + safety) and a numbered step list whose summary line
//!     names the kind and the kind's key field.
//!   * `json` — the [`GraphDocument`] as serde JSON. Optional fields are
//!     omitted (not `null`) so the parity test can byte-compare CLI and
//!     MCP outputs, and the round-trip back to Turtle stays stable.

use super::document::StepDocument;
use super::GraphShowResponse;

/// Maximum number of characters of a step's key field shown in the
/// summary line. Longer values are truncated with a trailing ellipsis.
const SUMMARY_VALUE_BUDGET: usize = 60;

/// Render the response as a multi-line text block — header, step list,
/// path footer.
#[must_use]
pub fn render_text(resp: &GraphShowResponse) -> String {
    let g = &resp.graph;
    let mut out = String::new();
    out.push_str(&format!("{}\n", g.id));
    out.push_str(&format!("Verifies:    {}\n", g.verifies));
    let env_line = match resp.environment_safety.as_deref() {
        Some(safety) => format!(
            "Environment: {env} (safety: {safety})\n",
            env = g.environment,
            safety = safety
        ),
        None => format!("Environment: {}\n", g.environment),
    };
    out.push_str(&env_line);
    out.push_str("Steps:\n");
    if g.steps.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for (idx, step) in g.steps.iter().enumerate() {
            let position = idx + 1;
            let summary = summarise_step(step);
            out.push_str(&format!(
                "  {position}. {kind:<18} {summary}\n",
                kind = step.kind_str(),
                summary = summary,
            ));
        }
    }
    out.push_str(&format!("Path: {}\n", resp.path.display()));
    out
}

/// One-line summary of the step's key field. Mirrors FT-043 §Outputs
/// AC #3 — names the predicate (e.g. `command=...`) and truncates long
/// values so a step row stays on one terminal line.
fn summarise_step(step: &StepDocument) -> String {
    match step {
        StepDocument::ShellCommand { command, .. } => format_kv("command", command),
        StepDocument::SparqlAssertion { query, .. } => format_kv("query", query),
        StepDocument::FileAssertion { path, .. } => format_kv("path", path),
        StepDocument::HttpRequest { method, url, .. } => {
            format!("{} {}", method, truncate_value(url))
        }
        StepDocument::WaitFor { timeout, .. } => format_kv("timeout", timeout),
        StepDocument::Capture { bind_as, .. } => format_kv("bindAs", bind_as),
    }
}

fn format_kv(key: &str, value: &str) -> String {
    let truncated = truncate_value(value);
    format!("{key}={truncated:?}")
}

/// Truncate `value` to [`SUMMARY_VALUE_BUDGET`] chars, appending an
/// ellipsis when truncation actually happens.
fn truncate_value(value: &str) -> String {
    if value.chars().count() <= SUMMARY_VALUE_BUDGET {
        return value.to_string();
    }
    let mut out: String = value.chars().take(SUMMARY_VALUE_BUDGET).collect();
    out.push('…');
    out
}

/// Render the graph document as JSON. Optional fields are omitted when
/// absent — matching the MCP envelope's `graph` field byte-for-byte.
#[must_use]
pub fn render_json(resp: &GraphShowResponse) -> String {
    serde_json::to_string_pretty(&resp.graph).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::verify_graph_show::document::GraphDocument;
    use serde_json::Value;
    use std::path::PathBuf;

    fn sample_response() -> GraphShowResponse {
        GraphShowResponse {
            graph: GraphDocument {
                id: "VG-001".to_string(),
                verifies: "FT-001".to_string(),
                environment: "BNCH-001-ephemeral-cli".to_string(),
                steps: vec![
                    StepDocument::ShellCommand {
                        command: "echo hi".to_string(),
                        expect_exit_code: Some(0),
                        capture_output: None,
                        provides_evidence_for: Vec::new(),
                    },
                    StepDocument::SparqlAssertion {
                        target: ".dec/store".to_string(),
                        query: "SELECT ?s WHERE { ?s ?p ?o }".to_string(),
                        expect_rows: Some(1),
                        provides_evidence_for: Vec::new(),
                    },
                ],
            },
            path: PathBuf::from("/tmp/.dec/verify/graph/VG-001.ttl"),
            environment_safety: Some("isolated".to_string()),
        }
    }

    #[test]
    fn text_renders_header_and_step_lines() {
        let s = render_text(&sample_response());
        assert!(s.contains("VG-001"));
        assert!(s.contains("Verifies:"));
        assert!(s.contains("FT-001"));
        assert!(s.contains("Environment:"));
        assert!(s.contains("BNCH-001-ephemeral-cli"));
        assert!(s.contains("safety: isolated"));
        assert!(s.contains("Steps:"));
        assert!(s.contains("  1. shell-command"));
        assert!(s.contains("  2. sparql-assertion"));
        assert!(s.contains("Path:"));
    }

    #[test]
    fn text_includes_command_summary() {
        let s = render_text(&sample_response());
        assert!(s.contains("command="), "expected command kv: {s}");
    }

    #[test]
    fn text_omits_safety_when_absent() {
        let mut r = sample_response();
        r.environment_safety = None;
        let s = render_text(&r);
        assert!(s.contains("Environment: BNCH-001-ephemeral-cli"));
        assert!(!s.contains("safety:"), "safety must be omitted: {s}");
    }

    #[test]
    fn text_empty_steps_renders_placeholder() {
        let mut r = sample_response();
        r.graph.steps.clear();
        let s = render_text(&r);
        assert!(s.contains("Steps:"));
        assert!(s.contains("(none)"));
    }

    #[test]
    fn json_emits_graph_document_only() {
        let s = render_json(&sample_response());
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_object());
        assert_eq!(v["id"], "VG-001");
        assert_eq!(v["verifies"], "FT-001");
        assert_eq!(v["environment"], "BNCH-001-ephemeral-cli");
        assert!(v["steps"].is_array());
        // path / safety are NOT part of the graph document JSON.
        assert!(v.get("path").is_none());
        assert!(v.get("environment_safety").is_none());
    }

    #[test]
    fn truncate_value_keeps_short_strings() {
        assert_eq!(truncate_value("abc"), "abc");
    }

    #[test]
    fn truncate_value_clips_long_strings() {
        let s = "x".repeat(SUMMARY_VALUE_BUDGET + 10);
        let out = truncate_value(&s);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() == SUMMARY_VALUE_BUDGET + 1);
    }

    #[test]
    fn render_text_preserves_dollar_placeholder() {
        let mut r = sample_response();
        r.graph.steps = vec![StepDocument::ShellCommand {
            command: "echo ${earlier_capture}".to_string(),
            expect_exit_code: None,
            capture_output: None,
            provides_evidence_for: Vec::new(),
        }];
        let s = render_text(&r);
        assert!(
            s.contains("${earlier_capture}"),
            "placeholder must render verbatim: {s}"
        );
    }
}
