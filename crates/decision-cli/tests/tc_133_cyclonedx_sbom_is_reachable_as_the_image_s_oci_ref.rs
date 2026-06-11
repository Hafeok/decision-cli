//! TC-133 — CycloneDX SBOM is reachable as the image's OCI referrer.
//!
//! Validates: FT-091 · ADR-059.
//! Spec: `.product/tests/TC-133-cyclonedx-sbom-is-reachable-as-the-image-s-oci-ref.md`
//!
//! Three claims this integration test pins down end-to-end against the
//! SBOM referrer validator + curator-bundle assembler shipped under
//! `core::sbom_referrer`, against the three success criteria the FT-091
//! feature_spec declares:
//!
//! 1. **An image built via the release workflow has an attached
//!    CycloneDX SBOM referrer that `cosign download sbom <image-ref>`
//!    resolves to.** Modelled here as: a syntactically-correct
//!    digest-pinned OCI reference (the canonical SBOM referrer
//!    descriptor URI shape per OCI v1.1) is admitted by the validator,
//!    survives RDF serialisation on the originating WorkerImageSubmission,
//!    and reaches the Curator-bundle assembler intact.
//! 2. **The Submission's `sbom_ref` is validated by SHACL as a
//!    syntactically-correct OCI referrer descriptor.** A Submission
//!    carrying a mutable-tag SBOM ref, an empty digest, or a
//!    non-`sha256` algorithm is refused by the SHACL validator with a
//!    `dec:claimed_sbom_ref`-path violation.
//! 3. **The Curator's bundle exposes the SBOM reference; bundle
//!    assembly fails when the SBOM is declared missing on a
//!    Submission.** Modelled here as: a Submission whose
//!    `claimed_sbom_ref` is empty causes
//!    `assemble_curator_submission_bundle` to return
//!    `CuratorSubmissionBundleError::SbomMissing`, and a Submission
//!    whose ref is malformed returns
//!    `CuratorSubmissionBundleError::SbomMalformed`.

use decision_cli::core::ontology::worker_image_submission::{
    validate_quads, SubmissionLifecycleState, WorkerImageSubmission,
};
use decision_cli::core::sbom_referrer::{
    assemble_curator_submission_bundle, validate_oci_referrer_uri, CuratorSubmissionBundleError,
    SHA256_HEX_DIGITS,
};
use decision_cli::vocab::{worker_image_submission_graph, IRI_DEC_CLAIMED_SBOM_REF};

/// 64 lowercase hex digits — the cosign-canonical sha256 digest body.
const SBOM_DIGEST_HEX: &str = "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";

/// Canonical SBOM referrer descriptor URI per FT-091 / ADR-059: a
/// digest-pinned OCI reference of the form `<host>/<repo>@sha256:<hex>`.
/// `cosign download sbom <ref>` resolves the attached SBOM via the
/// registry's referrers API at this digest.
fn canonical_sbom_uri() -> String {
    format!("ghcr.io/example/worker@sha256:{SBOM_DIGEST_HEX}")
}

/// Construct a Submission with every other field well-formed so the
/// only axis under test is the SBOM referrer.
fn submission_with_sbom(id: &str, sbom_ref: &str) -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: id.to_string(),
        candidate_registry_ref: format!("ghcr.io/example/{id}@sha256:{SBOM_DIGEST_HEX}"),
        claimed_capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref: sbom_ref.to_string(),
        claimed_signature_subject: format!(
            "https://github.com/example/{id}/.github/workflows/release.yml@refs/tags/v1.0.0"
        ),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: format!("https://github.com/example/{id}"),
        claimed_source_commit_hash: "abc123def4567890abcdef0123456789abcdef01".to_string(),
        claimed_build_run_url: format!("https://github.com/example/{id}/actions/runs/1"),
        lifecycle_state: SubmissionLifecycleState::Received,
        external_origin: format!("github-actions:example/{id}/runs/1"),
        produced_workerimage: None,
        produced_feedback: None,
    }
}

#[test]
fn admits_canonical_sbom_referrer_uri() {
    let parsed = validate_oci_referrer_uri(&canonical_sbom_uri())
        .expect("canonical sha256-pinned SBOM referrer URI must admit");
    assert_eq!(parsed.registry, "ghcr.io");
    assert_eq!(parsed.repository, "example/worker");
    assert_eq!(parsed.digest_hex.len(), SHA256_HEX_DIGITS);
    // Round-trip: reassembling the parsed components yields the original
    // canonical URI form `cosign download sbom <ref>` consumes.
    assert_eq!(parsed.as_uri(), canonical_sbom_uri());
}

#[test]
fn shacl_admits_submission_with_canonical_sbom_ref() {
    let sub = submission_with_sbom("sub-001", &canonical_sbom_uri());
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_quads(&quads).expect(
        "Submission with canonical OCI referrer SBOM ref must pass FT-091 SHACL validation",
    );
}

#[test]
fn shacl_rejects_submission_with_mutable_tag_sbom_ref() {
    let sub = submission_with_sbom("sub-001", "ghcr.io/example/worker:latest");
    let quads = sub.to_quads(worker_image_submission_graph());
    let err = validate_quads(&quads)
        .expect_err("Submission with mutable-tag SBOM ref must be refused by FT-091 SHACL");
    // The violation must name the dec:claimed_sbom_ref predicate so the
    // WorkerCurator's rejection Feedback can cite it.
    assert!(
        err.violations
            .iter()
            .any(|v| v.path == IRI_DEC_CLAIMED_SBOM_REF),
        "expected violation on dec:claimed_sbom_ref, got {:?}",
        err.violations
    );
    assert!(
        err.report.contains("FT-091"),
        "violation report must cite FT-091: {}",
        err.report
    );
}

#[test]
fn shacl_rejects_submission_with_short_digest_sbom_ref() {
    let sub = submission_with_sbom(
        "sub-001",
        "ghcr.io/example/worker@sha256:cafe", // too short
    );
    let quads = sub.to_quads(worker_image_submission_graph());
    let err =
        validate_quads(&quads).expect_err("Submission with short-digest SBOM ref must be refused");
    assert!(err
        .violations
        .iter()
        .any(|v| v.path == IRI_DEC_CLAIMED_SBOM_REF));
}

#[test]
fn shacl_rejects_submission_with_non_sha256_sbom_ref() {
    let sub = submission_with_sbom(
        "sub-001",
        // sha512 is OCI-spec-legal but FT-091 / ADR-059 narrows to
        // sha256 for slice 1; the validator surfaces the algorithm
        // mismatch as a sbom:digest-algorithm violation.
        &format!("ghcr.io/example/worker@sha512:{}", "a".repeat(128)),
    );
    let quads = sub.to_quads(worker_image_submission_graph());
    let err =
        validate_quads(&quads).expect_err("Submission with non-sha256 SBOM ref must be refused");
    assert!(err
        .violations
        .iter()
        .any(|v| v.path == IRI_DEC_CLAIMED_SBOM_REF));
}

#[test]
fn curator_bundle_exposes_sbom_referrer_for_well_formed_submission() {
    let sub = submission_with_sbom("sub-001", &canonical_sbom_uri());
    let bundle = assemble_curator_submission_bundle(&sub)
        .expect("curator bundle assembly must succeed for a well-formed Submission");
    assert_eq!(bundle.submission_id, "sub-001");
    // The bundle MUST expose the SBOM referrer URI verbatim so the
    // WorkerCurator can cite it in the admission verdict.
    assert_eq!(bundle.sbom_referrer_uri(), canonical_sbom_uri());
    assert_eq!(bundle.sbom_referrer.registry, "ghcr.io");
    assert_eq!(bundle.sbom_referrer.repository, "example/worker");
}

#[test]
fn curator_bundle_assembly_fails_when_sbom_is_declared_missing() {
    // Submission with no SBOM declared — the explicit success criterion
    // FT-091 calls out: "bundle assembly fails when the SBOM is
    // declared missing on a Submission."
    let sub = submission_with_sbom("sub-001", "");
    let err = assemble_curator_submission_bundle(&sub)
        .expect_err("Curator bundle assembly must refuse a Submission with no SBOM");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMissing { .. }
    ));
    assert_eq!(err.submission_id(), "sub-001");
    // Error message must reference FT-091 so the WorkerCurator session
    // can cite the originating feature_spec in its rejection Feedback.
    assert!(
        err.to_string().contains("FT-091"),
        "expected FT-091 citation in error, got: {err}"
    );
}

#[test]
fn curator_bundle_assembly_fails_when_sbom_ref_is_malformed() {
    let sub = submission_with_sbom("sub-001", "not-a-registry-ref");
    let err = assemble_curator_submission_bundle(&sub)
        .expect_err("Curator bundle assembly must refuse a malformed SBOM ref");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMalformed { .. }
    ));
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-133 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs the structural claims of TC-133 so this one function
/// reproduces the exit-criterion end-to-end.
#[test]
fn tc_133_cyclonedx_sbom_is_reachable_as_the_image_s_oci_ref() {
    // 1. The canonical SBOM referrer descriptor URI shape (FT-091 / ADR-059)
    //    is admitted by the syntactic validator — exactly what
    //    `cosign download sbom <image-ref>` consumes to resolve the
    //    attached CycloneDX document via the registry's referrers API.
    let parsed = validate_oci_referrer_uri(&canonical_sbom_uri())
        .expect("canonical SBOM referrer URI must admit");
    assert_eq!(parsed.as_uri(), canonical_sbom_uri());

    // 2. SHACL admits a WorkerImageSubmission whose claimed_sbom_ref
    //    carries the canonical referrer URI.
    let well_formed = submission_with_sbom("sub-001", &canonical_sbom_uri());
    let quads = well_formed.to_quads(worker_image_submission_graph());
    validate_quads(&quads).expect("well-formed Submission must pass SHACL");

    // 3. SHACL refuses the structural failure modes the validator must
    //    catch at write time: mutable tag, short digest, non-sha256
    //    algorithm. Each rejection cites dec:claimed_sbom_ref.
    for (label, ref_value) in [
        ("mutable-tag", "ghcr.io/example/worker:latest".to_string()),
        (
            "short-digest",
            "ghcr.io/example/worker@sha256:cafe".to_string(),
        ),
        (
            "non-sha256",
            format!("ghcr.io/example/worker@sha512:{}", "a".repeat(128)),
        ),
    ] {
        let sub = submission_with_sbom("sub-001", &ref_value);
        let bad_quads = sub.to_quads(worker_image_submission_graph());
        let err = validate_quads(&bad_quads).unwrap_err_or_else_label_failure(label);
        assert!(
            err.violations
                .iter()
                .any(|v| v.path == IRI_DEC_CLAIMED_SBOM_REF),
            "{label}: expected violation on dec:claimed_sbom_ref, got {:?}",
            err.violations
        );
    }

    // 4. Curator's bundle assembly exposes the SBOM reference for a
    //    well-formed Submission.
    let bundle = assemble_curator_submission_bundle(&well_formed)
        .expect("Curator bundle assembly must succeed for a well-formed Submission");
    assert_eq!(bundle.sbom_referrer_uri(), canonical_sbom_uri());

    // 5. Curator's bundle assembly FAILS when the SBOM is declared
    //    missing on a Submission — the explicit FT-091 success criterion.
    let missing = submission_with_sbom("sub-001", "");
    let err = assemble_curator_submission_bundle(&missing)
        .expect_err("Curator bundle assembly must refuse a missing-SBOM Submission");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMissing { .. }
    ));
    assert_eq!(err.submission_id(), "sub-001");

    // 6. Curator's bundle assembly also fails when the SBOM is declared
    //    but malformed — the same admission gate covers both shapes of
    //    "the Submission did not present a usable SBOM."
    let malformed = submission_with_sbom("sub-001", "ghcr.io/example/worker:latest");
    let err = assemble_curator_submission_bundle(&malformed)
        .expect_err("Curator bundle assembly must refuse a malformed-SBOM Submission");
    assert!(matches!(
        err,
        CuratorSubmissionBundleError::SbomMalformed { .. }
    ));
}

/// Tiny helper that turns `Err(e) -> e` into a labelled panic so the
/// driver loop in the checkpoint test reports which case failed.
trait UnwrapErrOrFail<E> {
    fn unwrap_err_or_else_label_failure(self, label: &str) -> E;
}

impl<T, E> UnwrapErrOrFail<E> for Result<T, E> {
    fn unwrap_err_or_else_label_failure(self, label: &str) -> E {
        match self {
            Ok(_) => panic!("{label}: expected SHACL refusal but validation passed"),
            Err(e) => e,
        }
    }
}
