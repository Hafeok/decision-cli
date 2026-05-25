//! Unit tests for WorkerImageSubmission serialisation and SHACL validation.

use oxigraph::model::Literal;

use crate::core::ontology::boundary_artifact::validate_boundary_artifact;
use crate::core::vocab::{
    worker_image_submission_graph, IRI_DEC_CANDIDATE_REGISTRY_REF,
    IRI_DEC_SUBMISSION_LIFECYCLE_STATE,
};

use super::shacl::validate_quads;
use super::types::{SubmissionLifecycleState, WorkerImageSubmission};

fn well_formed_submission() -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: "sub-001".to_string(),
        candidate_registry_ref:
            "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                .to_string(),
        claimed_capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref:
            "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef:sbom"
                .to_string(),
        claimed_signature_subject:
            "https://github.com/example/worker/.github/workflows/build.yml@refs/heads/main"
                .to_string(),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: "https://github.com/example/worker".to_string(),
        claimed_source_commit_hash: "abc123def456".to_string(),
        claimed_build_run_url: "https://github.com/example/worker/actions/runs/123".to_string(),
        lifecycle_state: SubmissionLifecycleState::Received,
        external_origin: "github-actions:example/worker/runs/123".to_string(),
        produced_workerimage: None,
        produced_feedback: None,
    }
}

#[test]
fn well_formed_submission_passes_shacl() {
    let sub = well_formed_submission();
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_quads(&quads).expect("well-formed WorkerImageSubmission must pass SHACL");
}

#[test]
fn boundary_artifact_validator_admits_well_formed_submission() {
    let sub = well_formed_submission();
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_boundary_artifact(&quads, &sub.iri())
        .expect("Submission carries dec:external_origin so the BoundaryArtifact shape passes");
}

#[test]
fn missing_registry_digest_is_rejected() {
    let sub = well_formed_submission();
    let mut quads = sub.to_quads(worker_image_submission_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_CANDIDATE_REGISTRY_REF {
            q.object = Literal::new_simple_literal("ghcr.io/example/worker:latest").into();
        }
    }
    let err = validate_quads(&quads).expect_err("non-digest candidate_registry_ref must fail SHACL");
    assert!(err.report.contains("@sha256:"), "{}", err.report);
}

#[test]
fn missing_capability_tag_is_rejected() {
    let mut sub = well_formed_submission();
    sub.claimed_capability_tags.clear();
    let quads = sub.to_quads(worker_image_submission_graph());
    let err = validate_quads(&quads).expect_err("zero claimed_capability_tags must fail SHACL");
    assert!(err.report.contains("claimed_capability_tag"), "{}", err.report);
}

#[test]
fn unknown_lifecycle_state_is_rejected() {
    let sub = well_formed_submission();
    let mut quads = sub.to_quads(worker_image_submission_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_SUBMISSION_LIFECYCLE_STATE {
            q.object = Literal::new_simple_literal("retired").into();
        }
    }
    let err = validate_quads(&quads).expect_err("unknown lifecycle state must fail SHACL");
    assert!(err.report.contains("submission_lifecycle_state"), "{}", err.report);
}

#[test]
fn missing_external_origin_is_rejected() {
    let mut sub = well_formed_submission();
    sub.external_origin = String::new();
    let quads = sub.to_quads(worker_image_submission_graph());
    let err = validate_quads(&quads).expect_err("empty external_origin must fail SHACL");
    assert!(err.report.contains("external_origin"), "{}", err.report);
}

#[test]
fn lifecycle_state_round_trip() {
    for s in [
        SubmissionLifecycleState::Received,
        SubmissionLifecycleState::UnderReview,
        SubmissionLifecycleState::Admitted,
        SubmissionLifecycleState::Rejected,
    ] {
        assert_eq!(SubmissionLifecycleState::try_from_str(s.as_str()), Some(s));
    }
    assert_eq!(SubmissionLifecycleState::try_from_str("retired"), None);
}
