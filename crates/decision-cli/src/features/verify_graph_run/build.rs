//! Response-building helpers for `dec verify graph run` (FT-099).
//!
//! Maps the runner's [`RunGraphResponse`] into the surface-level
//! [`super::GraphRunResponse`] consumed by both CLI and MCP. Pure
//! functions; no I/O.

use chrono::DateTime;

use crate::core::ontology::verification_graph::VerificationGraph;
use crate::core::ontology::verification_result::{VerificationGraphResult, VerificationStepTrace};
use crate::core::verify::runner::RunGraphResponse;

use super::{GraphRunResponse, StepSummary};

/// Build the surface response from the runner's reply.
#[must_use]
pub(super) fn from_runner(
    resp: RunGraphResponse,
    no_feedback: bool,
    graph: Option<&VerificationGraph>,
) -> GraphRunResponse {
    let RunGraphResponse {
        result,
        verdict,
        step_outcomes: _step_outcomes,
        emitted_feedback,
        result_artifact,
    } = resp;
    let graph_id = result_artifact.result_of.clone();
    let env_id = result_artifact.ran_in_environment.clone();
    let summaries = step_summaries(&result_artifact, graph);
    let feedback = if no_feedback {
        Vec::new()
    } else {
        emitted_feedback
            .iter()
            .map(|n| n.as_str().to_string())
            .collect()
    };
    GraphRunResponse {
        result_id: result.as_str().to_string(),
        graph_id,
        environment_id: env_id,
        verdict: verdict.as_str().to_string(),
        rationale: result_artifact.rationale.clone(),
        step_outcomes: summaries,
        emitted_feedback: feedback,
        session_id: None,
    }
}

fn step_summaries(
    artifact: &VerificationGraphResult,
    graph: Option<&VerificationGraph>,
) -> Vec<StepSummary> {
    artifact
        .step_traces
        .iter()
        .enumerate()
        .map(|(i, trace)| {
            let duration_ms = duration_ms(&trace.started_at, &trace.ended_at);
            let description = description(trace);
            let kind = graph
                .and_then(|g| g.steps.get(i))
                .map(|s| s.kind.as_str().to_string())
                .unwrap_or_else(|| "step".to_string());
            StepSummary {
                index: i,
                kind,
                outcome: trace.outcome.as_str().to_string(),
                duration_ms,
                description,
                step_id: trace.traces_step.clone(),
                trace_id: trace.id.clone(),
            }
        })
        .collect()
}

fn description(trace: &VerificationStepTrace) -> String {
    if !trace.error_message.is_empty() {
        return trace.error_message.clone();
    }
    if !trace.stdout_excerpt.is_empty() {
        return one_line_excerpt(&trace.stdout_excerpt);
    }
    String::new()
}

fn one_line_excerpt(s: &str) -> String {
    let first = s.lines().next().unwrap_or("").trim();
    if first.len() > 80 {
        format!("{}…", &first[..80])
    } else {
        first.to_string()
    }
}

fn duration_ms(started_at: &str, ended_at: &str) -> u64 {
    let s = DateTime::parse_from_rfc3339(started_at);
    let e = DateTime::parse_from_rfc3339(ended_at);
    if let (Ok(s), Ok(e)) = (s, e) {
        let ms = e.signed_duration_since(s).num_milliseconds();
        if ms > 0 {
            return ms as u64;
        }
    }
    0
}
