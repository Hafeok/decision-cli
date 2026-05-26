//! Text + JSON renderers for `dec verify feature` (FT-099).
//!
//! Pure functions over [`super::FeatureVerifyResponse`]. The CLI
//! surface calls these to emit stdout; no I/O.

use std::fmt::Write as _;

use serde_json::json;

use super::FeatureVerifyResponse;

/// Human-readable per-graph + per-TC + aggregate verdict block.
#[must_use]
pub fn render_text(resp: &FeatureVerifyResponse) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Feature {ft}", ft = resp.feature_id);

    if resp.dry_run {
        write_dry_run(&mut out, resp);
        return out;
    }

    write_per_graph(&mut out, resp);
    out.push('\n');
    write_per_tc(&mut out, resp);
    out.push('\n');
    write_coverage_gaps(&mut out, resp);
    out.push('\n');
    if let Some(agg) = &resp.aggregate {
        let _ = writeln!(out, "Aggregate verdict: {}", agg.verdict);
        let _ = writeln!(out, "Rationale:        {}", agg.rationale);
    }
    if !resp.coverage_gaps.is_empty() {
        let _ = writeln!(
            out,
            "\nSuggestion: run `dec verify graph generate {ft} --environment <env>` to remedy uncovered TCs.",
            ft = resp.feature_id,
        );
    }
    out
}

fn write_dry_run(out: &mut String, resp: &FeatureVerifyResponse) {
    let Some(enumeration) = &resp.enumeration else {
        return;
    };
    out.push_str("Dry-run enumeration\n");
    if enumeration.would_run.is_empty() {
        out.push_str("  Would run: (none)\n");
    } else {
        for entry in &enumeration.would_run {
            let _ = writeln!(out, "  Would run: {} ({})", entry.vg, entry.env);
        }
    }
    if enumeration.would_reuse.is_empty() {
        out.push_str("  Would reuse: (none)\n");
    } else {
        for entry in &enumeration.would_reuse {
            let vgr_label = entry.vgr.as_deref().unwrap_or("");
            let _ = writeln!(
                out,
                "  Would reuse: {} ({}) {}",
                entry.vg, entry.env, vgr_label
            );
        }
    }
}

fn write_per_graph(out: &mut String, resp: &FeatureVerifyResponse) {
    if resp.per_graph.is_empty() {
        out.push_str("  (no covering graphs found)\n");
        return;
    }
    for row in &resp.per_graph {
        let verdict = row.verdict.as_deref().unwrap_or("");
        let note = row.note.as_deref().unwrap_or("");
        let suffix = if note.is_empty() {
            String::new()
        } else {
            format!(" — {note}")
        };
        let _ = writeln!(
            out,
            "  {vg} ({env}) → {verdict}{suffix}",
            vg = row.vg,
            env = row.env,
            verdict = verdict,
            suffix = suffix,
        );
    }
}

fn write_per_tc(out: &mut String, resp: &FeatureVerifyResponse) {
    out.push_str("  Per-TC verdict:\n");
    if resp.per_tc.is_empty() {
        out.push_str("    (no TCs to report)\n");
        return;
    }
    for row in &resp.per_tc {
        let _ = writeln!(
            out,
            "    {tc:<12} {verdict:<22}  ({rationale})",
            tc = row.tc,
            verdict = row.verdict,
            rationale = row.rationale,
        );
    }
}

fn write_coverage_gaps(out: &mut String, resp: &FeatureVerifyResponse) {
    if resp.coverage_gaps.is_empty() {
        out.push_str("  Coverage gaps: none\n");
    } else {
        let _ = writeln!(out, "  Coverage gaps: {}", resp.coverage_gaps.join(", "));
    }
}

/// Render the full JSON document.
#[must_use]
pub fn render_json(resp: &FeatureVerifyResponse) -> String {
    let value = json!({
        "session_id": resp.session_id,
        "feature_id": resp.feature_id,
        "per_graph": resp.per_graph,
        "per_tc": resp.per_tc,
        "coverage_gaps": resp.coverage_gaps,
        "aggregate": resp.aggregate,
        "dry_run": resp.dry_run,
        "enumeration": resp.enumeration,
        "would_run": resp.enumeration.as_ref().map(|e| &e.would_run),
        "would_reuse": resp.enumeration.as_ref().map(|e| &e.would_reuse),
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}
