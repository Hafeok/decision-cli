//! Paired-session lookup helpers for `dec session show` (FT-025 / ADR-017).
//!
//! Resolves the `dec:DispatchGroup` containing the queried session and
//! its `dec:VerificationVerdict` (when present), then renders the
//! FT-025 "Paired:" block consumed by [`super::session_show`].

use std::fmt::Write;

use anyhow::{Context, Result};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::DEC_NS;

/// In-memory record of a `dec:DispatchGroup` reachable from a session.
#[derive(Debug, Clone, Default)]
struct PairedRow {
    /// IRI of the dispatch group containing this session.
    group_iri: String,
    /// `dec:dispatchStatus` literal.
    status: String,
    /// `dec:hasActionSession` IRI.
    action_session: String,
    /// `dec:hasInterpretationSession` IRI (may be empty).
    interpretation_session: String,
}

/// Materialised verifier verdict reachable through the dispatch group.
#[derive(Debug, Clone, Default)]
struct VerdictRow {
    /// Verdict artifact IRI.
    iri: String,
    /// `dec:verdict` literal (approved / rejected / amendment-required).
    verdict: String,
    /// `dec:rationale` literal.
    rationale: String,
    /// `dec:amendmentGuidance` literal, when present.
    amendment_guidance: String,
    /// IRIs of TCs / ADRs the verdict cites via `dec:violates`.
    violates: Vec<String>,
}

/// Locate the `dec:DispatchGroup` (if any) that contains `session_iri`
/// either as action or interpretation session, and render the FT-025
/// `Paired:` block. Returns `Ok(None)` when the session is not part of
/// any group — the slice-1 output stands on its own.
pub(super) fn lookup_paired_block(store: &Store, session_iri: &str) -> Result<Option<String>> {
    let Some(row) = lookup_paired_group(store, session_iri)? else {
        return Ok(None);
    };
    let verdict = lookup_verdict_for_group(store, &row)?;
    Ok(Some(render_paired_block(&row, session_iri, verdict.as_ref())))
}

fn lookup_paired_group(store: &Store, session_iri: &str) -> Result<Option<PairedRow>> {
    let q = build_paired_group_query(session_iri);
    let results = store
        .query(q.as_str())
        .context("session-show paired SPARQL")?;
    let QueryResults::Solutions(mut sols) = results else {
        return Ok(None);
    };
    let Some(sol_res) = sols.next() else {
        return Ok(None);
    };
    let sol = sol_res.context("session-show paired solution")?;
    let group_iri = sol.get("group").map(term_iri).unwrap_or_default();
    if group_iri.is_empty() {
        return Ok(None);
    }
    Ok(Some(PairedRow {
        group_iri,
        status: sol.get("status").map(term_literal).unwrap_or_default(),
        action_session: sol.get("action").map(term_iri).unwrap_or_default(),
        interpretation_session: sol.get("interp").map(term_iri).unwrap_or_default(),
    }))
}

fn build_paired_group_query(session_iri: &str) -> String {
    format!(
        "PREFIX dec: <{DEC_NS}>
SELECT ?group ?status ?action ?interp WHERE {{
  GRAPH ?g {{
    ?group a dec:DispatchGroup ;
           dec:hasActionSession ?action .
    OPTIONAL {{ ?group dec:dispatchStatus ?status }}
    OPTIONAL {{ ?group dec:hasInterpretationSession ?interp }}
    FILTER ( ?action = <{session_iri}> || ?interp = <{session_iri}> )
  }}
}} LIMIT 1"
    )
}

fn lookup_verdict_for_group(store: &Store, row: &PairedRow) -> Result<Option<VerdictRow>> {
    if row.interpretation_session.is_empty() {
        return Ok(None);
    }
    let q = build_verdict_query(&row.interpretation_session);
    let results = store
        .query(q.as_str())
        .context("session-show verdict SPARQL")?;
    let QueryResults::Solutions(mut sols) = results else {
        return Ok(None);
    };
    let Some(sol_res) = sols.next() else {
        return Ok(None);
    };
    let sol = sol_res.context("session-show verdict solution")?;
    let iri = sol.get("v").map(term_iri).unwrap_or_default();
    if iri.is_empty() {
        return Ok(None);
    }
    let verdict_value = sol.get("verdict").map(term_literal).unwrap_or_default();
    let rationale = sol.get("rationale").map(term_literal).unwrap_or_default();
    let amendment_guidance = sol
        .get("amendmentGuidance")
        .map(term_literal)
        .unwrap_or_default();
    let violates = collect_violates(store, &iri)?;
    Ok(Some(VerdictRow {
        iri,
        verdict: verdict_value,
        rationale,
        amendment_guidance,
        violates,
    }))
}

fn build_verdict_query(interpretation_session_iri: &str) -> String {
    format!(
        "PREFIX dec:  <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?v ?verdict ?rationale ?amendmentGuidance WHERE {{
  GRAPH ?g {{
    ?v a dec:VerificationVerdict ;
       prov:wasGeneratedBy <{interpretation_session_iri}> ;
       dec:verdict ?verdict ;
       dec:rationale ?rationale .
    OPTIONAL {{ ?v dec:amendmentGuidance ?amendmentGuidance }}
  }}
}} LIMIT 1"
    )
}

fn collect_violates(store: &Store, verdict_iri: &str) -> Result<Vec<String>> {
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
SELECT ?ref WHERE {{
  GRAPH ?g {{
    <{verdict_iri}> dec:violates ?ref .
  }}
}}"
    );
    let results = store
        .query(q.as_str())
        .context("session-show violates SPARQL")?;
    let QueryResults::Solutions(sols) = results else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.context("session-show violates row")?;
        if let Some(t) = sol.get("ref") {
            out.push(term_iri(t));
        }
    }
    out.sort();
    Ok(out)
}

fn render_paired_block(
    row: &PairedRow,
    session_iri: &str,
    verdict: Option<&VerdictRow>,
) -> String {
    let mut out = String::new();
    out.push_str("Paired:\n");
    let _ = writeln!(out, "  Dispatch group: {}", row.group_iri);
    let status = if row.status.is_empty() {
        "(unknown)"
    } else {
        row.status.as_str()
    };
    let _ = writeln!(out, "  Status:         {status}");
    let (paired_label, paired_value) = paired_session_label(row, session_iri);
    let _ = writeln!(out, "  {paired_label}: {paired_value}");
    match verdict {
        Some(v) => append_verdict(&mut out, v),
        None => out.push_str("  Verdict:        (none yet)\n"),
    }
    out
}

fn paired_session_label(row: &PairedRow, session_iri: &str) -> (&'static str, String) {
    if row.action_session == session_iri {
        let value = if row.interpretation_session.is_empty() {
            "(not yet dispatched)".to_string()
        } else {
            row.interpretation_session.clone()
        };
        ("Interpretation", value)
    } else {
        let value = if row.action_session.is_empty() {
            "(unknown)".to_string()
        } else {
            row.action_session.clone()
        };
        ("Action", value)
    }
}

fn append_verdict(out: &mut String, v: &VerdictRow) {
    let verdict_value = if v.verdict.is_empty() {
        "(unknown)"
    } else {
        v.verdict.as_str()
    };
    let _ = writeln!(out, "  Verdict:        {verdict_value}");
    let _ = writeln!(out, "    Verdict IRI:        {}", v.iri);
    let rationale_block = wrap_rationale(&v.rationale, 72);
    out.push_str("    Rationale:\n");
    for line in rationale_block {
        let _ = writeln!(out, "      {line}");
    }
    if !v.violates.is_empty() {
        out.push_str("    Violates:\n");
        for r in &v.violates {
            let _ = writeln!(out, "      - {r}");
        }
    }
    if !v.amendment_guidance.is_empty() {
        let guidance = wrap_rationale(&v.amendment_guidance, 72);
        out.push_str("    Amendment guidance:\n");
        for line in guidance {
            let _ = writeln!(out, "      {line}");
        }
    }
}

fn wrap_rationale(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec!["(empty)".to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }
            if current.len() + 1 + word.len() > width {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push("(empty)".to_string());
    }
    lines
}

/// Extract a stringly literal value from an oxigraph Term. Falls back
/// to the IRI / debug repr for non-literal terms.
pub(super) fn term_literal(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

/// Extract an IRI string from an oxigraph Term.
pub(super) fn term_iri(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}
