//! `dec feedback {list,…}` — feedback inspection and routing surface (FT-029 / FT-033).
//!
//! Slice 3 introduces a thin read-only surface over `dec:Feedback`
//! artifacts. The list view groups open feedback by class and target so
//! the operator can answer "what's pending in the system right now?"
//! without writing SPARQL.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, this feature
//! module imports from `core::feedback::*` (including the FT-029
//! routing table) and never reaches into sibling features. The
//! orchestration store is opened read-only — feedback mutations happen
//! through `StreamWriter` driven by other features.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

use crate::core::feedback::{list_open, Feedback};

/// One row of `dec feedback list --json` — used by the human renderer
/// and (post-FT-033) by the JSON output path.
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
        }
    }
}

/// Read every open feedback artifact scoped to the active stream from
/// the persisted orchestration store under `workdir/.dec/`.
///
/// The function returns rows in `(class, target_role, iri)` order so
/// the renderer can group without re-sorting. Empty result is a
/// well-formed answer (the operator may have nothing pending).
pub fn list(workdir: &Path) -> Result<Vec<FeedbackRow>> {
    let store = open_store(workdir)?;
    let stream = active_stream(&store)?;
    let mut rows: Vec<FeedbackRow> = list_open(&store, &stream)
        .map_err(|e| anyhow!("listing open feedback: {e}"))?
        .iter()
        .map(FeedbackRow::from)
        .collect();
    rows.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then_with(|| a.target_role.cmp(&b.target_role))
            .then_with(|| a.iri.cmp(&b.iri))
    });
    Ok(rows)
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

fn truncate_evidence(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut cut = max.saturating_sub(1);
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…", &s[..cut])
    }
}

fn open_store(workdir: &Path) -> Result<Store> {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    if !dump_path.exists() {
        return Err(anyhow!(
            "no orchestration store at {} — run `dec init` first",
            dump_path.display()
        ));
    }
    let bytes =
        std::fs::read(&dump_path).with_context(|| format!("reading {}", dump_path.display()))?;
    let store = Store::new().context("opening in-memory orchestration store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;
    Ok(store)
}

fn active_stream(store: &Store) -> Result<oxigraph::model::NamedNode> {
    use oxigraph::model::Term;
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               { ?s a dec:ValueStream . } \
               UNION \
               { GRAPH ?g { ?s a dec:ValueStream . } } \
             } LIMIT 1";
    let QueryResults::Solutions(mut sols) = store.query(q).context("locating active stream")?
    else {
        return Err(anyhow!(
            "no dec:ValueStream artifact found — store may be corrupt"
        ));
    };
    let Some(sol) = sols.next() else {
        return Err(anyhow!(
            "no dec:ValueStream artifact found — store may be corrupt"
        ));
    };
    let sol = sol.context("decoding active-stream row")?;
    let Some(Term::NamedNode(node)) = sol.get("s").cloned() else {
        return Err(anyhow!("active stream subject is not an IRI"));
    };
    Ok(node)
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
        }
    }

    #[test]
    fn format_empty_produces_marker() {
        assert_eq!(format_list(&[]), "(no open feedback)\n");
    }

    #[test]
    fn format_groups_by_class_and_target() {
        // `format_list` assumes the input is already sorted (`list()`
        // is the sort chokepoint). Pass pre-sorted rows that mirror
        // the same ordering — by (class, target, iri) ascending — so
        // the assertion targets the renderer in isolation.
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
        let spec_idx = lines[gap_idx..]
            .iter()
            .position(|l| *l == "  target: spec-author")
            .expect("spec-author target group");
        assert!(spec_idx > 0);
    }

    #[test]
    fn evidence_truncation_keeps_long_inputs_short() {
        let long = "x".repeat(120);
        let truncated = truncate_evidence(&long, 80);
        assert!(truncated.ends_with('…'));
        assert!(truncated.chars().count() <= 81);
    }
}
