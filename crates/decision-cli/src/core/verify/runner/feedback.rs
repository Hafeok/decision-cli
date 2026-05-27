//! Phase-5 feedback emission for failed evidence-bearing steps
//! (FT-098 §Phase 5.5).
//!
//! For every `(step, TC)` pair whose step has `outcome ∈ {fail,
//! unrunnable}` and whose parent step declares one or more
//! `dec:providesEvidenceFor` links, this module mints one
//! `dec:Feedback` artifact per linked TC through the StreamWriter
//! chokepoint.
//!
//! Feedback emission is best-effort per FT-098 §Behaviour — a write
//! failure is logged and does not alter the result verdict.

use std::path::Path;
use std::sync::Arc;

use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use crate::core::feedback::artifact::{Feedback, Severity};
use crate::core::feedback::class::FeedbackClass;
use crate::core::ontology::verdict::Verdict;
use crate::core::ontology::verification_graph::VerificationGraph;
use crate::core::ontology::verification_result::StepOutcome;
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::orchestration_graph;
use crate::core::StreamWriter;

use super::kinds::StepRunTrace;

/// Emit one `dec:Feedback` per linked TC for every step with
/// `outcome ∈ {fail, unrunnable}`. Returns the IRIs of every artifact
/// successfully committed.
///
/// FT-108: defect feedback routes by the graph's verdict:
///   - `rejected` (evidence regression on a TC) → `targetRole = "implementer"`
///   - `amendment-required` (graph-side failure) → `targetRole = "verifier"`
/// Unrunnable steps still emit `class = gap` to `spec-author` regardless.
pub(crate) fn emit_feedback_for_failures(
    workdir: &Path,
    graph: &VerificationGraph,
    traces: &[StepRunTrace],
    verdict: Verdict,
    run_activity: &NamedNode,
) -> Vec<NamedNode> {
    let mut emitted: Vec<NamedNode> = Vec::new();
    let to_emit = collect_feedback_inputs(graph, traces, verdict);
    if to_emit.is_empty() {
        return emitted;
    }
    // Load + open writer once.
    let dump = orchestration_dump_path(workdir);
    let Ok(store) = load_store_from_dump(&dump) else {
        return emitted;
    };
    let store = Arc::new(store);
    let Ok(scope) = ActiveScope::load(workdir) else {
        return emitted;
    };
    let Ok(stream_iri) = NamedNode::new(&scope.stream_iri) else {
        return emitted;
    };
    let Ok(writer) = StreamWriter::open(Arc::clone(&store), stream_iri.clone()) else {
        return emitted;
    };

    for input in to_emit {
        if let Some(iri) = write_one_feedback(&writer, &stream_iri, run_activity, &input) {
            emitted.push(iri);
        }
    }
    // Best-effort persist; ignore I/O errors.
    let _ = persist_store(&store, &dump);
    emitted
}

/// Per-(step, TC) feedback emission input.
struct FeedbackInput {
    tc: String,
    class: FeedbackClass,
    target_role: String,
    evidence: String,
}

fn collect_feedback_inputs(
    graph: &VerificationGraph,
    traces: &[StepRunTrace],
    verdict: Verdict,
) -> Vec<FeedbackInput> {
    let mut out: Vec<FeedbackInput> = Vec::new();
    for (i, step) in graph.steps.iter().enumerate() {
        if step.provides_evidence_for.is_empty() {
            continue;
        }
        let Some(trace) = traces.get(i) else { continue };
        let class = match trace.outcome {
            StepOutcome::Fail => FeedbackClass::Defect,
            StepOutcome::Unrunnable => FeedbackClass::Gap,
            StepOutcome::Pass => continue,
        };
        let target_role = target_role_for(class, verdict).to_string();
        let evidence = build_evidence_body(step.kind, trace, i);
        for tc in &step.provides_evidence_for {
            out.push(FeedbackInput {
                tc: tc.as_str().to_string(),
                class,
                target_role: target_role.clone(),
                evidence: evidence.clone(),
            });
        }
    }
    out
}

/// FT-108 routing rule: defect feedback's target role depends on the
/// per-graph verdict. `rejected` (code regression) → `"implementer"`;
/// `amendment-required` (graph at fault) → `"verifier"`. Anything else
/// (including the `gap` class for `unrunnable` steps) falls back to the
/// class's default target role.
fn target_role_for(class: FeedbackClass, verdict: Verdict) -> &'static str {
    match (class, verdict) {
        (FeedbackClass::Defect, Verdict::Rejected) => "implementer",
        (FeedbackClass::Defect, Verdict::AmendmentRequired) => "verifier",
        _ => class.default_target_role(),
    }
}

fn build_evidence_body(
    _kind: crate::core::ontology::verification_graph::StepKind,
    trace: &StepRunTrace,
    step_index: usize,
) -> String {
    if !trace.error_message.is_empty() {
        return format!(
            "step {step_index} produced outcome {outcome}; {msg}",
            outcome = trace.outcome.as_str(),
            msg = trace.error_message,
        );
    }
    format!(
        "step {step_index} produced outcome {outcome}",
        outcome = trace.outcome.as_str(),
    )
}

fn write_one_feedback(
    writer: &StreamWriter,
    stream_iri: &NamedNode,
    run_activity: &NamedNode,
    input: &FeedbackInput,
) -> Option<NamedNode> {
    let id = uuid::Uuid::new_v4();
    let iri = NamedNode::new_unchecked(format!("urn:dec:feedback:{id}"));
    let mut source_artifact: Option<NamedNode> = None;
    if let Ok(node) = NamedNode::new(input.tc.as_str()) {
        source_artifact = Some(node);
    }
    let feedback = Feedback {
        iri: iri.clone(),
        class: input.class.as_iri_value().to_string(),
        severity: Severity::Error,
        target_role: input.target_role.clone(),
        evidence: input.evidence.clone(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: run_activity.clone(),
        source_artifact,
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: stream_iri.clone(),
    };
    let quads = feedback.to_quads(orchestration_graph());
    let mutation = Mutation::insert(quads);
    match writer.commit(mutation) {
        Ok(_) => Some(iri),
        Err(_) => None,
    }
}
