//! Unit tests for VerificationVerdict serialisation and SHACL validation.

use oxigraph::model::NamedNode;

use super::shacl::validate_quads;
use super::types::{Verdict, VerdictArtifact};
use crate::core::vocab::orchestration_graph;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/iv-1";

fn rationale_str() -> String {
    "approved — exit criteria all met per TC-029.".to_string()
}

fn artifact(verdict: Verdict) -> VerdictArtifact {
    VerdictArtifact {
        iri: NamedNode::new("urn:dec:verdict:test:1").expect("iri"),
        verdict,
        rationale: rationale_str(),
        violates: Vec::new(),
        amendment_guidance: None,
        generated_by: NamedNode::new(SESSION_IRI).expect("session iri"),
        used: vec![NamedNode::new("urn:dec:code-change:1").expect("cc iri")],
        in_stream: NamedNode::new(STREAM_IRI).expect("stream iri"),
    }
}

#[test]
fn approved_verdict_passes_shacl() {
    let a = artifact(Verdict::Approved);
    let quads = a.to_quads(orchestration_graph());
    validate_quads(&quads).expect("approved verdict passes SHACL");
}

#[test]
fn rejected_verdict_requires_violates() {
    let a = artifact(Verdict::Rejected);
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("rejected without violates must fail");
    assert!(err.report.contains("dec:violates"), "{}", err.report);
}

#[test]
fn rejected_verdict_with_violates_passes() {
    let mut a = artifact(Verdict::Rejected);
    a.violates = vec![NamedNode::new("urn:tc:TC-029").expect("tc iri")];
    let quads = a.to_quads(orchestration_graph());
    validate_quads(&quads).expect("rejected with citation passes");
}

#[test]
fn amendment_required_needs_guidance_and_violates() {
    let mut a = artifact(Verdict::AmendmentRequired);
    a.violates = vec![NamedNode::new("urn:tc:TC-001").expect("tc iri")];
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("missing guidance must fail");
    assert!(err.report.contains("amendmentGuidance"), "{}", err.report);

    a.amendment_guidance = Some("Add a SHACL check for the missing field.".to_string());
    let quads = a.to_quads(orchestration_graph());
    validate_quads(&quads).expect("amendment-required with guidance + violates passes");
}

#[test]
fn rationale_minimum_length_enforced() {
    let mut a = artifact(Verdict::Approved);
    a.rationale = "too short".to_string();
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("short rationale must fail");
    assert!(err.report.contains("rationale"), "{}", err.report);
}

#[test]
fn verdict_parse_round_trips() {
    for v in [Verdict::Approved, Verdict::Rejected, Verdict::AmendmentRequired] {
        assert_eq!(Verdict::parse(v.as_str()), Some(v));
    }
    assert!(Verdict::parse("approved-with-caveats").is_none());
}
