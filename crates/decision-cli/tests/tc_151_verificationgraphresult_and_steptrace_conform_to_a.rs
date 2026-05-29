//! TC-151 — VerificationGraphResult and StepTrace conform to ADR-028 SHACL shapes.
//!
//! Spec: `.product/tests/TC-151-verificationgraphresult-and-steptrace-conform-to-a.md`
//! Validates: FT-097 · ADR-028.
//!
//! Round-trips a representative `dec:VerificationGraphResult` (and its
//! inner traces) through `StreamWriter` for a well-formed payload, and
//! asserts the SHACL gate refuses every malformed variant declared in
//! the TC scenarios.

use std::sync::Arc;

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::core::ontology::verification_result::{
    EvidenceProjection, StepOutcome, VerificationGraphResult, VerificationStepTrace,
};
use decision_cli::vocab::{verify_graph_named_graph, verify_result_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-151";
const RUN_ACTIVITY_IRI: &str = "https://decision-cli.dev/ns/activity/run/RUN-001";
const AGENT_IRI: &str = "https://decision-cli.dev/ns/agent/runner";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_graph() -> VerificationGraph {
    let id = "VG-FIXTURE-001";
    let steps = vec![
        VerificationStep::new(
            id,
            0,
            StepFields::ShellCommand {
                command: "true".into(),
                expect_exit_code: Some(0),
                capture_output: None,
            },
        ),
        VerificationStep::new(
            id,
            1,
            StepFields::SparqlAssertion {
                target: ".dec/store".into(),
                query: "SELECT * { ?s ?p ?o }".into(),
                expect_rows: Some(1),
            },
        ),
        VerificationStep::new(
            id,
            2,
            StepFields::FileAssertion {
                path: ".dec/state".into(),
                expect_hash: None,
                expect_content: None,
            },
        ),
    ];
    let feature = NamedNode::new_unchecked("https://decision-cli.dev/ns/feature/FT-097");
    let env = NamedNode::new_unchecked("https://decision-cli.dev/ns/bench/BNCH-001");
    VerificationGraph::new(id, ArtifactRef(feature), env, steps)
}

fn well_formed_result(graph: &VerificationGraph) -> VerificationGraphResult {
    let result_id = "https://decision-cli.dev/ns/result/VGR-001";
    let traces: Vec<VerificationStepTrace> = graph
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| VerificationStepTrace {
            id: format!("{result_id}/step/{i}"),
            traces_step: step.id.as_str().to_string(),
            outcome: StepOutcome::Pass,
            started_at: "2026-05-26T14:00:00Z".into(),
            ended_at: "2026-05-26T14:00:00.420Z".into(),
            exit_code: Some(0),
            stdout_excerpt: "ok\n".into(),
            stderr_excerpt: String::new(),
            error_message: String::new(),
            was_generated_by: RUN_ACTIVITY_IRI.into(),
        })
        .collect();
    VerificationGraphResult {
        id: result_id.into(),
        result_of: graph.id.as_str().to_string(),
        ran_in_environment: graph.environment.as_str().to_string(),
        verdict: Verdict::Approved,
        started_at: "2026-05-26T14:00:00Z".into(),
        ended_at: "2026-05-26T14:00:01.130Z".into(),
        step_traces: traces,
        evidence_for: vec![EvidenceProjection {
            tc: "https://decision-cli.dev/ns/tc/TC-EVI-A".into(),
            outcome: StepOutcome::Pass,
            from_step: format!("{result_id}/step/0"),
        }],
        rationale: "all 3 steps passed; 2 TCs received pass evidence".into(),
        was_generated_by: RUN_ACTIVITY_IRI.into(),
        was_attributed_to: AGENT_IRI.into(),
        created_at: "2026-05-26T14:00:01.130Z".into(),
    }
}

fn writer_with_graph_seeded(graph: &VerificationGraph) -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer =
        StreamWriter::bootstrap(Arc::clone(&store), stream).expect("StreamWriter::bootstrap");
    let graph_quads = graph.to_quads(verify_graph_named_graph());
    writer
        .commit(Mutation::insert(graph_quads))
        .expect("seed parent VerificationGraph quads");
    (store, writer)
}

fn commit_result(writer: &StreamWriter, result: &VerificationGraphResult) -> Result<(), String> {
    let quads = result.to_quads(verify_result_graph());
    writer
        .commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn truncate_trace_iri(traces: &[VerificationStepTrace], n: usize) -> Vec<VerificationStepTrace> {
    traces.iter().take(n).cloned().collect()
}

// ---------------------------------------------------------------------------
// The aggregate TC test — all five scenarios.
// ---------------------------------------------------------------------------

#[test]
fn tc_151_verificationgraphresult_and_steptrace_conform_to_a() {
    scenario_a_well_formed_result_is_accepted();
    scenario_b_length_parity_violation_is_rejected();
    scenario_c_unknown_step_iri_is_rejected();
    scenario_d_verdict_vs_trace_inconsistency_is_rejected();
    scenario_e_short_rationale_is_rejected();
}

fn scenario_a_well_formed_result_is_accepted() {
    let graph = fixture_graph();
    let (store, writer) = writer_with_graph_seeded(&graph);
    let result = well_formed_result(&graph);
    commit_result(&writer, &result).expect("Scenario A: well-formed result must commit");
    // Sanity: the result subject is reachable in the store.
    let result_iri = NamedNode::new_unchecked(&result.id);
    let mut found = false;
    for q in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(result_iri).as_ref()),
            Some(
                NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").as_ref(),
            ),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if matches!(&q.object, Term::NamedNode(n) if n.as_str() == "https://decision-cli.dev/ns#VerificationGraphResult")
        {
            found = true;
            break;
        }
    }
    assert!(found, "Scenario A: result subject must reach store");
}

fn scenario_b_length_parity_violation_is_rejected() {
    let graph = fixture_graph();
    let (_store, writer) = writer_with_graph_seeded(&graph);
    let mut result = well_formed_result(&graph);
    // Emit only two traces for a three-step graph.
    result.step_traces = truncate_trace_iri(&result.step_traces, 2);
    let err = commit_result(&writer, &result)
        .expect_err("Scenario B: length-parity mismatch must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "Scenario B: expected SHACL violation, got {err}"
    );
    assert!(
        err.contains("stepTraces"),
        "Scenario B: error must name dec:stepTraces, got {err}"
    );
}

fn scenario_c_unknown_step_iri_is_rejected() {
    let graph = fixture_graph();
    let (_store, writer) = writer_with_graph_seeded(&graph);
    let mut result = well_formed_result(&graph);
    // Re-point trace 2 at an IRI that doesn't exist in the parent graph.
    result.step_traces[2].traces_step =
        "https://decision-cli.dev/ns/step/VG-FIXTURE-001/99".into();
    let err = commit_result(&writer, &result)
        .expect_err("Scenario C: unknown step IRI must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "Scenario C: expected SHACL violation, got {err}"
    );
    assert!(
        err.contains("tracesStep"),
        "Scenario C: error must name dec:tracesStep, got {err}"
    );
}

fn scenario_d_verdict_vs_trace_inconsistency_is_rejected() {
    let graph = fixture_graph();
    let (_store, writer) = writer_with_graph_seeded(&graph);
    let mut result = well_formed_result(&graph);
    // Mark step 2 as fail, but leave verdict = Approved.
    result.step_traces[2].outcome = StepOutcome::Fail;
    result.step_traces[2].error_message = "expected exit 0, got 1".into();
    // verdict stays Approved → inconsistency
    let err = commit_result(&writer, &result)
        .expect_err("Scenario D: verdict-vs-trace inconsistency must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "Scenario D: expected SHACL violation, got {err}"
    );
    assert!(
        err.contains("verdict"),
        "Scenario D: error must name dec:verdict, got {err}"
    );
}

fn scenario_e_short_rationale_is_rejected() {
    let graph = fixture_graph();
    let (_store, writer) = writer_with_graph_seeded(&graph);
    let mut result = well_formed_result(&graph);
    result.rationale = "ok".into();
    let err = commit_result(&writer, &result)
        .expect_err("Scenario E: short rationale must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "Scenario E: expected SHACL violation, got {err}"
    );
    assert!(
        err.contains("rationale"),
        "Scenario E: error must name dec:rationale, got {err}"
    );
}

// Suppress unused warnings on imports used only in struct construction.
#[allow(dead_code)]
fn _used(_q: &Quad) {}
