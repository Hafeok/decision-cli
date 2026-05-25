//! Unit tests for WorkerImage serialisation and SHACL validation.

use oxigraph::model::Literal;

use super::shacl::validate_quads;
use super::types::{EligibilityStatus, WorkerImage};
use crate::core::vocab::{worker_image_graph, IRI_DEC_REGISTRY_REF};

fn well_formed_image() -> WorkerImage {
    WorkerImage {
        id: "example-worker".to_string(),
        name: "Example Worker".to_string(),
        version: "1.2.0".to_string(),
        registry_ref: "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        compatible_roles: Vec::new(),
        signed_by_subject: "https://github.com/example/worker/.github/workflows/build.yml@refs/heads/main".to_string(),
        signed_by_issuer: "https://token.actions.githubusercontent.com".to_string(),
        sbom_ref: "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef:sbom".to_string(),
        conformance_audits: Vec::new(),
        eligibility_status: EligibilityStatus::Qualified,
        source_repo_uri: "https://github.com/example/worker".to_string(),
        source_commit_hash: "abc123def456".to_string(),
        build_run_url: "https://github.com/example/worker/actions/runs/123".to_string(),
    }
}

#[test]
fn well_formed_image_passes_shacl() {
    let img = well_formed_image();
    let quads = img.to_quads(worker_image_graph());
    validate_quads(&quads).expect("well-formed WorkerImage must pass SHACL");
}

#[test]
fn missing_registry_digest_is_rejected() {
    let img = well_formed_image();
    let mut quads = img.to_quads(worker_image_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_REGISTRY_REF {
            q.object = Literal::new_simple_literal("ghcr.io/example/worker:latest").into();
        }
    }
    let err = validate_quads(&quads).expect_err("non-digest registry_ref must fail SHACL");
    assert!(err.report.contains("@sha256:"), "{}", err.report);
}

#[test]
fn missing_capability_tag_is_rejected() {
    let mut img = well_formed_image();
    img.capability_tags.clear();
    let quads = img.to_quads(worker_image_graph());
    let err = validate_quads(&quads).expect_err("zero capability_tags must fail SHACL");
    assert!(err.report.contains("capability_tag"), "{}", err.report);
}

#[test]
fn non_semver_version_is_rejected() {
    let mut img = well_formed_image();
    img.version = "v1.2".to_string();
    let quads = img.to_quads(worker_image_graph());
    let err = validate_quads(&quads).expect_err("non-semver version must fail SHACL");
    assert!(err.report.contains("semver"), "{}", err.report);
}

#[test]
fn eligibility_round_trip() {
    for s in [
        EligibilityStatus::Qualified,
        EligibilityStatus::Candidate,
        EligibilityStatus::Deprecated,
        EligibilityStatus::Pulled,
    ] {
        assert_eq!(EligibilityStatus::try_from_str(s.as_str()), Some(s));
    }
    assert_eq!(EligibilityStatus::try_from_str("retired"), None);
}
