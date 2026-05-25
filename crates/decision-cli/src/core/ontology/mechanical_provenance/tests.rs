//! Unit tests for FT-069 / ADR-038 mechanical-provenance materialiser + validator.

use oxigraph::model::{NamedNode, NamedNodeRef};

use crate::core::vocab::IRI_DEC_GRAPH_ORCHESTRATION;

use super::{materialise_quads, validate_quads, validate_subject, SessionAttribution};

const TS: &str = "2026-05-25T12:00:00Z";

fn artifact() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/artifact/x")
}

fn session() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1")
}

fn agent() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/agent/a1")
}

fn graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION)
}

#[test]
fn materialise_emits_three_predicates_for_single_agent() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let quads = materialise_quads(&a, &attribution, graph());
    assert_eq!(quads.len(), 3, "{quads:?}");
    let preds: Vec<&str> = quads.iter().map(|q| q.predicate.as_str()).collect();
    assert!(preds.contains(&"http://www.w3.org/ns/prov#wasGeneratedBy"));
    assert!(preds.contains(&"http://www.w3.org/ns/prov#wasAttributedTo"));
    assert!(preds.contains(&"http://www.w3.org/ns/prov#generatedAtTime"));
}

#[test]
fn materialise_with_two_attributed_agents_emits_two_attributed_to_triples() {
    let a = artifact();
    let attribution = SessionAttribution::new(
        session(),
        vec![
            agent(),
            NamedNode::new_unchecked("https://decision-cli.dev/ns/agent/a2"),
        ],
        TS,
    );
    let quads = materialise_quads(&a, &attribution, graph());
    let attributed = quads
        .iter()
        .filter(|q| q.predicate.as_str() == "http://www.w3.org/ns/prov#wasAttributedTo")
        .count();
    assert_eq!(attributed, 2);
}

#[test]
fn well_formed_subject_passes_validator() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let quads = materialise_quads(&a, &attribution, graph());
    validate_quads(&quads, &[a]).expect("well-formed mechanical block must validate");
}

#[test]
fn missing_was_generated_by_is_rejected() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let mut quads = materialise_quads(&a, &attribution, graph());
    quads.retain(|q| q.predicate.as_str() != "http://www.w3.org/ns/prov#wasGeneratedBy");
    let err = validate_quads(&quads, &[a]).expect_err("missing wasGeneratedBy must fail");
    assert!(err.report.contains("prov#wasGeneratedBy"), "{}", err.report);
    assert!(err.report.contains("missing required"), "{}", err.report);
}

#[test]
fn missing_was_attributed_to_is_rejected() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![], TS);
    let quads = materialise_quads(&a, &attribution, graph());
    let err = validate_quads(&quads, &[a]).expect_err("empty attributed_to must fail");
    assert!(err.report.contains("prov#wasAttributedTo"), "{}", err.report);
}

#[test]
fn missing_generated_at_time_is_rejected() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let mut quads = materialise_quads(&a, &attribution, graph());
    quads.retain(|q| q.predicate.as_str() != "http://www.w3.org/ns/prov#generatedAtTime");
    let err = validate_quads(&quads, &[a]).expect_err("missing generatedAtTime must fail");
    assert!(
        err.report.contains("prov#generatedAtTime"),
        "{}",
        err.report
    );
}

#[test]
fn malformed_generated_at_time_lexical_is_rejected() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], "not-a-date");
    let quads = materialise_quads(&a, &attribution, graph());
    let err = validate_quads(&quads, &[a]).expect_err("malformed datetime must fail");
    assert!(
        err.report.contains("prov#generatedAtTime"),
        "{}",
        err.report
    );
    assert!(err.report.contains("not-a-date"), "{}", err.report);
}

#[test]
fn validate_subject_returns_empty_for_conformant_block() {
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let quads = materialise_quads(&a, &attribution, graph());
    let violations = validate_subject(&quads, &a);
    assert!(violations.is_empty(), "{violations:?}");
}

#[test]
fn validator_reports_multiple_violations_when_all_three_missing() {
    let a = artifact();
    let violations = validate_subject(&[], &a);
    assert_eq!(violations.len(), 3);
}
