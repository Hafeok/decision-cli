//! Text + JSON rendering for `dec verify graph run` (FT-099).
//!
//! Pure functions over [`super::GraphRunResponse`]. Surface adapters
//! (CLI) call into these to emit stdout; no I/O here.

use std::fmt::Write as _;

use serde_json::json;

use super::GraphRunResponse;

/// Render the human-readable per-step trace + verdict block.
#[must_use]
pub fn render_text(resp: &GraphRunResponse) -> String {
    let mut out = String::new();
    write_header(&mut out, resp);
    for step in &resp.step_outcomes {
        write_step_row(&mut out, step);
    }
    out.push('\n');
    let _ = writeln!(out, "Verdict: {}", resp.verdict);
    let _ = writeln!(out, "Rationale: {}", resp.rationale);
    let _ = writeln!(out, "Result:    {}", resp.result_id);
    if !resp.emitted_feedback.is_empty() {
        write_feedback_block(&mut out, resp);
    }
    out
}

fn write_header(out: &mut String, resp: &GraphRunResponse) {
    let _ = writeln!(
        out,
        "Running {graph} in {env}",
        graph = resp.graph_id,
        env = resp.environment_id,
    );
}

fn write_step_row(out: &mut String, step: &super::StepSummary) {
    let desc = step.description.as_str();
    let desc = if desc.len() > 60 {
        format!("{}…", &desc[..60])
    } else {
        desc.to_string()
    };
    let _ = writeln!(
        out,
        "  [{idx}] {kind:<20} {outcome:<11} {duration:>5} ms    {desc}",
        idx = step.index,
        kind = step.kind,
        outcome = step.outcome,
        duration = step.duration_ms,
        desc = desc,
    );
}

fn write_feedback_block(out: &mut String, resp: &GraphRunResponse) {
    let _ = writeln!(out, "Feedback:");
    for fb in &resp.emitted_feedback {
        let _ = writeln!(out, "  {fb}");
    }
}

/// Render the JSON document per FT-099 §Outputs (`--format json`).
#[must_use]
pub fn render_json(resp: &GraphRunResponse) -> String {
    let value = json!({
        "session_id": resp.session_id,
        "result_id": resp.result_id,
        "graph_id": resp.graph_id,
        "environment_id": resp.environment_id,
        "verdict": resp.verdict,
        "rationale": resp.rationale,
        "step_outcomes": resp.step_outcomes,
        "emitted_feedback": resp.emitted_feedback,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}
