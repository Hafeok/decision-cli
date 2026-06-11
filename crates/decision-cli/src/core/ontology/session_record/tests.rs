//! In-crate unit tests for the session_record SHACL validator.

use oxigraph::model::NamedNode;

use crate::core::ontology::role_binding::TriggerSignal;
use crate::core::vocab::orchestration_graph;

use super::types::SessionRecord;
use super::validate_quads;

fn s_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/session/{id}"))
}

fn cap_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/capability/{id}/v1"))
}

#[test]
fn root_session_passes_shacl() {
    let s = SessionRecord {
        iri: s_iri("root"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: cap_iri("code-writer"),
    };
    let quads = s.to_quads(orchestration_graph());
    validate_quads(&quads).expect("root session passes");
}

#[test]
fn escalated_session_without_reason_fails() {
    let s = SessionRecord {
        iri: s_iri("escalated"),
        escalated_from: Some(s_iri("root")),
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: cap_iri("code-writer"),
    };
    let mut quads = s.to_quads(orchestration_graph());
    // Mirror the inverse triple manually so only the missing-reason
    // constraint fires.
    quads.extend(s.escalated_to_quad(orchestration_graph()));
    let err = validate_quads(&quads).expect_err("missing reason must fail");
    assert!(err.report.contains("escalation_reason"), "{}", err.report);
}

#[test]
fn root_session_with_reason_fails() {
    let s = SessionRecord {
        iri: s_iri("rogue-root"),
        escalated_from: None,
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: cap_iri("code-writer"),
    };
    let quads = s.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("reason without from must fail");
    assert!(err.report.contains("escalation_reason"), "{}", err.report);
}
