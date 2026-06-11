//! Unit tests for the FT-031 worker feedback ingest chokepoint.

use std::sync::Arc;

use oxigraph::model::{GraphNameRef, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use super::{
    apply, parse_records, FeedbackApplyError, FeedbackEmission, ParseRecordError,
    FEEDBACK_RECORD_SENTINEL,
};
use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::{value_stream_class, IRI_DEC_GRAPH_ORCHESTRATION};

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/act-test";

fn open_writer() -> StreamWriter {
    let store = Arc::new(Store::new().expect("oxigraph in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let graph = NamedNodeRef::new(IRI_DEC_GRAPH_ORCHESTRATION).expect("graph iri");
    let cls = value_stream_class();
    let rdf_type = NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let quad = Quad::new(
        stream.clone(),
        rdf_type,
        cls.into_owned(),
        GraphNameRef::NamedNode(graph).into_owned(),
    );
    store
        .transaction(|mut tx| tx.insert(quad.as_ref()).map(|_| ()))
        .expect("seed value stream");
    StreamWriter::open(store, stream).expect("open stream writer")
}

fn sentinel_line(payload: &str) -> String {
    format!("{FEEDBACK_RECORD_SENTINEL} {payload}\n")
}

#[test]
fn parse_records_ignores_non_sentinel_lines() {
    let stream = "ordinary worker log line\n{\"normal\": \"WorkerResponse\"}\n";
    let (ok, errs) = parse_records(stream);
    assert!(ok.is_empty());
    assert!(errs.is_empty());
}

#[test]
fn parse_records_extracts_single_sentinel_record() {
    let payload = serde_json::json!({
        "feedback_class": "gap",
        "severity": "medium",
        "evidence": "feature_spec FT-031 line 42 underspecifies the routing fallback",
        "recommendation": "amend FT-031",
    })
    .to_string();
    let stream = format!(
        "chatter line\n{}{}",
        sentinel_line(&payload),
        "trailing log\n"
    );
    let (ok, errs) = parse_records(&stream);
    assert_eq!(ok.len(), 1);
    assert!(errs.is_empty());
    assert_eq!(ok[0].feedback_class, "gap");
    assert_eq!(ok[0].severity, "medium");
    assert!(ok[0].evidence.contains("underspecifies"));
}

#[test]
fn parse_records_collects_malformed_records_separately() {
    let good = serde_json::json!({
        "feedback_class": "contradiction",
        "severity": "high",
        "evidence": "ADR-005 vs FT-026 disagree on dec:inStream cardinality",
    })
    .to_string();
    let stream = format!(
        "{}{}",
        sentinel_line(&good),
        sentinel_line("{ not valid json ]"),
    );
    let (ok, errs) = parse_records(&stream);
    assert_eq!(ok.len(), 1);
    assert_eq!(errs.len(), 1);
    match &errs[0] {
        ParseRecordError::Malformed { line_no, .. } => assert_eq!(*line_no, 2),
    }
}

#[test]
fn apply_writes_one_feedback_artifact_in_produced_state() {
    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    let emission = FeedbackEmission {
        feedback_class: "gap".to_string(),
        severity: "medium".to_string(),
        evidence: "the bundle for FT-031 lacks the routing fallback semantics".to_string(),
        recommendation: Some("add a routing-table seed".to_string()),
        target_role_override: None,
        blocking: None,
        disposition_rationale: None,
        source_session: String::new(),
    };
    let iri = apply(&writer, &emission, Some(&session)).expect("apply");

    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH ?g {{ \
           <{iri}> a dec:Feedback ; dec:lifecycleState \"produced\" ; \
                   dec:feedbackClass \"gap\" ; \
                   dec:sourceSession <{SESSION_IRI}> ; \
                   dec:targetRole \"spec-author\" ; \
                   dec:inStream <{STREAM_IRI}> . }} }}",
        iri = iri.as_str(),
    );
    let store = writer.inner().store();
    let oxigraph::sparql::QueryResults::Boolean(b) = store.query(q.as_str()).expect("ask") else {
        panic!("expected boolean")
    };
    assert!(b, "feedback artifact must satisfy the ASK pattern");
}

#[test]
fn apply_uses_target_role_override_when_provided() {
    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    let emission = FeedbackEmission {
        feedback_class: "gap".to_string(),
        severity: "medium".to_string(),
        evidence: "an override-target case for the apply path test ≥20 chars".to_string(),
        recommendation: None,
        target_role_override: Some("architect".to_string()),
        blocking: None,
        disposition_rationale: None,
        source_session: String::new(),
    };
    let iri = apply(&writer, &emission, Some(&session)).expect("apply");
    let store = writer.inner().store();
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH ?g {{ <{iri}> dec:targetRole \"architect\" . }} }}",
        iri = iri.as_str(),
    );
    let oxigraph::sparql::QueryResults::Boolean(b) = store.query(q.as_str()).expect("ask") else {
        panic!("expected boolean")
    };
    assert!(b, "target role override must be persisted");
}

#[test]
fn apply_records_disposition_override_when_blocking_differs_from_default() {
    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    // `defect` class defaults to non-blocking; the worker overrides to blocking.
    let emission = FeedbackEmission {
        feedback_class: "defect".to_string(),
        severity: "high".to_string(),
        evidence: "shipping this defect breaks downstream dispatches; pausing".to_string(),
        recommendation: None,
        target_role_override: None,
        blocking: Some(true),
        disposition_rationale: Some(
            "downstream dispatches depend on the routing record I would have shipped".to_string(),
        ),
        source_session: String::new(),
    };
    let iri = apply(&writer, &emission, Some(&session)).expect("apply");
    let store = writer.inner().store();
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH ?g {{ <{iri}> dec:dispositionOverride \"blocking\" ; \
                                    dec:dispositionRationale ?r . }} }}",
        iri = iri.as_str(),
    );
    let oxigraph::sparql::QueryResults::Boolean(b) = store.query(q.as_str()).expect("ask") else {
        panic!("expected boolean")
    };
    assert!(b, "disposition override + rationale must be persisted");
}

#[test]
fn apply_does_not_record_override_when_blocking_matches_class_default() {
    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    // `gap` defaults to blocking; re-confirming with blocking=true must NOT
    // register an override (the artifact stays under the class default).
    let emission = FeedbackEmission {
        feedback_class: "gap".to_string(),
        severity: "high".to_string(),
        evidence: "the bundle lacks routing fallback semantics required to proceed".to_string(),
        recommendation: None,
        target_role_override: None,
        blocking: Some(true),
        disposition_rationale: None,
        source_session: String::new(),
    };
    let iri = apply(&writer, &emission, Some(&session)).expect("apply");
    let store = writer.inner().store();
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH ?g {{ <{iri}> dec:dispositionOverride ?o . }} }}",
        iri = iri.as_str(),
    );
    let oxigraph::sparql::QueryResults::Boolean(b) = store.query(q.as_str()).expect("ask") else {
        panic!("expected boolean")
    };
    assert!(
        !b,
        "no override should be recorded when blocking matches default"
    );
}

#[test]
fn apply_returns_no_active_session_when_unspecified() {
    let writer = open_writer();
    let emission = FeedbackEmission {
        feedback_class: "gap".to_string(),
        severity: "medium".to_string(),
        evidence: "missing source_session must error out instead of writing".to_string(),
        recommendation: None,
        target_role_override: None,
        blocking: None,
        disposition_rationale: None,
        source_session: String::new(),
    };
    let err = apply(&writer, &emission, None).expect_err("must require source session");
    assert!(matches!(err, FeedbackApplyError::NoActiveSession));
}

#[test]
fn parse_then_apply_end_to_end_round_trip() {
    let payload = serde_json::json!({
        "feedback_class": "scope-issue",
        "severity": "low",
        "evidence": "feature_spec drifts beyond slice-3 bounds, surfacing in flight",
        "target_role_override": "slice-curator",
    })
    .to_string();
    let stream = sentinel_line(&payload);
    let (records, errs) = parse_records(&stream);
    assert_eq!(records.len(), 1);
    assert!(errs.is_empty());

    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    let iri = apply(&writer, &records[0], Some(&session)).expect("apply");

    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ GRAPH ?g {{ \
           <{iri}> a dec:Feedback ; dec:lifecycleState \"produced\" ; \
                   dec:feedbackClass \"scope-issue\" ; \
                   dec:targetRole \"slice-curator\" . }} }}",
        iri = iri.as_str(),
    );
    let oxigraph::sparql::QueryResults::Boolean(b) =
        writer.inner().store().query(q.as_str()).expect("ask")
    else {
        panic!("expected boolean")
    };
    assert!(b, "end-to-end record must land in produced state");
}

#[test]
fn unknown_class_still_writes_with_empty_target_role_default() {
    // Per ADR-022 error handling: unknown classes are written anyway;
    // SHACL catches them at the write boundary. Verify the parser
    // does not blow up; the applier surfaces the SHACL rejection.
    let emission = FeedbackEmission {
        feedback_class: "definitely-not-a-real-class".to_string(),
        severity: "medium".to_string(),
        evidence: "an out-of-vocab class to test the parser's defensive path".to_string(),
        recommendation: None,
        target_role_override: None,
        blocking: None,
        disposition_rationale: None,
        source_session: String::new(),
    };
    let writer = open_writer();
    let session = NamedNode::new(SESSION_IRI).expect("session iri");
    let result = apply(&writer, &emission, Some(&session));
    // Either the writer rejects (SHACL) or it commits with an empty
    // target role + invented class — both are acceptable per the
    // error-handling rules in FT-031 §Error handling.
    if let Err(e) = result {
        assert!(matches!(e, FeedbackApplyError::Commit(_)));
    }
}
