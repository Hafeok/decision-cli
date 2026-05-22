//! TC-038 — Closed feedback references its addressing artifact via PROV-O.
//!
//! Per ADR-024 §SHACL fragment, the `closed` terminal state requires both
//! `dec:closedBy` (the actor that closed the loop) and `dec:addressingArtifact`
//! (the artifact that resolved the feedback — the PROV-O link from the closed
//! feedback to its resolution). The two layers must agree:
//!
//! 1. The Rust `validate_quads` SHACL validator rejects a `closed` feedback
//!    that omits either field.
//! 2. The `StreamWriter` chokepoint refuses the write before the underlying
//!    writer sees it.
//!
//! This test fixes the invariant in code: a closed feedback artifact can
//! never be persisted without a queryable PROV-O link back to whatever
//! artifact addressed the underlying issue.

use std::sync::Arc;

use decision_cli::core::feedback::{validate_quads, Feedback, LifecycleState, Severity};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Subject, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/feedback-closed-provo";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/act-cp-1";
const CLOSER_SESSION_IRI: &str = "https://decision-cli.dev/ns/session/closer-cp-1";
const ADDRESSING_IRI: &str = "urn:dec:feature-spec:FT-026:amendment-1";
const FEEDBACK_IRI: &str = "urn:dec:feedback:tc-038:1";

const DEC_ADDRESSING_ARTIFACT: &str = "https://decision-cli.dev/ns#addressingArtifact";
const DEC_CLOSED_BY: &str = "https://decision-cli.dev/ns#closedBy";

fn closed_feedback() -> Feedback {
    Feedback {
        iri: NamedNode::new(FEEDBACK_IRI).expect("feedback iri"),
        class: "gap".to_string(),
        severity: Severity::Warning,
        target_role: "spec-author".to_string(),
        evidence: "feature_spec FT-026 line 12 underspecifies addressing flow".to_string(),
        recommendation: None,
        lifecycle_state: LifecycleState::Closed.as_str().to_string(),
        source_session: NamedNode::new(SESSION_IRI).expect("session iri"),
        source_artifact: None,
        addressing_artifact: Some(NamedNode::new(ADDRESSING_IRI).expect("addressing iri")),
        closed_by: Some(NamedNode::new(CLOSER_SESSION_IRI).expect("closer session iri")),
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: NamedNode::new(STREAM_IRI).expect("stream iri"),
    }
}

#[test]
fn closed_with_addressing_artifact_passes_shacl() {
    let f = closed_feedback();
    let quads = f.to_quads(orchestration_graph());
    validate_quads(&quads).expect("closed feedback with full PROV-O closure must pass SHACL");
}

#[test]
fn closed_without_addressing_artifact_fails_shacl() {
    let mut f = closed_feedback();
    f.addressing_artifact = None;
    let quads = f.to_quads(orchestration_graph());
    let err = validate_quads(&quads)
        .expect_err("closed feedback missing dec:addressingArtifact must fail SHACL");
    assert!(
        err.report.contains("dec:addressingArtifact"),
        "report must reference dec:addressingArtifact; got:\n{}",
        err.report
    );
}

#[test]
fn closed_without_closed_by_fails_shacl() {
    let mut f = closed_feedback();
    f.closed_by = None;
    let quads = f.to_quads(orchestration_graph());
    let err =
        validate_quads(&quads).expect_err("closed feedback missing dec:closedBy must fail SHACL");
    assert!(
        err.report.contains("dec:closedBy"),
        "report must reference dec:closedBy; got:\n{}",
        err.report
    );
}

#[test]
fn stream_writer_refuses_closed_without_addressing_artifact() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let mut f = closed_feedback();
    f.addressing_artifact = None;
    let quads = f.to_quads(orchestration_graph());
    let err = writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect_err("StreamWriter must refuse a closed feedback missing dec:addressingArtifact");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("SHACL violation") && msg.contains("dec:addressingArtifact"),
        "writer must surface a SHACL violation citing dec:addressingArtifact; got: {msg}"
    );

    // Refused write must not be persisted.
    let exists = store
        .quads_for_pattern(Some(Subject::NamedNode(f.iri).as_ref()), None, None, None)
        .next()
        .is_some();
    assert!(
        !exists,
        "refused closed feedback must not be persisted in the orchestration store"
    );
}

#[test]
fn closed_feedback_links_to_addressing_artifact_via_query() {
    // End-to-end: persist a closed feedback through the writer, then
    // SPARQL-query for the PROV-O closure link. This is the durable
    // shape: every closed feedback artifact yields exactly one
    // dec:addressingArtifact IRI when queried by IRI.
    use oxigraph::sparql::QueryResults;

    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let f = closed_feedback();
    let quads = f.to_quads(orchestration_graph());
    writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect("a closed feedback with full PROV-O closure must commit");

    // Confirm dec:addressingArtifact lives in the store.
    let mut addressing_iris: Vec<String> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(f.iri.clone()).as_ref()),
            Some(oxigraph::model::NamedNodeRef::new_unchecked(
                DEC_ADDRESSING_ARTIFACT,
            )),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            addressing_iris.push(n.as_str().to_string());
        }
    }
    assert_eq!(
        addressing_iris,
        vec![ADDRESSING_IRI.to_string()],
        "closed feedback must expose its addressing artifact for PROV-O queries"
    );

    // dec:closedBy must also be queryable (the other half of the closure).
    let mut closers: Vec<String> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(f.iri.clone()).as_ref()),
            Some(oxigraph::model::NamedNodeRef::new_unchecked(DEC_CLOSED_BY)),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            closers.push(n.as_str().to_string());
        }
    }
    assert_eq!(
        closers,
        vec![CLOSER_SESSION_IRI.to_string()],
        "closed feedback must expose its dec:closedBy actor"
    );

    // SPARQL ASK proves the same shape, end-to-end, by walking the
    // graph as future PROV-O consumers will. Use a UNION over default
    // and named graphs so the assertion is graph-placement-agnostic.
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ \
           {{ <{fb}> dec:lifecycleState \"closed\" ; \
                     dec:addressingArtifact <{addr}> ; \
                     dec:closedBy <{closer}> . }} \
           UNION \
           {{ GRAPH ?g {{ \
               <{fb}> dec:lifecycleState \"closed\" ; \
                      dec:addressingArtifact <{addr}> ; \
                      dec:closedBy <{closer}> . \
           }} }} \
         }}",
        fb = FEEDBACK_IRI,
        addr = ADDRESSING_IRI,
        closer = CLOSER_SESSION_IRI,
    );
    let answered = matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)));
    assert!(
        answered,
        "SPARQL ASK must confirm the full closed/closedBy/addressingArtifact triple shape"
    );
}
