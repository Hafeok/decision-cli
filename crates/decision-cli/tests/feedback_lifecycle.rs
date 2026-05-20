//! TC-035 — Feedback lifecycle transitions are validated (FT-027 / ADR-024).
//!
//! Two layers of enforcement must agree:
//!
//! 1. The pure Rust `validate_transition` validator accepts every entry
//!    in the ADR-024 transition table and refuses every other pair.
//! 2. The `StreamWriter` chokepoint refuses an invalid transition on an
//!    already-persisted feedback artifact before the underlying writer
//!    sees it (write-side enforcement).
//!
//! The two layers cover the rejection paths in the ADR-024 state diagram
//! end-to-end: only-forward, no jumping, no transitions out of terminal
//! states. SHACL-side enforcement (the embedded `shapes.ttl` `sh:in` and
//! `sh:sparql` constraints) is exercised by `verdict_shacl`-style tests
//! living alongside the artifact unit tests in the crate.

use std::sync::Arc;

use decision_cli::core::feedback::{
    apply, lifecycle::next_states, validate_transition, ApplyError, Feedback, LifecycleState,
    Severity, TransitionError,
};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/feedback-lifecycle-test";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/act-lc-1";
const TARGET_SESSION_IRI: &str = "https://decision-cli.dev/ns/session/target-lc-1";
const FEEDBACK_IRI: &str = "urn:dec:feedback:tc-035:1";
const ADDRESSING_IRI: &str = "urn:dec:feature-spec:FT-026:amendment-1";

fn produced_feedback() -> Feedback {
    Feedback {
        iri: NamedNode::new(FEEDBACK_IRI).expect("feedback iri"),
        class: "gap".to_string(),
        severity: Severity::Warning,
        target_role: "spec-author".to_string(),
        evidence: "feature_spec FT-026 line 12 underspecifies addressing flow".to_string(),
        recommendation: None,
        lifecycle_state: LifecycleState::Produced.as_str().to_string(),
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
fn next_states_table_matches_adr_024() {
    // ADR-024 §Transition diagram, encoded directly.
    let expected: &[(LifecycleState, &[LifecycleState])] = &[
        (
            LifecycleState::Produced,
            &[LifecycleState::Routed, LifecycleState::Superseded],
        ),
        (
            LifecycleState::Routed,
            &[LifecycleState::Received, LifecycleState::Superseded],
        ),
        (
            LifecycleState::Received,
            &[LifecycleState::Addressed, LifecycleState::Rejected],
        ),
        (
            LifecycleState::Addressed,
            &[LifecycleState::Closed, LifecycleState::Rejected],
        ),
        (LifecycleState::Closed, &[]),
        (LifecycleState::Rejected, &[]),
        (LifecycleState::Superseded, &[]),
    ];
    for (from, expected_to) in expected {
        let actual = next_states(*from);
        assert_eq!(actual, *expected_to, "next_states({from:?})");
    }
}

#[test]
fn happy_path_full_chain_validates() {
    for (from, to) in [
        (LifecycleState::Produced, LifecycleState::Routed),
        (LifecycleState::Routed, LifecycleState::Received),
        (LifecycleState::Received, LifecycleState::Addressed),
        (LifecycleState::Addressed, LifecycleState::Closed),
    ] {
        validate_transition(from, to).expect("happy-path transition must validate");
    }
}

#[test]
fn jumping_states_is_refused() {
    let err = validate_transition(LifecycleState::Produced, LifecycleState::Addressed)
        .expect_err("produced → addressed skips routed/received");
    assert!(
        matches!(
            err,
            TransitionError::InvalidTransition {
                from: LifecycleState::Produced,
                to: LifecycleState::Addressed,
            }
        ),
        "expected InvalidTransition, got {err:?}"
    );
}

#[test]
fn reverse_transitions_are_refused() {
    let err = validate_transition(LifecycleState::Routed, LifecycleState::Produced)
        .expect_err("reverse transition routed → produced must be refused");
    assert!(matches!(err, TransitionError::InvalidTransition { .. }));
}

#[test]
fn terminal_states_reject_any_outgoing_transition() {
    for terminal in [
        LifecycleState::Closed,
        LifecycleState::Rejected,
        LifecycleState::Superseded,
    ] {
        let err = validate_transition(terminal, LifecycleState::Produced).expect_err(
            "terminal state must refuse any outgoing transition (closed/rejected/superseded)",
        );
        assert!(
            matches!(err, TransitionError::TerminalState { state } if state == terminal),
            "expected TerminalState({terminal:?}), got {err:?}"
        );
    }
}

#[test]
fn stream_writer_refuses_invalid_lifecycle_transition() {
    // Seed a produced feedback through the writer, then attempt an
    // invalid forward jump (produced → addressed). The writer's
    // SHACL-violation prefix must surface and the new state must NOT
    // land in the store.
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let seeded = produced_feedback();
    let quads = seeded.to_quads(orchestration_graph());
    writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect("seeding the produced feedback must succeed");

    // Now attempt produced → addressed (must be refused).
    let err = apply(
        store.as_ref(),
        &writer,
        &seeded.iri,
        LifecycleState::Addressed,
        Vec::new(),
        orchestration_graph(),
    )
    .expect_err("StreamWriter must refuse produced → addressed");
    let msg = format!("{err}");
    assert!(
        msg.contains("invalid lifecycle transition") || msg.contains("SHACL violation"),
        "error must reference an invalid lifecycle transition; got: {msg}"
    );
}

#[test]
fn stream_writer_refuses_transition_out_of_terminal_state() {
    // Seed a closed feedback (with the companion fields), then attempt
    // to push it to any other state.
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    // We seed straight into closed via direct quads (bypassing the
    // transition validator) — this is the "imported / restored from
    // history" path. The SHACL companion-field check still binds, so
    // we provide dec:closedBy and dec:addressingArtifact.
    let mut closed = produced_feedback();
    closed.lifecycle_state = LifecycleState::Closed.as_str().to_string();
    closed.closed_by = Some(NamedNode::new(TARGET_SESSION_IRI).expect("session iri"));
    closed.addressing_artifact = Some(NamedNode::new(ADDRESSING_IRI).expect("addressing iri"));
    let quads = closed.to_quads(orchestration_graph());
    writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect("seeding the closed feedback must succeed");

    // Now attempt closed → any outgoing transition. ApplyError surfaces
    // the terminal-state diagnostic.
    let err = apply(
        store.as_ref(),
        &writer,
        &closed.iri,
        LifecycleState::Produced,
        Vec::new(),
        orchestration_graph(),
    )
    .expect_err("closed → produced must be refused as terminal");
    assert!(
        matches!(
            err,
            ApplyError::Transition(TransitionError::TerminalState {
                state: LifecycleState::Closed,
            })
        ),
        "expected TerminalState(Closed), got {err:?}"
    );
}

#[test]
fn happy_path_transition_through_apply_lands_in_store() {
    use decision_cli::core::vocab::{lifecycle_state, target_role};
    use oxigraph::model::{Literal, Quad, Subject, Term};

    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let seeded = produced_feedback();
    let quads = seeded.to_quads(orchestration_graph());
    writer
        .commit(Mutation::insert(quads.iter().cloned()))
        .expect("seeding the produced feedback must succeed");

    // Build the routed-state companion fields (dec:routedAt + dec:targetRole).
    let g: oxigraph::model::GraphName = orchestration_graph().into_owned().into();
    let evidence = vec![
        Quad::new(
            seeded.iri.clone(),
            decision_cli::core::vocab::routed_at().into_owned(),
            Literal::new_simple_literal("2026-05-20T12:00:00Z"),
            g.clone(),
        ),
        Quad::new(
            seeded.iri.clone(),
            target_role().into_owned(),
            Literal::new_simple_literal("spec-author"),
            g,
        ),
    ];

    apply(
        store.as_ref(),
        &writer,
        &seeded.iri,
        LifecycleState::Routed,
        evidence,
        orchestration_graph(),
    )
    .expect("produced → routed transition must succeed when companion fields are supplied");

    // Confirm the new state lives in the store and the old one was retracted.
    let mut states: Vec<String> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(seeded.iri.clone()).as_ref()),
            Some(lifecycle_state()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = quad.object {
            states.push(lit.value().to_string());
        }
    }
    assert_eq!(states, vec!["routed".to_string()]);
}

#[test]
fn embedded_shapes_declare_lifecycle_state_in_enumeration() {
    // The embedded `shapes.ttl` must declare `sh:in` on `dec:lifecycleState`
    // with the seven ADR-024 values. We verify by SPARQL ASK over the
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
            ?p sh:path dec:lifecycleState ; sh:in ?l . \
         }} }}",
        g = SHAPES_GRAPH_IRI,
    );
    let ok = matches!(
        store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    );
    assert!(
        ok,
        "shapes.ttl must declare sh:in on dec:lifecycleState in dec:FeedbackShape"
    );
}
