//! TC-169 — Dispatch-time validator rejects a proposal that references
//! commands, namespaces, or hosts not in the bundle.
//!
//! Validates: FT-102 · ADR-066.
//! Spec: `.product/tests/TC-169-dispatch-time-validator-rejects-a-proposal-that-re.md`
//!
//! The validator runs against a populated bundle's enrichment block; the
//! test constructs `EnrichmentFields` directly and the proposal in code,
//! sidestepping the catalog seeding required by TC-168. The chokepoint
//! contract proved here is independent of catalog state — `validate_proposal`
//! is a pure function of `(bundle, proposal)`.

use decision_cli::verify_graph_generate::enrichment::{
    CliCommand, CliSurface, EnrichmentFields, EnvCapabilities, OntologyVocabulary,
};
use decision_cli::verify_graph_generate::proposal::{
    GraphProposal, NewProposal, ProposedStep,
};
use decision_cli::verify_graph_generate::validator::{
    validate_proposal, ViolationKind,
};
use serde_json::json;

fn fixture_enrichment() -> EnrichmentFields {
    EnrichmentFields {
        cli_surface: CliSurface {
            commands: vec![
                CliCommand {
                    command: "dec verify graph new".to_string(),
                    capability_version: "0.3.0".to_string(),
                    source_cr: "CR-001".to_string(),
                },
                CliCommand {
                    command: "dec sparql query".to_string(),
                    capability_version: "0.3.0".to_string(),
                    source_cr: "CR-002".to_string(),
                },
            ],
            dec_subcommands: vec![
                "dec verify graph new".to_string(),
                "dec sparql query".to_string(),
            ],
            capability_version: "0.3.0".to_string(),
        },
        ontology_vocabulary: OntologyVocabulary {
            namespace: "https://decision-cli.dev/ns#".to_string(),
            prefix: "dec".to_string(),
            namespaces: vec!["https://decision-cli.dev/ns#".to_string()],
            classes: vec![
                "VerificationGraph".to_string(),
                "VerificationStep".to_string(),
            ],
            source_od: "OD-001".to_string(),
        },
        env_capabilities: EnvCapabilities {
            binaries_on_path: vec!["dec".to_string(), "bash".to_string()],
            writable_paths: vec!["$DEC_VERIFY_TMP".to_string(), "./".to_string()],
            allowed_hosts: vec!["api.dec.test".to_string()],
            environment_variables: vec![
                "DEC_VERIFY_TMP".to_string(),
                "PATH".to_string(),
            ],
            pre_seeded_artifacts: Vec::new(),
        },
        ..EnrichmentFields::default()
    }
}

fn step(kind: &str, fields: serde_json::Value) -> ProposedStep {
    ProposedStep {
        step_type: kind.to_string(),
        fields: fields.as_object().cloned().unwrap_or_default(),
        provides_evidence_for: Vec::new(),
    }
}

fn proposal(steps: Vec<ProposedStep>) -> GraphProposal {
    GraphProposal::new_new(
        "bundle-hash-fixture",
        NewProposal {
            environment: "ENV-001".to_string(),
            steps,
            rationale: "TC-169 fixture proposal".to_string(),
        },
    )
}

#[test]
fn tc_169_dispatch_time_validator_rejects_a_proposal_that_re() {
    // Compose all scenarios into the headline TC test so the runner gets
    // a single entry point. Each scenario also has its own #[test] for
    // granular debug.
    scenario_a_happy_path_no_violations();
    scenario_b_unknown_binary_rejected();
    scenario_c_unknown_dec_subcommand_rejected();
    scenario_d_unknown_sparql_namespace_rejected();
    scenario_e_w3c_whitelisted_namespace_passes();
    scenario_f_file_path_outside_writable_rejected();
    scenario_g_http_host_outside_allowed_rejected();
    scenario_h_multiple_violations_all_reported();
}

#[test]
fn scenario_a_happy_path_no_violations() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![
        step(
            "shell-command",
            json!({"command": "dec verify graph new --verifies FT-X"}),
        ),
        step(
            "sparql-assertion",
            json!({
                "target": "$DEC_VERIFY_TMP",
                "query": "PREFIX dec: <https://decision-cli.dev/ns#> PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> SELECT * WHERE { ?s rdf:type dec:Session }",
            }),
        ),
    ]);
    let v = validate_proposal(&p, &enrichment);
    assert!(v.is_empty(), "expected no violations, got {v:?}");
}

#[test]
fn scenario_b_unknown_binary_rejected() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "shell-command",
        json!({"command": "curl https://example.com/probe"}),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::Binary);
    assert_eq!(v[0].referenced_thing, "curl");
    assert!(
        v[0].why_rejected
            .contains("binaries_on_path"),
        "why_rejected should mention binaries_on_path; got {}",
        v[0].why_rejected
    );
}

#[test]
fn scenario_c_unknown_dec_subcommand_rejected() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "shell-command",
        json!({"command": "dec verify result inspect VGR-001"}),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::DecSubcommand);
    assert!(
        v[0].referenced_thing.starts_with("dec verify result"),
        "referenced_thing should start with 'dec verify result'; got {}",
        v[0].referenced_thing,
    );
}

#[test]
fn scenario_d_unknown_sparql_namespace_rejected() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "sparql-assertion",
        json!({
            "target": "$DEC_VERIFY_TMP",
            "query": "PREFIX foo: <https://fake.example/ns#> SELECT * WHERE { ?s foo:p ?o }",
        }),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::SparqlNamespace);
    assert_eq!(v[0].referenced_thing, "https://fake.example/ns#");
}

#[test]
fn scenario_e_w3c_whitelisted_namespace_passes() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "sparql-assertion",
        json!({
            "target": "$DEC_VERIFY_TMP",
            "query": "PREFIX prov: <http://www.w3.org/ns/prov#> SELECT * WHERE { ?s a prov:Activity }",
        }),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert!(v.is_empty(), "W3C prov:_ should be whitelisted; got {v:?}");
}

#[test]
fn scenario_f_file_path_outside_writable_rejected() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "file-assertion",
        json!({"target": "/etc/passwd"}),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::FilePath);
    assert_eq!(v[0].referenced_thing, "/etc/passwd");
}

#[test]
fn scenario_g_http_host_outside_allowed_rejected() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![step(
        "http-request",
        json!({"url": "https://evil.example/probe"}),
    )]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, ViolationKind::HttpHost);
    assert_eq!(v[0].referenced_thing, "evil.example");
}

#[test]
fn scenario_h_multiple_violations_all_reported() {
    let enrichment = fixture_enrichment();
    let p = proposal(vec![
        step(
            "shell-command",
            json!({"command": "dec verify result inspect VGR-001"}),
        ),
        step(
            "sparql-assertion",
            json!({
                "target": "$DEC_VERIFY_TMP",
                "query": "PREFIX foo: <https://fake.example/ns#> SELECT * WHERE { ?s ?p ?o }",
            }),
        ),
        step("file-assertion", json!({"target": "/etc/passwd"})),
    ]);
    let v = validate_proposal(&p, &enrichment);
    assert_eq!(
        v.len(),
        3,
        "validator must report all three violations independently, not short-circuit: {v:?}"
    );
    let kinds: Vec<ViolationKind> = v.iter().map(|x| x.kind).collect();
    assert!(kinds.contains(&ViolationKind::DecSubcommand));
    assert!(kinds.contains(&ViolationKind::SparqlNamespace));
    assert!(kinds.contains(&ViolationKind::FilePath));
}
