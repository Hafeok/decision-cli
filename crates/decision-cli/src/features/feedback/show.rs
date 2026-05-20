//! `dec feedback show <iri>` — full feedback record dump (FT-033).
//!
//! Read-only command that resolves a feedback artifact by IRI and prints
//! every populated predicate as a key-value table (or as a JSON object
//! under `--format json`). Errors map to exit code 1 — the CLI adapter
//! translates [`ShowError::NotFound`] specifically into the structured
//! "artifact not found" message FT-033 §Error handling names.

use std::path::Path;

use anyhow::{anyhow, Result};
use oxigraph::model::NamedNode;

use crate::core::feedback::{get, Feedback, FeedbackReadError};

use super::format::{json_escape, json_opt};
use super::store_io::open_readonly_store;

/// Structured error surface for `dec feedback show`.
#[derive(Debug, thiserror::Error)]
pub enum ShowError {
    /// IRI doesn't parse as a NamedNode.
    #[error("invalid feedback IRI: {0}")]
    InvalidIri(String),
    /// No feedback artifact exists at the given IRI in the active stream.
    #[error("feedback not found: <{0}>")]
    NotFound(String),
    /// Store-level failure surfaced from `core::feedback::read`.
    #[error("reading feedback record: {0}")]
    Read(String),
    /// Underlying store / scope load failure.
    #[error("{0}")]
    Open(String),
}

/// Resolve a feedback artifact by IRI. Returns the [`Feedback`] record
/// or a structured [`ShowError`] suitable for CLI rendering.
pub fn show(workdir: &Path, iri: &str) -> Result<Feedback, ShowError> {
    let store = open_readonly_store(workdir).map_err(|e| ShowError::Open(format!("{e:#}")))?;
    let node = NamedNode::new(iri).map_err(|_| ShowError::InvalidIri(iri.to_string()))?;
    match get(&store, &node) {
        Ok(fb) => Ok(fb),
        Err(FeedbackReadError::NotFound { iri }) => Err(ShowError::NotFound(iri)),
        Err(e) => Err(ShowError::Read(format!("{e}"))),
    }
}

/// Render a feedback record as a human-readable key-value table.
#[must_use]
pub fn format_show(fb: &Feedback) -> String {
    let mut out = String::new();
    out.push_str(&format!("iri:                  {}\n", fb.iri.as_str()));
    out.push_str(&format!("class:                {}\n", fb.class));
    out.push_str(&format!("lifecycle_state:      {}\n", fb.lifecycle_state));
    out.push_str(&format!("severity:             {}\n", fb.severity.as_str()));
    out.push_str(&format!("target_role:          {}\n", fb.target_role));
    out.push_str(&format!("in_stream:            {}\n", fb.in_stream.as_str()));
    out.push_str(&format!(
        "source_session:       {}\n",
        fb.source_session.as_str()
    ));
    push_optional(&mut out, "source_artifact", opt_iri(&fb.source_artifact));
    push_optional(&mut out, "addressing_artifact", opt_iri(&fb.addressing_artifact));
    push_optional(&mut out, "closed_by", opt_iri(&fb.closed_by));
    push_optional(&mut out, "superseded_by", opt_iri(&fb.superseded_by));
    push_optional(&mut out, "receiving_session", opt_iri(&fb.receiving_session));
    push_optional(&mut out, "routed_at", fb.routed_at.clone());
    push_optional(&mut out, "rejection_reason", fb.rejection_reason.clone());
    push_optional(&mut out, "recommendation", fb.recommendation.clone());
    push_optional(&mut out, "disposition_override", fb.disposition_override.clone());
    push_optional(
        &mut out,
        "disposition_rationale",
        fb.disposition_rationale.clone(),
    );
    out.push_str("evidence:\n");
    out.push_str(&indent_block(&fb.evidence, "  "));
    out.push('\n');
    out
}

/// Render the feedback as a JSON object suitable for `--format json`.
#[must_use]
pub fn format_show_json(fb: &Feedback) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"iri\": \"{}\",\n", json_escape(fb.iri.as_str())));
    out.push_str(&format!("  \"class\": \"{}\",\n", json_escape(&fb.class)));
    out.push_str(&format!(
        "  \"lifecycle_state\": \"{}\",\n",
        json_escape(&fb.lifecycle_state)
    ));
    out.push_str(&format!(
        "  \"severity\": \"{}\",\n",
        json_escape(fb.severity.as_str())
    ));
    out.push_str(&format!(
        "  \"target_role\": \"{}\",\n",
        json_escape(&fb.target_role)
    ));
    out.push_str(&format!(
        "  \"in_stream\": \"{}\",\n",
        json_escape(fb.in_stream.as_str())
    ));
    out.push_str(&format!(
        "  \"source_session\": \"{}\",\n",
        json_escape(fb.source_session.as_str())
    ));
    out.push_str(&format!(
        "  \"source_artifact\": {},\n",
        json_opt(fb.source_artifact.as_ref().map(NamedNode::as_str))
    ));
    out.push_str(&format!(
        "  \"addressing_artifact\": {},\n",
        json_opt(fb.addressing_artifact.as_ref().map(NamedNode::as_str))
    ));
    out.push_str(&format!(
        "  \"closed_by\": {},\n",
        json_opt(fb.closed_by.as_ref().map(NamedNode::as_str))
    ));
    out.push_str(&format!(
        "  \"superseded_by\": {},\n",
        json_opt(fb.superseded_by.as_ref().map(NamedNode::as_str))
    ));
    out.push_str(&format!(
        "  \"receiving_session\": {},\n",
        json_opt(fb.receiving_session.as_ref().map(NamedNode::as_str))
    ));
    out.push_str(&format!(
        "  \"routed_at\": {},\n",
        json_opt(fb.routed_at.as_deref())
    ));
    out.push_str(&format!(
        "  \"rejection_reason\": {},\n",
        json_opt(fb.rejection_reason.as_deref())
    ));
    out.push_str(&format!(
        "  \"recommendation\": {},\n",
        json_opt(fb.recommendation.as_deref())
    ));
    out.push_str(&format!(
        "  \"disposition_override\": {},\n",
        json_opt(fb.disposition_override.as_deref())
    ));
    out.push_str(&format!(
        "  \"disposition_rationale\": {},\n",
        json_opt(fb.disposition_rationale.as_deref())
    ));
    out.push_str(&format!(
        "  \"evidence\": \"{}\"\n",
        json_escape(&fb.evidence)
    ));
    out.push_str("}\n");
    out
}

/// Render an error payload as JSON for the `--format json` error path
/// (FT-033 §Error handling: "errors are still printed as JSON on stderr").
#[must_use]
pub fn format_error_json(message: &str) -> String {
    format!("{{\"error\": \"{}\"}}\n", json_escape(message))
}

fn push_optional(out: &mut String, label: &str, value: Option<String>) {
    if let Some(v) = value {
        out.push_str(&format!("{label}: {v}\n", v = v));
    }
}

fn opt_iri(n: &Option<NamedNode>) -> Option<String> {
    n.as_ref().map(|n| n.as_str().to_string())
}

fn indent_block(s: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        out.push_str(prefix);
        out.push_str(line);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str(prefix);
    } else {
        // Strip the trailing newline added by the loop so callers can
        // append their own without double-spacing.
        out.pop();
    }
    out
}

/// Convenience wrapper used by the CLI adapter to translate a
/// [`ShowError`] into an `anyhow::Result` for uniform reporting.
pub fn show_anyhow(workdir: &Path, iri: &str) -> Result<Feedback> {
    show(workdir, iri).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::feedback::Severity;
    use oxigraph::model::NamedNode;

    fn sample() -> Feedback {
        Feedback {
            iri: NamedNode::new("urn:f:show:1").expect("iri"),
            class: "gap".to_string(),
            severity: Severity::Warning,
            target_role: "spec-author".to_string(),
            evidence: "spec line 42 underspecifies the gap".to_string(),
            recommendation: Some("amend spec".to_string()),
            lifecycle_state: "produced".to_string(),
            source_session: NamedNode::new("urn:s:1").expect("session"),
            source_artifact: None,
            addressing_artifact: None,
            closed_by: None,
            rejection_reason: None,
            superseded_by: None,
            routed_at: None,
            receiving_session: None,
            disposition_override: None,
            disposition_rationale: None,
            in_stream: NamedNode::new("urn:stream:1").expect("stream"),
        }
    }

    #[test]
    fn format_show_includes_required_and_optional_fields() {
        let out = format_show(&sample());
        assert!(out.contains("iri:                  urn:f:show:1"));
        assert!(out.contains("class:                gap"));
        assert!(out.contains("lifecycle_state:      produced"));
        assert!(out.contains("recommendation: amend spec"));
        assert!(out.contains("evidence:"));
        assert!(out.contains("  spec line 42"));
    }

    #[test]
    fn format_show_json_emits_required_keys() {
        let json = format_show_json(&sample());
        assert!(json.contains("\"iri\": \"urn:f:show:1\""));
        assert!(json.contains("\"class\": \"gap\""));
        assert!(json.contains("\"lifecycle_state\": \"produced\""));
        assert!(json.contains("\"addressing_artifact\": null"));
        assert!(json.contains("\"recommendation\": \"amend spec\""));
    }

    #[test]
    fn format_error_json_is_one_line() {
        let s = format_error_json("not found");
        assert!(s.starts_with("{\"error\":"));
        assert!(s.ends_with('\n'));
    }
}
