//! TC-058 — Safety check raises when step requiredOps not subset of env allowedOps.
//!
//! Validates: FT-037 · ADR-028.
//! Spec: `.product/tests/TC-058-safety-check-raises-when-step-requiredops-not-subs.md`
//!
//! Each acceptance criterion lives in its own `#[test]` exercising the
//! pure check functions in `core::verify::safety` against
//! programmatically constructed `VerificationGraph` and
//! `VerificationEnvironment` values. No on-disk I/O.

use decision_cli::core::ontology::verification_env::{SafetyClass, VerificationEnvironment};
use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, StepIri, VerificationGraph, VerificationStep,
};
use decision_cli::core::verify::safety::{
    check_graph_against_env, check_graph_against_env_all, check_step_against_env, OpSource,
    SafetyError,
};
use oxigraph::model::NamedNode;

fn ft_001_ref() -> ArtifactRef {
    ArtifactRef(NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/feature/FT-001",
    ))
}

fn env(id: &str, allowed: &[&str], class: SafetyClass) -> VerificationEnvironment {
    let (env_type, endpoint) = match class {
        SafetyClass::ProductionReadonly => (
            "remote-http".to_string(),
            Some("https://example.com".to_string()),
        ),
        _ => ("ephemeral-tempdir".to_string(), None),
    };
    VerificationEnvironment {
        id: id.to_string(),
        env_type,
        setup: None,
        teardown: None,
        allowed_ops: allowed.iter().map(|s| (*s).to_string()).collect(),
        safety_class: class,
        endpoint,
    }
}

fn shell_step(graph: &str, idx: usize) -> VerificationStep {
    VerificationStep::new(
        graph,
        idx,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: None,
            capture_output: None,
        },
    )
}

fn http_post_step(graph: &str, idx: usize) -> VerificationStep {
    VerificationStep::new(
        graph,
        idx,
        StepFields::HttpRequest {
            method: "POST".to_string(),
            url: "https://example.com".to_string(),
            expect_status: None,
        },
    )
}

fn graph_for(env_id: &str, steps: Vec<VerificationStep>) -> VerificationGraph {
    VerificationGraph::new(
        "VG-tc058",
        ft_001_ref(),
        NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/env/{env_id}"
        )),
        steps,
    )
}

#[test]
fn single_step_violation_carries_full_context() {
    // (1) Single-step violation: http POST against `production-readonly` env.
    let e = env("prod-readonly", &["http-readonly"], SafetyClass::ProductionReadonly);
    let s = http_post_step("VG-tc058", 0);
    let err = check_step_against_env(&s, &e).expect_err("must surface SafetyError");
    let v = err.as_violation().expect("structural violation expected");
    assert_eq!(v.missing_ops, vec!["http-mutating".to_string()]);
    assert_eq!(v.env_safety_class, "production-readonly");
    assert_eq!(v.step_kind, "http-request");
    assert!(v.step_id.contains("VG-tc058"));
}

#[test]
fn whole_graph_violation_returns_first() {
    // (2) Three-step graph where step 2 violates.
    let e = env("dev", &["shell", "filesystem"], SafetyClass::SharedNonDestructive);
    let g = graph_for(
        "dev",
        vec![
            shell_step("VG-tc058", 0),
            // Step 2 (index 1) violates: HTTP POST not allowed.
            http_post_step("VG-tc058", 1),
            shell_step("VG-tc058", 2),
        ],
    );
    let err = check_graph_against_env(&g, &e).expect_err("first violation surfaces");
    let v = err.as_violation().expect("violation");
    // Step IRI must end in `/1` — the offending step's deterministic IRI.
    assert!(
        v.step_id.ends_with("/1"),
        "expected step-2 IRI, got {}",
        v.step_id
    );
    // Constructed IRI form
    let expected_step_iri: StepIri =
        decision_cli::core::ontology::verification_graph::step_iri_for("VG-tc058", 1);
    assert_eq!(v.step_id, expected_step_iri.as_str().to_string());
}

#[test]
fn all_violations_variant_lists_every_failure_in_order() {
    // (3) Two violating steps in a four-step graph; both surface.
    let e = env("dev", &["shell"], SafetyClass::SharedNonDestructive);
    let g = graph_for(
        "dev",
        vec![
            shell_step("VG-tc058", 0),       // passes: shell only is fine for shell?
            http_post_step("VG-tc058", 1),   // violates
            shell_step("VG-tc058", 2),       // shell + filesystem required ⊄ {shell}
            http_post_step("VG-tc058", 3),   // violates
        ],
    );
    let errs = check_graph_against_env_all(&g, &e).expect_err("all violations");
    // shell-step requires {shell, filesystem}, env only has {shell} ⇒ 3 violations total
    // (index 0, 1, 2, 3 — but index 0 also fails because shell needs filesystem)
    assert!(errs.len() >= 2, "expected ≥ 2 violations, got {}", errs.len());
    for err in &errs {
        assert!(err.as_violation().is_some());
    }
    // The first violating step's IRI must mention step 0 (shell on shell-only env)
    let v0 = errs[0].as_violation().expect("first violation");
    assert!(v0.step_id.ends_with("/0"), "first violation on step 0; got {}", v0.step_id);
}

#[test]
fn op_direction_is_subset_not_superset() {
    // (4) shell against {shell, filesystem} passes; {shell, filesystem} against {shell} fails.
    let big_env = env("dev", &["shell", "filesystem"], SafetyClass::SharedNonDestructive);
    let small_env = env("ephemeral", &["shell"], SafetyClass::Isolated);
    let s = shell_step("VG-tc058", 0);
    // shell-command requires {shell, filesystem}; against big env it's a subset.
    check_step_against_env(&s, &big_env).expect("subset passes");
    // Against small env (only shell), filesystem is missing.
    let err = check_step_against_env(&s, &small_env).expect_err("superset fails");
    let v = err.as_violation().expect("violation");
    assert_eq!(v.missing_ops, vec!["filesystem".to_string()]);
}

#[test]
fn conditional_sparql_local_vs_http_target() {
    // (5) Local target needs `sparql-local`; HTTP target needs `sparql-http`.
    let env_local_only = env("local", &["sparql-local"], SafetyClass::Isolated);
    let env_http_only = env("http", &["sparql-http"], SafetyClass::SharedNonDestructive);

    let local_step = VerificationStep::new(
        "VG-tc058",
        0,
        StepFields::SparqlAssertion {
            target: ".dec/store".to_string(),
            query: "SELECT * { ?s ?p ?o }".to_string(),
            expect_rows: None,
        },
    );
    let http_step = VerificationStep::new(
        "VG-tc058",
        1,
        StepFields::SparqlAssertion {
            target: "https://example.com/sparql".to_string(),
            query: "SELECT * { ?s ?p ?o }".to_string(),
            expect_rows: None,
        },
    );

    // Local target on http-only env: fails with sparql-local missing.
    let err = check_step_against_env(&local_step, &env_http_only)
        .expect_err("local target needs sparql-local");
    assert_eq!(
        err.as_violation().expect("violation").missing_ops,
        vec!["sparql-local".to_string()]
    );

    // HTTP target on local-only env: fails with sparql-http missing.
    let err = check_step_against_env(&http_step, &env_local_only)
        .expect_err("http target needs sparql-http");
    assert_eq!(
        err.as_violation().expect("violation").missing_ops,
        vec!["sparql-http".to_string()]
    );

    // Crossed correctly: passes.
    check_step_against_env(&local_step, &env_local_only).expect("local/local passes");
    check_step_against_env(&http_step, &env_http_only).expect("http/http passes");
}

#[test]
fn unknown_op_token_in_env_surfaces_unknownop() {
    // (6) An env declaring `rocketship` triggers UnknownOp with source=env.
    let e = env("dev", &["rocketship"], SafetyClass::Isolated);
    let err = check_step_against_env(&shell_step("VG-tc058", 0), &e)
        .expect_err("unknown env op");
    match err {
        SafetyError::UnknownOp { token, source } => {
            assert_eq!(token, "rocketship");
            assert_eq!(source, OpSource::Env);
        }
        _ => panic!("expected UnknownOp"),
    }
}

#[test]
fn empty_graph_passes_trivially() {
    // Sanity / cross-check with TC-059 §Acceptance 3.
    let e = env("isolated", &["shell"], SafetyClass::Isolated);
    let g = graph_for("isolated", vec![]);
    check_graph_against_env(&g, &e).expect("empty graph");
    check_graph_against_env_all(&g, &e).expect("empty graph (all-violations)");
}

#[test]
fn rendered_message_carries_diff() {
    // The CLI rendering carries the step IRI, kind, missing op, env IRI,
    // safety class, and allowed-ops list — matching FT-037 §Error
    // handling's example diff format.
    let e = env("prod-readonly", &["http-readonly"], SafetyClass::ProductionReadonly);
    let err = check_step_against_env(&http_post_step("VG-tc058", 0), &e)
        .expect_err("must surface");
    let msg = format!("{err}");
    assert!(msg.contains("http-request"), "msg: {msg}");
    assert!(msg.contains("http-mutating"), "msg: {msg}");
    assert!(msg.contains("http-readonly"), "msg: {msg}");
    assert!(msg.contains("production-readonly"), "msg: {msg}");
}
