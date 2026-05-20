//! `dec feedback list` — open feedback inspector (FT-029 / FT-033).
//!
//! Phase A scope: list every open `dec:Feedback` artifact scoped to the
//! active stream, optionally filtered by state / class / target role,
//! grouped by `(class, target_role)` in the text renderer and emitted as
//! a JSON array under the `--format json` path.
//!
//! The TC-039 contract (`scripts/checks/dec-feedback-list.sh`) requires
//! the public surface to keep exposing `list` and `format_list` at the
//! module's *parent* path; the parent `mod.rs` re-exports both.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::core::feedback::{list_open, Feedback};

use super::format::{json_escape, json_opt, truncate_evidence};
use super::store_io::{active_stream_iri, open_readonly_store};

/// One row of `dec feedback list` — used by both the human renderer
/// and the JSON output path.
#[derive(Debug, Clone)]
pub struct FeedbackRow {
    /// IRI of the feedback artifact.
    pub iri: String,
    /// Controlled-vocabulary class literal (`gap`, `contradiction`, …).
    pub class: String,
    /// Resolved target role id.
    pub target_role: String,
    /// Lifecycle state literal.
    pub lifecycle_state: String,
    /// Severity hint (`info` / `warning` / `error`).
    pub severity: String,
    /// Evidence citation (truncated for display).
    pub evidence: String,
    /// RFC3339 timestamp of the routing transition, if any.
    pub routed_at: Option<String>,
    /// Source session IRI (PROV-O link back to the emitter).
    pub source_session: String,
}

impl From<&Feedback> for FeedbackRow {
    fn from(fb: &Feedback) -> Self {
        Self {
            iri: fb.iri.as_str().to_string(),
            class: fb.class.clone(),
            target_role: fb.target_role.clone(),
            lifecycle_state: fb.lifecycle_state.clone(),
            severity: fb.severity.as_str().to_string(),
            evidence: fb.evidence.clone(),
            routed_at: fb.routed_at.clone(),
            source_session: fb.source_session.as_str().to_string(),
        }
    }
}

/// Optional filters applied before rendering. All filters are
/// conjunctive — a row must satisfy every set predicate to survive.
#[derive(Debug, Default, Clone)]
pub struct ListFilters {
    /// Restrict to rows whose `lifecycleState` matches.
    pub state: Option<String>,
    /// Restrict to rows whose `feedbackClass` matches.
    pub class: Option<String>,
    /// Restrict to rows whose `targetRole` matches.
    pub target: Option<String>,
}

/// Read every open feedback artifact scoped to the active stream from
/// the persisted orchestration store under `workdir/.dec/`. No filters.
///
/// The function returns rows in `(class, target_role, iri)` order so
/// the renderer can group without re-sorting. Empty result is a
/// well-formed answer (the operator may have nothing pending).
pub fn list(workdir: &Path) -> Result<Vec<FeedbackRow>> {
    list_filtered(workdir, &ListFilters::default())
}

/// Same as [`list`] but applies the supplied filters before sorting.
pub fn list_filtered(workdir: &Path, filters: &ListFilters) -> Result<Vec<FeedbackRow>> {
    let store = open_readonly_store(workdir)?;
    let stream = active_stream_iri(&store)?;
    let mut rows: Vec<FeedbackRow> = list_open(&store, &stream)
        .map_err(|e| anyhow!("listing open feedback: {e}"))?
        .iter()
        .map(FeedbackRow::from)
        .collect();
    rows.retain(|r| match_filters(r, filters));
    rows.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then_with(|| a.target_role.cmp(&b.target_role))
            .then_with(|| a.iri.cmp(&b.iri))
    });
    Ok(rows)
}

fn match_filters(row: &FeedbackRow, filters: &ListFilters) -> bool {
    if let Some(s) = &filters.state {
        if row.lifecycle_state != *s {
            return false;
        }
    }
    if let Some(c) = &filters.class {
        if row.class != *c {
            return false;
        }
    }
    if let Some(t) = &filters.target {
        if row.target_role != *t {
            return false;
        }
    }
    true
}

/// Render the list as a human-readable table grouped by class and
/// target role. The exact text is part of the TC-039 contract.
#[must_use]
pub fn format_list(rows: &[FeedbackRow]) -> String {
    if rows.is_empty() {
        return "(no open feedback)\n".to_string();
    }
    let mut out = String::new();
    let mut current_class: Option<&str> = None;
    let mut current_target: Option<&str> = None;
    for row in rows {
        if current_class != Some(row.class.as_str()) {
            out.push_str(&format!("class: {}\n", row.class));
            current_class = Some(row.class.as_str());
            current_target = None;
        }
        if current_target != Some(row.target_role.as_str()) {
            out.push_str(&format!("  target: {}\n", row.target_role));
            current_target = Some(row.target_role.as_str());
        }
        out.push_str(&format!(
            "    - {iri}  [{state}]  severity={sev}  evidence={ev}\n",
            iri = row.iri,
            state = row.lifecycle_state,
            sev = row.severity,
            ev = truncate_evidence(&row.evidence, 80),
        ));
    }
    out
}

/// Render the list as a JSON array of feedback objects. Used by the
/// `--format json` path; each row mirrors the fields exposed in the
/// text renderer.
#[must_use]
pub fn format_list_json(rows: &[FeedbackRow]) -> String {
    if rows.is_empty() {
        return "[]\n".to_string();
    }
    let mut out = String::from("[\n");
    for (i, row) in rows.iter().enumerate() {
        out.push_str("  {");
        out.push_str(&format!("\"iri\": \"{}\", ", json_escape(&row.iri)));
        out.push_str(&format!("\"class\": \"{}\", ", json_escape(&row.class)));
        out.push_str(&format!(
            "\"target_role\": \"{}\", ",
            json_escape(&row.target_role)
        ));
        out.push_str(&format!(
            "\"lifecycle_state\": \"{}\", ",
            json_escape(&row.lifecycle_state)
        ));
        out.push_str(&format!("\"severity\": \"{}\", ", json_escape(&row.severity)));
        out.push_str(&format!(
            "\"evidence\": \"{}\", ",
            json_escape(&row.evidence)
        ));
        out.push_str(&format!(
            "\"routed_at\": {}, ",
            json_opt(row.routed_at.as_deref())
        ));
        out.push_str(&format!(
            "\"source_session\": \"{}\"",
            json_escape(&row.source_session)
        ));
        out.push('}');
        if i + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(iri: &str, class: &str, target: &str, state: &str) -> FeedbackRow {
        FeedbackRow {
            iri: iri.to_string(),
            class: class.to_string(),
            target_role: target.to_string(),
            lifecycle_state: state.to_string(),
            severity: "warning".to_string(),
            evidence: format!("evidence for {iri}"),
            routed_at: None,
            source_session: "urn:s:1".to_string(),
        }
    }

    #[test]
    fn format_empty_produces_marker() {
        assert_eq!(format_list(&[]), "(no open feedback)\n");
    }

    #[test]
    fn format_groups_by_class_and_target() {
        let rows = vec![
            row("urn:f:4", "contradiction", "architect", "produced"),
            row("urn:f:3", "gap", "architect", "produced"),
            row("urn:f:1", "gap", "spec-author", "produced"),
            row("urn:f:2", "gap", "spec-author", "routed"),
        ];
        let out = format_list(&rows);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "class: contradiction");
        assert_eq!(lines[1], "  target: architect");
        let gap_idx = lines
            .iter()
            .position(|l| *l == "class: gap")
            .expect("gap section");
        assert_eq!(lines[gap_idx + 1], "  target: architect");
    }

    #[test]
    fn json_empty_emits_array_marker() {
        assert_eq!(format_list_json(&[]), "[]\n");
    }

    #[test]
    fn json_one_row_has_required_fields() {
        let rows = vec![row("urn:f:1", "gap", "spec-author", "produced")];
        let out = format_list_json(&rows);
        assert!(out.contains("\"iri\": \"urn:f:1\""));
        assert!(out.contains("\"class\": \"gap\""));
        assert!(out.contains("\"target_role\": \"spec-author\""));
        assert!(out.contains("\"lifecycle_state\": \"produced\""));
        assert!(out.contains("\"routed_at\": null"));
    }

    #[test]
    fn filters_state_class_and_target() {
        let rows = vec![
            row("urn:f:1", "gap", "spec-author", "produced"),
            row("urn:f:2", "gap", "spec-author", "routed"),
            row("urn:f:3", "contradiction", "architect", "produced"),
        ];
        let kept: Vec<&FeedbackRow> = rows
            .iter()
            .filter(|r| {
                match_filters(
                    r,
                    &ListFilters {
                        state: Some("produced".to_string()),
                        class: Some("gap".to_string()),
                        target: None,
                    },
                )
            })
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].iri, "urn:f:1");
    }
}
