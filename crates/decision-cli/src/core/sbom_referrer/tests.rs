//! Unit tests for SBOM OCI referrer URI validation and curator-bundle assembly (FT-091).

use crate::core::ontology::worker_image_submission::{
    SubmissionLifecycleState, WorkerImageSubmission,
};

use super::{
    assemble_curator_submission_bundle, validate_oci_referrer_uri, CuratorSubmissionBundleError,
};

fn well_formed_uri() -> String {
    format!(
        "ghcr.io/example/worker@sha256:{}",
        "deadbeef".repeat(8) // 8 * 8 = 64 lowercase hex chars
    )
}

#[test]
fn admits_canonical_uri() {
    let parsed = validate_oci_referrer_uri(&well_formed_uri())
        .expect("canonical SBOM referrer URI must admit");
    assert_eq!(parsed.registry, "ghcr.io");
    assert_eq!(parsed.repository, "example/worker");
    assert_eq!(parsed.digest_hex.len(), 64);
    assert_eq!(parsed.as_uri(), well_formed_uri());
}

#[test]
fn admits_registry_with_port() {
    let s = format!(
        "registry.example.com:5000/team/worker@sha256:{}",
        "ab".repeat(32)
    );
    let parsed = validate_oci_referrer_uri(&s).expect("registry-with-port URI must admit");
    assert_eq!(parsed.registry, "registry.example.com:5000");
    assert_eq!(parsed.repository, "team/worker");
}

#[test]
fn rejects_empty_string() {
    let err = validate_oci_referrer_uri("").expect_err("empty URI must reject");
    assert!(err.violations.iter().any(|v| v.code == "sbom:non-empty"));
}

#[test]
fn rejects_mutable_tag_reference() {
    let err = validate_oci_referrer_uri("ghcr.io/example/worker:latest")
        .expect_err("mutable-tag URI must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:digest-pinned"));
}

#[test]
fn rejects_short_digest() {
    let err = validate_oci_referrer_uri("ghcr.io/example/worker@sha256:cafe")
        .expect_err("short-digest URI must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:digest-length"));
}

#[test]
fn rejects_uppercase_digest() {
    let s = format!("ghcr.io/example/worker@sha256:{}", "A".repeat(64));
    let err = validate_oci_referrer_uri(&s).expect_err("uppercase digest must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:digest-charset"));
}

#[test]
fn rejects_non_sha256_algorithm() {
    let s = format!("ghcr.io/example/worker@sha512:{}", "a".repeat(64));
    let err = validate_oci_referrer_uri(&s).expect_err("non-sha256 algorithm must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:digest-algorithm"));
}

#[test]
fn rejects_missing_repository() {
    let s = format!("ghcr.io@sha256:{}", "a".repeat(64));
    let err = validate_oci_referrer_uri(&s).expect_err("no `/` in reference must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:reference-shape"));
}

#[test]
fn rejects_uppercase_repository_path() {
    let s = format!("ghcr.io/EXAMPLE/worker@sha256:{}", "a".repeat(64));
    let err = validate_oci_referrer_uri(&s).expect_err("uppercase repo must reject");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "sbom:repository-chars"));
}

#[test]
fn rejects_missing_registry() {
    let s = format!("/example/worker@sha256:{}", "a".repeat(64));
    let err = validate_oci_referrer_uri(&s).expect_err("missing registry must reject");
    assert!(err.violations.iter().any(|v| v.code == "sbom:registry"));
}

#[test]
fn collects_every_violation_in_one_pass() {
    // Malformed in *both* the reference (uppercase, missing `/`) AND the
    // digest (wrong algorithm, wrong length). The validator must surface
    // every defect so the WorkerCurator's rejection Feedback can name
    // them all at once.
    let err =
        validate_oci_referrer_uri("FOO@sha512:xyz").expect_err("multi-violation URI must reject");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    assert!(
        codes.contains(&"sbom:reference-shape"),
        "expected sbom:reference-shape in {codes:?}"
    );
    assert!(
        codes.contains(&"sbom:digest-algorithm"),
        "expected sbom:digest-algorithm in {codes:?}"
    );
}

fn well_formed_submission(id: &str, sbom_ref: &str) -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: id.to_string(),
        candidate_registry_ref: format!("ghcr.io/example/{id}@sha256:{}", "deadbeef".repeat(8)),
        claimed_capability_tags: vec!["code-writer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref: sbom_ref.to_string(),
        claimed_signature_subject: format!(
            "https://github.com/example/{id}/.github/workflows/release.yml@refs/tags/v1.0.0"
        ),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: format!("https://github.com/example/{id}"),
        claimed_source_commit_hash: "abc123def456".to_string(),
        claimed_build_run_url: format!("https://github.com/example/{id}/actions/runs/1"),
        lifecycle_state: SubmissionLifecycleState::Received,
        external_origin: format!("github-actions:example/{id}/runs/1"),
        produced_workerimage: None,
        produced_feedback: None,
    }
}

#[test]
fn curator_bundle_admits_well_formed_submission() {
    let sub = well_formed_submission("sub-001", &well_formed_uri());
    let bundle = assemble_curator_submission_bundle(&sub)
        .expect("well-formed Submission must produce a Curator bundle");
    assert_eq!(bundle.submission_id, "sub-001");
    assert_eq!(bundle.sbom_referrer_uri(), well_formed_uri());
}

#[test]
fn curator_bundle_refuses_missing_sbom() {
    let sub = well_formed_submission("sub-001", "");
    let err = assemble_curator_submission_bundle(&sub)
        .expect_err("Submission with empty sbom_ref must be refused");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMissing { .. }
    ));
    assert_eq!(err.submission_id(), "sub-001");
}

#[test]
fn curator_bundle_refuses_whitespace_only_sbom() {
    let sub = well_formed_submission("sub-001", "   \t  ");
    let err = assemble_curator_submission_bundle(&sub)
        .expect_err("whitespace-only sbom_ref must be refused as missing");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMissing { .. }
    ));
}

#[test]
fn curator_bundle_refuses_malformed_sbom() {
    let sub = well_formed_submission("sub-001", "ghcr.io/example/worker:latest");
    let err =
        assemble_curator_submission_bundle(&sub).expect_err("malformed sbom_ref must be refused");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMalformed { .. }
    ));
    assert_eq!(err.submission_id(), "sub-001");
}
