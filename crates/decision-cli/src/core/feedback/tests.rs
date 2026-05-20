//! Unit tests for Feedback serialisation, SHACL validation, and reads.

use oxigraph::model::{NamedNode, Subject};
use oxigraph::store::Store;

use super::artifact::{Feedback, Severity};
use super::read::{get, list_by_class, list_by_target, list_open, FeedbackReadError};
use super::shacl::validate_quads;
use crate::core::vocab::orchestration_graph;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/act-1";
const FEEDBACK_IRI: &str = "urn:dec:feedback:test:1";

fn happy_artifact() -> Feedback {
    Feedback {
        iri: NamedNode::new(FEEDBACK_IRI).expect("feedback iri"),
        class: "gap".to_string(),
        severity: Severity::Warning,
        target_role: "spec-author".to_string(),
        evidence: "feature_spec FT-026 §Behaviour line 4 underspecifies the addressing flow"
            .to_string(),
        recommendation: Some("amend the spec with a worked example".to_string()),
        lifecycle_state: "produced".to_string(),
        source_session: NamedNode::new(SESSION_IRI).expect("session iri"),
        source_artifact: None,
        addressing_artifact: None,
        closed_by: None,
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
fn happy_feedback_passes_shacl() {
    let f = happy_artifact();
    let quads = f.to_quads(orchestration_graph());
    validate_quads(&quads).expect("happy feedback passes SHACL");
}

#[test]
fn missing_required_field_fails_shacl() {
    let mut quads = happy_artifact().to_quads(orchestration_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#evidence");
    let err = validate_quads(&quads).expect_err("missing evidence must fail SHACL");
    assert!(err.report.contains("dec:evidence"), "{}", err.report);
}

#[test]
fn empty_evidence_fails_shacl() {
    let mut a = happy_artifact();
    a.evidence = String::new();
    let quads = a.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("empty evidence must fail SHACL");
    assert!(err.report.contains("dec:evidence"), "{}", err.report);
}

#[test]
fn serialisation_includes_all_required_predicates() {
    let f = happy_artifact();
    let quads = f.to_quads(orchestration_graph());
    for p in [
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "https://decision-cli.dev/ns#feedbackClass",
        "https://decision-cli.dev/ns#lifecycleState",
        "https://decision-cli.dev/ns#targetRole",
        "https://decision-cli.dev/ns#evidence",
        "https://decision-cli.dev/ns#severity",
        "https://decision-cli.dev/ns#sourceSession",
        "https://decision-cli.dev/ns#inStream",
    ] {
        assert!(
            quads.iter().any(|q| q.predicate.as_str() == p),
            "expected predicate {p}"
        );
    }
}

#[test]
fn severity_parse_round_trips() {
    for s in [Severity::Info, Severity::Warning, Severity::Error] {
        assert_eq!(Severity::parse(s.as_str()), Some(s));
    }
    assert!(Severity::parse("critical").is_none());
}

#[test]
fn get_returns_not_found_for_missing_iri() {
    let store = Store::new().expect("store");
    let iri = NamedNode::new("urn:dec:feedback:missing").expect("iri");
    let err = get(&store, &iri).expect_err("missing feedback resolves to NotFound");
    assert!(matches!(err, FeedbackReadError::NotFound { .. }));
}

#[test]
fn get_round_trips_a_persisted_feedback() {
    let store = Store::new().expect("store");
    let f = happy_artifact();
    insert_quads(&store, &f.to_quads(orchestration_graph()));
    let back = get(&store, &f.iri).expect("get round trips");
    assert_eq!(back.class, f.class);
    assert_eq!(back.target_role, f.target_role);
    assert_eq!(back.evidence, f.evidence);
    assert_eq!(back.lifecycle_state, f.lifecycle_state);
    assert_eq!(back.source_session, f.source_session);
    assert_eq!(back.in_stream, f.in_stream);
    assert_eq!(back.recommendation.as_deref(), Some("amend the spec with a worked example"));
}

#[test]
fn list_open_filters_terminal_states() {
    let store = Store::new().expect("store");
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let open = happy_artifact();
    let mut closed = happy_artifact();
    closed.iri = NamedNode::new("urn:dec:feedback:test:2").expect("iri-2");
    closed.lifecycle_state = "closed".to_string();
    insert_quads(&store, &open.to_quads(orchestration_graph()));
    insert_quads(&store, &closed.to_quads(orchestration_graph()));
    let result = list_open(&store, &stream).expect("list_open");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].iri.as_str(), FEEDBACK_IRI);
}

#[test]
fn list_by_class_groups_by_feedback_class() {
    let store = Store::new().expect("store");
    let mut gap = happy_artifact();
    gap.class = "gap".to_string();
    let mut defect = happy_artifact();
    defect.iri = NamedNode::new("urn:dec:feedback:test:def").expect("iri");
    defect.class = "defect".to_string();
    insert_quads(&store, &gap.to_quads(orchestration_graph()));
    insert_quads(&store, &defect.to_quads(orchestration_graph()));
    let gaps = list_by_class(&store, "gap").expect("list_by_class gap");
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].class, "gap");
    let defects = list_by_class(&store, "defect").expect("list_by_class defect");
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].class, "defect");
}

#[test]
fn list_by_target_groups_by_role_id() {
    let store = Store::new().expect("store");
    let mut spec = happy_artifact();
    spec.target_role = "spec-author".to_string();
    let mut arch = happy_artifact();
    arch.iri = NamedNode::new("urn:dec:feedback:test:arch").expect("iri");
    arch.target_role = "architect".to_string();
    insert_quads(&store, &spec.to_quads(orchestration_graph()));
    insert_quads(&store, &arch.to_quads(orchestration_graph()));
    let architects = list_by_target(&store, "architect").expect("list_by_target architect");
    assert_eq!(architects.len(), 1);
    assert_eq!(architects[0].target_role, "architect");
}

fn insert_quads(store: &Store, quads: &[oxigraph::model::Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
    // Sanity: persisted via subject pattern.
    let subj = &quads[0].subject;
    if let Subject::NamedNode(_) = subj {
        // Nothing else needed — store::insert is synchronous.
    }
}
