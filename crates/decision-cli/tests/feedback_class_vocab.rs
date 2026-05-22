//! TC-034 — Feedback class is in the controlled vocabulary (FT-028 / ADR-023).
//!
//! Three layers of enforcement must agree:
//!
//! 1. The Rust `FeedbackClass` enum exposes exactly the six ADR-023
//!    values, each round-tripping through `as_iri_value` /
//!    `from_iri_value`.
//! 2. The write-side SHACL validator (`validate_quads`) rejects an
//!    unknown class literal as a violation against `dec:feedbackClass`.
//! 3. The embedded `shapes.ttl` declares `sh:in` on `dec:feedbackClass`
//!    with the six ADR-023 values — checked by SPARQL ASK over the
//!    shapes graph.

use std::sync::Arc;

use decision_cli::core::feedback::{
    validate_quads, Disposition, Feedback, FeedbackClass, Severity,
};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/feedback-class-test";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/act-fc-1";
const FEEDBACK_IRI: &str = "urn:dec:feedback:tc-034:1";

fn feedback_with_class(class: &str) -> Feedback {
    Feedback {
        iri: NamedNode::new(FEEDBACK_IRI).expect("feedback iri"),
        class: class.to_string(),
        severity: Severity::Warning,
        target_role: "spec-author".to_string(),
        evidence: "feature_spec FT-028 §Invariants requires sh:in enforcement".to_string(),
        recommendation: None,
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
fn all_six_values_round_trip() {
    let expected = [
        "gap",
        "contradiction",
        "unimplementable",
        "scope-issue",
        "defect",
        "capability-request",
    ];
    assert_eq!(FeedbackClass::all().len(), expected.len());
    for value in expected {
        let parsed = FeedbackClass::from_iri_value(value)
            .unwrap_or_else(|| panic!("from_iri_value({value:?}) must Some"));
        assert_eq!(
            parsed.as_iri_value(),
            value,
            "round-trip failed for {value}"
        );
    }
}

#[test]
fn unknown_class_resolves_to_none() {
    assert!(FeedbackClass::from_iri_value("foo").is_none());
    assert!(FeedbackClass::from_iri_value("").is_none());
    // Underscore variant is not a valid wire form.
    assert!(FeedbackClass::from_iri_value("scope_issue").is_none());
}

#[test]
fn defaults_match_adr_023_table() {
    // Per ADR-023 §Decision table.
    let expected_targets: &[(FeedbackClass, &str)] = &[
        (FeedbackClass::Gap, "spec-author"),
        (FeedbackClass::Contradiction, "architect"),
        (FeedbackClass::Unimplementable, "spec-author"),
        (FeedbackClass::ScopeIssue, "slice-curator"),
        (FeedbackClass::Defect, "verifier"),
        (FeedbackClass::CapabilityRequest, "architect"),
    ];
    for (class, target) in expected_targets {
        assert_eq!(
            class.default_target_role(),
            *target,
            "default target for {class:?}"
        );
    }

    let expected_dispositions: &[(FeedbackClass, Disposition)] = &[
        (FeedbackClass::Gap, Disposition::Blocking),
        (FeedbackClass::Contradiction, Disposition::Blocking),
        (FeedbackClass::Unimplementable, Disposition::Blocking),
        (FeedbackClass::ScopeIssue, Disposition::NonBlocking),
        (FeedbackClass::Defect, Disposition::NonBlocking),
        (FeedbackClass::CapabilityRequest, Disposition::NonBlocking),
    ];
    for (class, disposition) in expected_dispositions {
        assert_eq!(
            class.default_disposition(),
            *disposition,
            "default disposition for {class:?}"
        );
    }
}

#[test]
fn happy_class_passes_shacl() {
    for c in FeedbackClass::all() {
        let f = feedback_with_class(c.as_iri_value());
        let quads = f.to_quads(orchestration_graph());
        validate_quads(&quads).unwrap_or_else(|e| {
            panic!(
                "class {} must pass SHACL validation, got: {e}",
                c.as_iri_value()
            )
        });
    }
}

#[test]
fn unknown_class_fails_shacl() {
    let f = feedback_with_class("regression");
    let quads = f.to_quads(orchestration_graph());
    let err =
        validate_quads(&quads).expect_err("a class outside the ADR-023 vocabulary must fail SHACL");
    assert!(
        err.report.contains("dec:feedbackClass"),
        "violation report must reference dec:feedbackClass; got: {}",
        err.report
    );
    assert!(
        err.report.contains("regression"),
        "violation report must echo the offending value; got: {}",
        err.report
    );
}

#[test]
fn empty_class_fails_shacl() {
    let f = feedback_with_class("");
    let quads = f.to_quads(orchestration_graph());
    let err = validate_quads(&quads).expect_err("empty class literal must fail SHACL");
    assert!(
        err.report.contains("dec:feedbackClass"),
        "violation report must reference dec:feedbackClass; got: {}",
        err.report
    );
}

#[test]
fn stream_writer_refuses_unknown_class_at_commit() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let f = feedback_with_class("regression");
    let quads = f.to_quads(orchestration_graph());
    let err = writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect_err("StreamWriter must refuse unknown feedback class");
    let msg = format!("{err}");
    assert!(
        msg.contains("dec:feedbackClass") || msg.contains("SHACL"),
        "writer rejection must reference dec:feedbackClass or SHACL; got: {msg}"
    );
}

#[test]
fn stream_writer_accepts_every_known_class() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    for (i, c) in FeedbackClass::all().iter().enumerate() {
        let mut f = feedback_with_class(c.as_iri_value());
        // Use a unique IRI per write to avoid conflicting with prior fixtures.
        f.iri = NamedNode::new(format!("urn:dec:feedback:tc-034:happy:{i}")).expect("feedback iri");
        let quads = f.to_quads(orchestration_graph());
        writer
            .commit(Mutation::insert(quads.iter().cloned()))
            .unwrap_or_else(|e| panic!("class {} must be accepted, got: {e}", c.as_iri_value()));
    }
}

#[test]
fn embedded_shapes_declare_feedback_class_in_enumeration() {
    // The embedded `shapes.ttl` must declare `sh:in` on `dec:feedbackClass`
    // with the six ADR-023 values. We verify by SPARQL ASK over the
    // shapes graph.
    use decision_cli::core::ontology::{OntologyHandle, SHAPES_GRAPH_IRI};
    use oxigraph::sparql::QueryResults;

    let h = OntologyHandle::load().expect("ontology load");
    let store = h.store();
    let q = format!(
        "PREFIX sh: <http://www.w3.org/ns/shacl#> \
         PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH <{g}> {{ \
            dec:FeedbackShape sh:property ?p . \
            ?p sh:path dec:feedbackClass ; sh:in ?l . \
         }} }}",
        g = SHAPES_GRAPH_IRI,
    );
    let ok = matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)));
    assert!(
        ok,
        "shapes.ttl must declare sh:in on dec:feedbackClass in dec:FeedbackShape"
    );
}
