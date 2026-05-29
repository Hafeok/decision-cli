//! TC-056 — Each VerificationStep kind SHACL shape validates its required fields.
//!
//! Validates: FT-036 · ADR-028.
//! Spec: `.product/tests/TC-056-each-verificationstep-kind-shacl-shape-validates-i.md`
//!
//! Each acceptance criterion is one `#[test]` exercising a single
//! step kind against the `StreamWriter` chokepoint. A successful commit
//! confirms SHACL passed; a `SHACL violation` error confirms SHACL rejected
//! the mutation before the step quads were persisted.

use std::sync::Arc;

use decision_cli::core::ontology::verification_graph::io::{
    from_turtle_bytes, to_canonical_turtle,
};
use decision_cli::core::ontology::verification_graph::shacl::validate_quads as validate_graph_shacl;
use decision_cli::core::ontology::verification_graph::{
    step_iri_for, ArtifactRef, StepFields, StepKind, VerificationGraph, VerificationStep,
};
use decision_cli::vocab::{verify_graph_named_graph, IRI_DEC_COMMAND};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-056";

fn writer() -> StreamWriter {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer")
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn graph_id() -> &'static str {
    "VG-TC-056"
}

fn ft_001() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/feature/FT-001")
}

fn env_iri() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli")
}

fn graph_with(step: VerificationStep) -> VerificationGraph {
    VerificationGraph::new(graph_id(), ArtifactRef(ft_001()), env_iri(), vec![step])
}

#[test]
fn shell_command_pass_and_fail() {
    let w = writer();
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::ShellCommand {
            command: "ls".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("shell-command commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::ShellCommand {
            command: "ls".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    );
    let mut g = graph_with(bad);
    // Strip dec:command quad: a malformed instance.
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != IRI_DEC_COMMAND);
    // We need a fresh writer to avoid the previous successful commit
    // polluting the store with conflicting blank-node names — but the
    // SHACL check only looks at the inserted quad set, so reuse is safe.
    let err = commit_quads(&w, quads).expect_err("missing command must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#command"), "{err}");
    let _ = &mut g;
}

#[test]
fn sparql_assertion_pass_and_fail() {
    let w = writer();
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::SparqlAssertion {
            target: ".dec/store".to_string(),
            query: "SELECT ?s WHERE { ?s ?p ?o }".to_string(),
            expect_rows: Some(1),
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("sparql-assertion commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::SparqlAssertion {
            target: ".dec/store".to_string(),
            query: String::new(),
            expect_rows: None,
        },
    );
    let g = graph_with(bad);
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#query");
    let err = commit_quads(&w, quads).expect_err("missing query must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#query"), "{err}");
}

#[test]
fn file_assertion_pass_and_fail() {
    let w = writer();
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::FileAssertion {
            path: ".dec/store/orchestration.nq".to_string(),
            expect_hash: None,
            expect_content: None,
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("file-assertion commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::FileAssertion {
            path: ".dec/store/orchestration.nq".to_string(),
            expect_hash: None,
            expect_content: None,
        },
    );
    let g = graph_with(bad);
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#path");
    let err = commit_quads(&w, quads).expect_err("missing path must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#path"), "{err}");
}

#[test]
fn http_request_pass_and_fail() {
    let w = writer();
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            expect_status: Some(200),
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("http-request commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            expect_status: None,
        },
    );
    let g = graph_with(bad);
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#url");
    let err = commit_quads(&w, quads).expect_err("missing url must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#url"), "{err}");
}

#[test]
fn wait_for_pass_and_fail() {
    let w = writer();
    let cond = NamedNode::new_unchecked("https://decision-cli.dev/ns/step/VG-TC-056/99");
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::WaitFor {
            condition: cond.clone(),
            timeout: "PT10S".to_string(),
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("wait-for commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::WaitFor {
            condition: cond,
            timeout: "PT10S".to_string(),
        },
    );
    let g = graph_with(bad);
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#timeout");
    let err = commit_quads(&w, quads).expect_err("missing timeout must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#timeout"), "{err}");
}

#[test]
fn capture_pass_and_fail() {
    let w = writer();
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::Capture {
            from_step: None,
            bind_as: "manifest_sha".to_string(),
        },
    );
    let g = graph_with(good);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("capture commits");

    let bad = VerificationStep::new(
        graph_id(),
        0,
        StepFields::Capture {
            from_step: None,
            bind_as: "manifest_sha".to_string(),
        },
    );
    let g = graph_with(bad);
    let mut quads = g.to_quads(verify_graph_named_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#bindAs");
    let err = commit_quads(&w, quads).expect_err("missing bindAs must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("ns#bindAs"), "{err}");
}

#[test]
fn unknown_step_type_is_rejected_by_shacl() {
    let good = VerificationStep::new(
        graph_id(),
        0,
        StepFields::ShellCommand {
            command: "ls".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    );
    let g = graph_with(good);
    let mut quads = g.to_quads(verify_graph_named_graph());
    // Replace the stepType literal with "rocketship".
    for q in quads.iter_mut() {
        if q.predicate.as_str() == "https://decision-cli.dev/ns#stepType" {
            if matches!(q.object, oxigraph::model::Term::Literal(_)) {
                q.object = oxigraph::model::Literal::new_simple_literal("rocketship").into();
            }
        }
    }
    let err = validate_graph_shacl(&quads).expect_err("unknown stepType must fail SHACL");
    assert!(err.report.contains("stepType"), "{}", err.report);
    assert!(err.report.contains("rocketship"), "{}", err.report);
}

#[test]
fn unknown_step_type_is_rejected_at_parse_time() {
    use decision_cli::core::ontology::verification_graph::io::GraphIoError;
    // Build a graph with a "rocketship" stepType in the on-disk Turtle.
    let good_step = VerificationStep::new(
        graph_id(),
        0,
        StepFields::ShellCommand {
            command: "ls".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    );
    let g = graph_with(good_step);
    let ttl = to_canonical_turtle(&g).replace("\"shell-command\"", "\"rocketship\"");
    let err = from_turtle_bytes(std::path::Path::new("test.ttl"), ttl.as_bytes())
        .expect_err("unknown stepType must surface UnknownStepKind");
    match err {
        GraphIoError::UnknownStepKind { source, .. } => {
            assert_eq!(source.value, "rocketship");
        }
        other => panic!("expected UnknownStepKind, got: {other:?}"),
    }
}

#[test]
fn placeholder_command_is_preserved_verbatim() {
    let w = writer();
    let placeholder = "dec init ${prior_stream}";
    let step = VerificationStep::new(
        graph_id(),
        0,
        StepFields::ShellCommand {
            command: placeholder.to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    let g = graph_with(step.clone());
    // Commit succeeds — placeholder is just a literal.
    commit_quads(&w, g.to_quads(verify_graph_named_graph()))
        .expect("placeholder commits as literal");

    // On-disk Turtle preserves it verbatim.
    let ttl = to_canonical_turtle(&g);
    assert!(
        ttl.contains("${prior_stream}"),
        "placeholder must appear verbatim in Turtle:\n{ttl}"
    );

    // Round-trip preserves it.
    let parsed = from_turtle_bytes(std::path::Path::new("test.ttl"), ttl.as_bytes())
        .expect("round-trip parse");
    let parsed_command = match &parsed.steps[0].fields {
        StepFields::ShellCommand { command, .. } => command.as_str(),
        other => panic!("expected shell-command, got: {other:?}"),
    };
    assert_eq!(parsed_command, placeholder);
}

#[test]
fn step_iri_is_deterministic() {
    let a = step_iri_for("VG-001", 3);
    let b = step_iri_for("VG-001", 3);
    assert_eq!(a, b);
    assert!(a.as_str().ends_with("/VG-001/3"));
    let c = step_iri_for("VG-001", 4);
    assert_ne!(a, c);
    assert!(c.as_str().ends_with("/VG-001/4"));
}

#[test]
fn step_kind_round_trips_to_str_and_back() {
    for kind in [
        StepKind::ShellCommand,
        StepKind::SparqlAssertion,
        StepKind::FileAssertion,
        StepKind::HttpRequest,
        StepKind::WaitFor,
        StepKind::Capture,
    ] {
        assert_eq!(StepKind::parse(kind.as_str()), Some(kind));
    }
    assert!(StepKind::parse("rocketship").is_none());
}
