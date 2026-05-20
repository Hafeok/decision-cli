//! TC-030 — rejected/amendment-required verdicts must cite at least one
//! TC or ADR via `dec:violates` (FT-020 / ADR-018 conditional SHACL).
//!
//! This is the conditional clause of the ADR-018 SHACL shape: an
//! approved verdict may omit `dec:violates`, but `rejected` and
//! `amendment-required` MUST carry ≥ 1 reference.

use std::sync::Arc;

use decision_cli::core::ontology::verdict::{validate_quads, Verdict, VerdictArtifact};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Subject};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/interp-x";

fn base_artifact(verdict: Verdict) -> VerdictArtifact {
    VerdictArtifact {
        iri: NamedNode::new("urn:dec:verdict:tc-030:1").expect("verdict iri"),
        verdict,
        rationale: "Verdict — see linked TC for the specific failure.".to_string(),
        violates: Vec::new(),
        amendment_guidance: None,
        generated_by: NamedNode::new(SESSION_IRI).expect("session iri"),
        used: vec![NamedNode::new("urn:dec:code-change:1").expect("cc iri")],
        in_stream: NamedNode::new(STREAM_IRI).expect("stream iri"),
    }
}

#[test]
fn rejected_without_violates_is_refused() {
    let a = base_artifact(Verdict::Rejected);
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads)
        .expect_err("rejected verdict without dec:violates must be refused");
    assert!(
        err.report.contains("dec:violates"),
        "report should mention dec:violates; got:\n{}",
        err.report
    );
}

#[test]
fn rejected_with_one_citation_passes() {
    let mut a = base_artifact(Verdict::Rejected);
    a.violates = vec![NamedNode::new("urn:tc:TC-029").expect("tc iri")];
    let quads = a.to_quads(orchestration_graph());
    validate_quads(&quads).expect("rejected with one TC citation passes");
}

#[test]
fn amendment_required_without_violates_is_refused() {
    let mut a = base_artifact(Verdict::AmendmentRequired);
    a.amendment_guidance = Some("Add the missing SHACL shape.".to_string());
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads)
        .expect_err("amendment-required verdict without dec:violates must be refused");
    assert!(
        err.report.contains("dec:violates"),
        "report should mention dec:violates; got:\n{}",
        err.report
    );
}

#[test]
fn amendment_required_with_violates_and_guidance_passes() {
    let mut a = base_artifact(Verdict::AmendmentRequired);
    a.violates = vec![NamedNode::new("urn:adr:ADR-018").expect("adr iri")];
    a.amendment_guidance = Some("Add the missing SHACL shape.".to_string());
    let quads = a.to_quads(orchestration_graph());
    validate_quads(&quads).expect("amendment-required with both fields passes");
}

#[test]
fn stream_writer_refuses_rejected_without_citation() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let a = base_artifact(Verdict::Rejected); // no violates
    let quads = a.to_quads(orchestration_graph());
    let mutation = Mutation::insert(quads.iter().cloned());
    let err = writer
        .commit(mutation)
        .expect_err("StreamWriter must refuse rejected verdict missing dec:violates");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("SHACL violation") && msg.contains("dec:violates"),
        "writer must report a SHACL violation citing dec:violates; got: {msg}"
    );

    // Refused verdict must not be persisted.
    let exists = store
        .quads_for_pattern(
            Some(Subject::NamedNode(a.iri).as_ref()),
            None,
            None,
            None,
        )
        .next()
        .is_some();
    assert!(
        !exists,
        "refused verdict must not be persisted in the orchestration store"
    );
}

#[test]
fn stream_writer_accepts_rejected_with_citation() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let mut a = base_artifact(Verdict::Rejected);
    a.violates = vec![NamedNode::new("urn:tc:TC-029").expect("tc iri")];
    let quads = a.to_quads(orchestration_graph());
    let mutation = Mutation::insert(quads.iter().cloned());
    writer
        .commit(mutation)
        .expect("rejected verdict with one citation commits");
}
