//! TC-057 — VerificationGraph round-trips Turtle ↔ store preserving step order.
//!
//! Validates: FT-036 · ADR-028.
//! Spec: `.product/tests/TC-057-verificationgraph-round-trips-turtle-store-preserv.md`

use std::path::Path;

use decision_cli::core::ontology::verification_graph::io::{
    from_turtle_bytes, to_canonical_turtle,
};
use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, StepKind, VerificationGraph, VerificationStep,
};
use decision_cli::vocab::verify_graph_named_graph;
use oxigraph::model::NamedNode;

fn ft_001() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/feature/FT-001")
}

fn tc_013() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/tc/TC-013")
}

fn env_ephemeral() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli")
}

#[test]
fn empty_graph_round_trips() {
    let g = VerificationGraph::new("VG-001", ArtifactRef(ft_001()), env_ephemeral(), vec![]);
    let ttl = to_canonical_turtle(&g);
    let parsed =
        from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("empty graph parses");
    assert_eq!(parsed, g, "empty graph round-trips");
}

#[test]
fn ordered_step_list_preserves_order() {
    let id = "VG-002";
    let steps = vec![
        VerificationStep::new(
            id,
            0,
            StepFields::ShellCommand {
                command: "dec init".to_string(),
                expect_exit_code: Some(0),
                capture_output: None,
            },
        ),
        VerificationStep::new(
            id,
            1,
            StepFields::SparqlAssertion {
                target: ".dec/store".to_string(),
                query: "SELECT ?s WHERE { ?s ?p ?o }".to_string(),
                expect_rows: Some(1),
            },
        ),
        VerificationStep::new(
            id,
            2,
            StepFields::FileAssertion {
                path: ".dec/store/orchestration.nq".to_string(),
                expect_hash: None,
                expect_content: None,
            },
        ),
    ];
    let g = VerificationGraph::new(id, ArtifactRef(ft_001()), env_ephemeral(), steps);
    let ttl = to_canonical_turtle(&g);
    let parsed =
        from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("ordered steps parse");
    assert_eq!(parsed.steps.len(), 3);
    assert_eq!(parsed.steps[0].kind, StepKind::ShellCommand);
    assert_eq!(parsed.steps[1].kind, StepKind::SparqlAssertion);
    assert_eq!(parsed.steps[2].kind, StepKind::FileAssertion);
    assert_eq!(parsed, g, "in-memory model is byte-equal after round-trip");
}

#[test]
fn step_iris_stable_across_reloads() {
    let id = "VG-003";
    let step = VerificationStep::new(
        id,
        0,
        StepFields::ShellCommand {
            command: "ls".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    );
    let expected_iri = step.id.clone();
    let g = VerificationGraph::new(id, ArtifactRef(ft_001()), env_ephemeral(), vec![step]);
    let ttl = to_canonical_turtle(&g);
    let a = from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("first parse");
    let b = from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("second parse");
    assert_eq!(a.steps[0].id, expected_iri);
    assert_eq!(
        a.steps[0].id, b.steps[0].id,
        "step IRIs stable across reloads"
    );
}

#[test]
fn polymorphic_verifies_round_trips_for_tc() {
    let g = VerificationGraph::new(
        "VG-004",
        ArtifactRef(tc_013()),
        env_ephemeral(),
        vec![VerificationStep::new(
            "VG-004",
            0,
            StepFields::Capture {
                from_step: None,
                bind_as: "snapshot".to_string(),
            },
        )],
    );
    let ttl = to_canonical_turtle(&g);
    let parsed =
        from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("TC-targeted graph parses");
    assert_eq!(parsed.verifies.0, tc_013());
}

#[test]
fn polymorphic_verifies_round_trips_for_feature() {
    let g = VerificationGraph::new(
        "VG-005",
        ArtifactRef(ft_001()),
        env_ephemeral(),
        vec![VerificationStep::new(
            "VG-005",
            0,
            StepFields::Capture {
                from_step: None,
                bind_as: "snapshot".to_string(),
            },
        )],
    );
    let ttl = to_canonical_turtle(&g);
    let parsed = from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes())
        .expect("feature-targeted graph parses");
    assert_eq!(parsed.verifies.0, ft_001());
}

#[test]
fn placeholder_strings_round_trip_verbatim() {
    let placeholder = "dec verify ${earlier_capture}";
    let g = VerificationGraph::new(
        "VG-006",
        ArtifactRef(ft_001()),
        env_ephemeral(),
        vec![VerificationStep::new(
            "VG-006",
            0,
            StepFields::ShellCommand {
                command: placeholder.to_string(),
                expect_exit_code: None,
                capture_output: None,
            },
        )],
    );
    let ttl = to_canonical_turtle(&g);
    assert!(
        ttl.contains("${earlier_capture}"),
        "ttl carries placeholder verbatim:\n{ttl}"
    );
    let parsed =
        from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes()).expect("placeholder parses");
    match &parsed.steps[0].fields {
        StepFields::ShellCommand { command, .. } => {
            assert_eq!(command, placeholder);
        }
        _ => panic!("expected shell-command"),
    }
}

#[test]
fn reload_yields_equal_projection() {
    let g = VerificationGraph::new(
        "VG-007",
        ArtifactRef(ft_001()),
        env_ephemeral(),
        vec![
            VerificationStep::new(
                "VG-007",
                0,
                StepFields::HttpRequest {
                    method: "GET".to_string(),
                    url: "https://example.com".to_string(),
                    expect_status: Some(200),
                },
            ),
            VerificationStep::new(
                "VG-007",
                1,
                StepFields::WaitFor {
                    condition: NamedNode::new_unchecked(
                        "https://decision-cli.dev/ns/step/VG-007/0",
                    ),
                    timeout: "PT5S".to_string(),
                },
            ),
        ],
    );
    let ttl_1 = to_canonical_turtle(&g);
    let parsed = from_turtle_bytes(Path::new("test.ttl"), ttl_1.as_bytes()).expect("parse 1");
    let ttl_2 = to_canonical_turtle(&parsed);
    assert_eq!(ttl_1, ttl_2, "canonical Turtle is fixpoint across reparse");
    let parsed_again = from_turtle_bytes(Path::new("test.ttl"), ttl_2.as_bytes()).expect("parse 2");
    assert_eq!(parsed, parsed_again);
}

#[test]
fn canonical_serializer_is_deterministic() {
    let g = VerificationGraph::new(
        "VG-008",
        ArtifactRef(ft_001()),
        env_ephemeral(),
        vec![VerificationStep::new(
            "VG-008",
            0,
            StepFields::SparqlAssertion {
                target: ".dec/store".to_string(),
                query: "SELECT ?x WHERE { ?x a ?t }".to_string(),
                expect_rows: Some(2),
            },
        )],
    );
    let a = to_canonical_turtle(&g);
    let b = to_canonical_turtle(&g);
    assert_eq!(a, b, "canonical Turtle must be byte-stable");
}

#[test]
fn coverage_predicate_round_trips() {
    let tc_a = NamedNode::new_unchecked("https://decision-cli.dev/ns/tc/TC-100");
    let tc_b = NamedNode::new_unchecked("https://decision-cli.dev/ns/tc/TC-101");
    let mut step = VerificationStep::new(
        "VG-009",
        0,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    );
    step.provides_evidence_for = vec![tc_a.clone(), tc_b.clone()];
    let g = VerificationGraph::new("VG-009", ArtifactRef(ft_001()), env_ephemeral(), vec![step]);
    let ttl = to_canonical_turtle(&g);
    let parsed = from_turtle_bytes(Path::new("test.ttl"), ttl.as_bytes())
        .expect("coverage predicate parses");
    let parsed_evidence = &parsed.steps[0].provides_evidence_for;
    assert!(parsed_evidence.contains(&tc_a));
    assert!(parsed_evidence.contains(&tc_b));
    assert_eq!(parsed_evidence.len(), 2);
}

#[test]
fn step_kind_round_trips_in_serialized_turtle() {
    // Smoke test: every step kind appears in the canonical Turtle stream
    // with the expected `dec:stepType` literal.
    for kind in [
        StepKind::ShellCommand,
        StepKind::SparqlAssertion,
        StepKind::FileAssertion,
        StepKind::HttpRequest,
        StepKind::WaitFor,
        StepKind::Capture,
    ] {
        let fields = match kind {
            StepKind::ShellCommand => StepFields::ShellCommand {
                command: "true".to_string(),
                expect_exit_code: None,
                capture_output: None,
            },
            StepKind::SparqlAssertion => StepFields::SparqlAssertion {
                target: ".dec/store".to_string(),
                query: "SELECT * { ?s ?p ?o }".to_string(),
                expect_rows: None,
            },
            StepKind::FileAssertion => StepFields::FileAssertion {
                path: ".dec/store".to_string(),
                expect_hash: None,
                expect_content: None,
            },
            StepKind::HttpRequest => StepFields::HttpRequest {
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                expect_status: None,
            },
            StepKind::WaitFor => StepFields::WaitFor {
                condition: NamedNode::new_unchecked("https://decision-cli.dev/ns/step/VG-x/0"),
                timeout: "PT1S".to_string(),
            },
            StepKind::Capture => StepFields::Capture {
                from_step: None,
                bind_as: "x".to_string(),
            },
        };
        let g = VerificationGraph::new(
            "VG-kind",
            ArtifactRef(ft_001()),
            env_ephemeral(),
            vec![VerificationStep::new("VG-kind", 0, fields)],
        );
        let ttl = to_canonical_turtle(&g);
        assert!(
            ttl.contains(kind.as_str()),
            "ttl must mention kind {kind:?}:\n{ttl}"
        );
    }
}

#[test]
fn graph_to_quads_uses_named_graph_target() {
    // Sanity check: every quad lands in the supplied named graph.
    let g = VerificationGraph::new(
        "VG-010",
        ArtifactRef(ft_001()),
        env_ephemeral(),
        vec![VerificationStep::new(
            "VG-010",
            0,
            StepFields::Capture {
                from_step: None,
                bind_as: "x".to_string(),
            },
        )],
    );
    let quads = g.to_quads(verify_graph_named_graph());
    let target = verify_graph_named_graph().as_str();
    for q in &quads {
        if let oxigraph::model::GraphName::NamedNode(n) = &q.graph_name {
            assert_eq!(n.as_str(), target, "graph quad must land in verify-graph");
        } else {
            panic!("quad missing named graph: {q:?}");
        }
    }
}
