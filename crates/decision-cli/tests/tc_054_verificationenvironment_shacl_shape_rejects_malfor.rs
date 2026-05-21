//! TC-054 — VerificationEnvironment SHACL shape rejects malformed env.
//!
//! Validates: FT-035 · ADR-028.
//! Spec: `.product/tests/TC-054-verificationenvironment-shacl-shape-rejects-malfor.md`
//!
//! Each acceptance criterion is one `#[test]` exercising a single
//! invariant against the `StreamWriter` chokepoint with the embedded
//! ontology bundle loaded. The harness writes into an in-memory store;
//! a successful commit confirms SHACL passed, a `SHACL violation` error
//! confirms SHACL rejected the mutation before any quads were persisted.

use std::sync::Arc;

use decision_cli::core::ontology::verification_env::{
    SafetyClass, VerificationEnvironment,
};
use decision_cli::vocab::verify_env_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-054";

fn ephemeral_env() -> VerificationEnvironment {
    VerificationEnvironment {
        id: "ENV-001-ephemeral-cli".to_string(),
        env_type: "ephemeral-tempdir".to_string(),
        setup: Some("mkdir -p $TMPDIR && cd $TMPDIR".to_string()),
        teardown: Some("rm -rf $TMPDIR".to_string()),
        allowed_ops: vec![
            "shell".to_string(),
            "filesystem".to_string(),
            "sparql-local".to_string(),
        ],
        safety_class: SafetyClass::Isolated,
        endpoint: None,
    }
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

#[test]
fn well_formed_ephemeral_env_commits() {
    let (store, w) = writer();
    let env = ephemeral_env();
    let quads = env.to_quads(verify_env_graph());
    commit_quads(&w, quads).expect("well-formed ephemeral env commits cleanly");
    // The env now lives in the store under its canonical IRI.
    let iri = env.iri();
    let exists = store
        .quads_for_pattern(Some(Subject::NamedNode(iri).as_ref()), None, None, None)
        .next()
        .is_some();
    assert!(exists, "env must be persisted after a successful commit");
}

#[test]
fn missing_env_type_is_rejected() {
    let (store, w) = writer();
    let env = ephemeral_env();
    let mut quads = env.to_quads(verify_env_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#envType");
    let err = commit_quads(&w, quads.clone()).expect_err("missing envType must fail");
    assert!(
        err.contains("SHACL violation"),
        "error should be tagged as a SHACL violation; got: {err}"
    );
    assert!(
        err.contains("envType"),
        "detail string must name envType; got: {err}"
    );
    // No persistence on failure.
    let exists = store
        .quads_for_pattern(Some(Subject::NamedNode(env.iri()).as_ref()), None, None, None)
        .next()
        .is_some();
    assert!(!exists, "env quads must NOT persist after SHACL failure");
}

#[test]
fn unknown_safety_class_is_rejected() {
    let (_store, w) = writer();
    let env = ephemeral_env();
    let mut quads = env.to_quads(verify_env_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == "https://decision-cli.dev/ns#safetyClass" {
            if matches!(q.object, Term::Literal(_)) {
                q.object = oxigraph::model::Literal::new_simple_literal("yolo").into();
            }
        }
    }
    let err = commit_quads(&w, quads).expect_err("unknown safetyClass must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("safetyClass"), "{err}");
    // Detail must list the accepted vocabulary values.
    for expected in ["isolated", "shared-non-destructive", "production-readonly"] {
        assert!(
            err.contains(expected),
            "detail string must list {expected} among accepted values; got: {err}"
        );
    }
}

#[test]
fn empty_allowed_ops_is_rejected() {
    let (_store, w) = writer();
    let env = VerificationEnvironment {
        allowed_ops: Vec::new(),
        ..ephemeral_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = commit_quads(&w, quads).expect_err("empty allowedOps must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("allowedOps"), "{err}");
}

#[test]
fn remote_env_without_endpoint_is_rejected() {
    let (_store, w) = writer();
    let env = VerificationEnvironment {
        env_type: "remote-http".to_string(),
        endpoint: None,
        ..ephemeral_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = commit_quads(&w, quads).expect_err("remote env without endpoint must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("endpoint"), "{err}");
}

#[test]
fn remote_env_with_endpoint_commits() {
    let (_store, w) = writer();
    let env = VerificationEnvironment {
        env_type: "remote-http".to_string(),
        endpoint: Some("https://dev.decision-cli.dev".to_string()),
        ..ephemeral_env()
    };
    let quads = env.to_quads(verify_env_graph());
    commit_quads(&w, quads).expect("remote env with endpoint commits cleanly");
}

#[test]
fn local_env_with_endpoint_is_rejected() {
    let (_store, w) = writer();
    let env = VerificationEnvironment {
        env_type: "ephemeral-tempdir".to_string(),
        endpoint: Some("https://example.com".to_string()),
        ..ephemeral_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = commit_quads(&w, quads).expect_err("local env with endpoint must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("endpoint"), "{err}");
}

#[test]
fn embedded_shapes_declare_verification_environment_shape() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    let target = NamedNode::new("https://decision-cli.dev/ns#VerificationEnvironment")
        .expect("class iri");
    let mut has_shape = false;
    for q in h.shapes_graph() {
        if q.predicate.as_str() == "http://www.w3.org/ns/shacl#targetClass" {
            if let Term::NamedNode(n) = &q.object {
                if n == &target {
                    has_shape = true;
                    break;
                }
            }
        }
    }
    assert!(
        has_shape,
        "shapes.ttl must declare a sh:NodeShape with sh:targetClass dec:VerificationEnvironment"
    );
}
