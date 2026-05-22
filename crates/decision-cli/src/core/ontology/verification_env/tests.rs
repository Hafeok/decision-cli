//! Unit tests for `VerificationEnvironment` types, SHACL validation, and I/O.

use std::path::PathBuf;

use super::io::from_turtle_bytes;
use super::shacl::validate_quads;
use super::types::{SafetyClass, VerificationEnvironment};
use super::write::to_canonical_turtle;
use crate::core::vocab::verify_env_graph;

fn ephemeral_cli_env() -> VerificationEnvironment {
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
        fixture_source: None,
    }
}

#[test]
fn safety_class_round_trips() {
    for sc in [
        SafetyClass::Isolated,
        SafetyClass::SharedNonDestructive,
        SafetyClass::ProductionReadonly,
    ] {
        assert_eq!(SafetyClass::parse(sc.as_str()), Some(sc));
    }
    assert!(SafetyClass::parse("yolo").is_none());
}

#[test]
fn ephemeral_env_passes_shacl() {
    let env = ephemeral_cli_env();
    let quads = env.to_quads(verify_env_graph());
    validate_quads(&quads).expect("ephemeral-cli env passes SHACL");
}

#[test]
fn missing_env_type_fails_shacl() {
    let env = ephemeral_cli_env();
    let mut quads = env.to_quads(verify_env_graph());
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#envType");
    let err = validate_quads(&quads).expect_err("missing envType must fail");
    assert!(err.report.contains("envType"), "{}", err.report);
}

#[test]
fn unknown_safety_class_fails_shacl() {
    let env = VerificationEnvironment {
        safety_class: SafetyClass::Isolated,
        ..ephemeral_cli_env()
    };
    let mut quads = env.to_quads(verify_env_graph());
    // Surgically replace the safety class literal with "yolo".
    for q in quads.iter_mut() {
        if q.predicate.as_str() == "https://decision-cli.dev/ns#safetyClass" {
            if let oxigraph::model::Term::Literal(_) = q.object {
                q.object = oxigraph::model::Literal::new_simple_literal("yolo").into();
            }
        }
    }
    let err = validate_quads(&quads).expect_err("unknown safetyClass must fail");
    assert!(err.report.contains("safetyClass"), "{}", err.report);
    assert!(err.report.contains("isolated"), "{}", err.report);
}

#[test]
fn empty_allowed_ops_fails_shacl() {
    let env = VerificationEnvironment {
        allowed_ops: Vec::new(),
        ..ephemeral_cli_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = validate_quads(&quads).expect_err("empty allowedOps must fail");
    assert!(err.report.contains("allowedOps"), "{}", err.report);
}

#[test]
fn remote_env_without_endpoint_fails_shacl() {
    let env = VerificationEnvironment {
        env_type: "remote-http".to_string(),
        endpoint: None,
        ..ephemeral_cli_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = validate_quads(&quads).expect_err("remote env requires endpoint");
    assert!(err.report.contains("endpoint"), "{}", err.report);
}

#[test]
fn remote_env_with_endpoint_passes_shacl() {
    let env = VerificationEnvironment {
        env_type: "remote-http".to_string(),
        endpoint: Some("https://dev.decision-cli.dev".to_string()),
        ..ephemeral_cli_env()
    };
    let quads = env.to_quads(verify_env_graph());
    validate_quads(&quads).expect("remote env with endpoint passes");
}

#[test]
fn local_env_with_endpoint_fails_shacl() {
    let env = VerificationEnvironment {
        env_type: "ephemeral-tempdir".to_string(),
        endpoint: Some("https://example.com".to_string()),
        ..ephemeral_cli_env()
    };
    let quads = env.to_quads(verify_env_graph());
    let err = validate_quads(&quads).expect_err("local env must not carry endpoint");
    assert!(err.report.contains("endpoint"), "{}", err.report);
}

#[test]
fn canonical_turtle_round_trips() {
    let env = ephemeral_cli_env();
    let ttl = to_canonical_turtle(&env);
    let parsed =
        from_turtle_bytes(&PathBuf::from("test.ttl"), ttl.as_bytes()).expect("round-trip parse");
    assert_eq!(parsed, env);
}

#[test]
fn canonical_turtle_is_deterministic() {
    let env = ephemeral_cli_env();
    let a = to_canonical_turtle(&env);
    let b = to_canonical_turtle(&env);
    assert_eq!(a, b, "canonical Turtle must be byte-stable");
}
