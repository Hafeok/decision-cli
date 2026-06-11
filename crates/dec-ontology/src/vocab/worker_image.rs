//! FT-086 / ADR-055 — `dec:WorkerImage` vocabulary.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

/// Class IRI for `dec:WorkerImage` (FT-086 / ADR-055).
pub const IRI_DEC_WORKER_IMAGE: &str = "https://decision-cli.dev/ns#WorkerImage";

/// Named graph holding the worker-image catalog projections.
pub const IRI_DEC_GRAPH_WORKER_IMAGE: &str = "https://decision-cli.dev/ns/graph/worker-image";

/// IRI prefix for minted worker-image IRIs:
/// `https://decision-cli.dev/ns/worker-image/<id>/v<version>`.
pub const IRI_DEC_WORKER_IMAGE_PREFIX: &str = "https://decision-cli.dev/ns/worker-image/";

/// `dec:worker_image_id` — stable id for catalog lookup.
pub const IRI_DEC_WORKER_IMAGE_ID: &str = "https://decision-cli.dev/ns#worker_image_id";

/// `dec:worker_image_name` — human-readable name.
pub const IRI_DEC_WORKER_IMAGE_NAME: &str = "https://decision-cli.dev/ns#worker_image_name";

/// `dec:worker_image_version` — semver (≥1).
pub const IRI_DEC_WORKER_IMAGE_VERSION: &str = "https://decision-cli.dev/ns#worker_image_version";

/// `dec:registry_ref` — OCI reference with digest (e.g.
/// `ghcr.io/example/worker@sha256:abc…`).
pub const IRI_DEC_REGISTRY_REF: &str = "https://decision-cli.dev/ns#registry_ref";

/// `dec:capability_tag` — capability-tag claim. Multi-valued; the SHACL
/// shape requires at least one.
pub const IRI_DEC_CAPABILITY_TAG: &str = "https://decision-cli.dev/ns#capability_tag";

/// `dec:compatible_role` — IRI of a `dec:Role` this image can serve.
/// Multi-valued.
pub const IRI_DEC_COMPATIBLE_ROLE: &str = "https://decision-cli.dev/ns#compatible_role";

/// `dec:signed_by_subject` — sigstore Fulcio certificate subject.
pub const IRI_DEC_SIGNED_BY_SUBJECT: &str = "https://decision-cli.dev/ns#signed_by_subject";

/// `dec:signed_by_issuer` — sigstore Fulcio issuer URI.
pub const IRI_DEC_SIGNED_BY_ISSUER: &str = "https://decision-cli.dev/ns#signed_by_issuer";

/// `dec:sbom_ref` — OCI referrer URI for the SBOM attestation.
pub const IRI_DEC_SBOM_REF: &str = "https://decision-cli.dev/ns#sbom_ref";

/// `dec:conformance_audit` — IRI of a `dec:ConformanceAudit`. Multi-valued.
pub const IRI_DEC_CONFORMANCE_AUDIT: &str = "https://decision-cli.dev/ns#conformance_audit";

/// `dec:eligibility_status` — `qualified` | `candidate` | `deprecated` | `pulled`.
pub const IRI_DEC_ELIGIBILITY_STATUS: &str = "https://decision-cli.dev/ns#eligibility_status";

/// `dec:source_repo_uri` — provenance: source repo URL.
pub const IRI_DEC_SOURCE_REPO_URI: &str = "https://decision-cli.dev/ns#source_repo_uri";

/// `dec:source_commit_hash` — provenance: commit SHA the image was built from.
pub const IRI_DEC_SOURCE_COMMIT_HASH: &str = "https://decision-cli.dev/ns#source_commit_hash";

/// `dec:build_run_url` — provenance: CI run URL (GitHub Actions, etc.).
pub const IRI_DEC_BUILD_RUN_URL: &str = "https://decision-cli.dev/ns#build_run_url";

// --- Eligibility status literals ---------------------------------------------

pub const ELIGIBILITY_QUALIFIED: &str = "qualified";
pub const ELIGIBILITY_CANDIDATE: &str = "candidate";
pub const ELIGIBILITY_DEPRECATED: &str = "deprecated";
pub const ELIGIBILITY_PULLED: &str = "pulled";

#[must_use]
pub fn worker_image_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WORKER_IMAGE)
}

#[must_use]
pub fn worker_image_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_WORKER_IMAGE)
}

#[must_use]
pub fn worker_image_id_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WORKER_IMAGE_ID)
}

#[must_use]
pub fn worker_image_name_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WORKER_IMAGE_NAME)
}

#[must_use]
pub fn worker_image_version_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WORKER_IMAGE_VERSION)
}

#[must_use]
pub fn registry_ref_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_REGISTRY_REF)
}

#[must_use]
pub fn capability_tag_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_TAG)
}

#[must_use]
pub fn compatible_role_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COMPATIBLE_ROLE)
}

#[must_use]
pub fn signed_by_subject_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SIGNED_BY_SUBJECT)
}

#[must_use]
pub fn signed_by_issuer_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SIGNED_BY_ISSUER)
}

#[must_use]
pub fn sbom_ref_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SBOM_REF)
}

#[must_use]
pub fn conformance_audit_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CONFORMANCE_AUDIT)
}

#[must_use]
pub fn eligibility_status_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ELIGIBILITY_STATUS)
}

#[must_use]
pub fn source_repo_uri_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SOURCE_REPO_URI)
}

#[must_use]
pub fn source_commit_hash_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SOURCE_COMMIT_HASH)
}

#[must_use]
pub fn build_run_url_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BUILD_RUN_URL)
}
