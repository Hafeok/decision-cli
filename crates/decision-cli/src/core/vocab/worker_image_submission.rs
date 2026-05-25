//! FT-087 / ADR-040 / ADR-055 — `dec:WorkerImageSubmission` vocabulary.
//!
//! A WorkerImageSubmission is the boundary artifact CI posts when a worker
//! author releases a new image version. It carries the claim payload the
//! WorkerCurator consumes for admission (FT-092). Classification is
//! `dec:InitialRequest`, a `dec:BoundaryArtifact` subclass: there is no
//! upstream motivational origin in the orchestration graph itself.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

/// Class IRI for `dec:WorkerImageSubmission` (FT-087 / ADR-055).
pub const IRI_DEC_WORKER_IMAGE_SUBMISSION: &str =
    "https://decision-cli.dev/ns#WorkerImageSubmission";

/// Named graph holding the worker-image-submission catalog projections.
pub const IRI_DEC_GRAPH_WORKER_IMAGE_SUBMISSION: &str =
    "https://decision-cli.dev/ns/graph/worker-image-submission";

/// IRI prefix for minted submission IRIs:
/// `https://decision-cli.dev/ns/worker-image-submission/<id>`.
pub const IRI_DEC_WORKER_IMAGE_SUBMISSION_PREFIX: &str =
    "https://decision-cli.dev/ns/worker-image-submission/";

/// `dec:submission_id` — stable id for catalog lookup.
pub const IRI_DEC_SUBMISSION_ID: &str = "https://decision-cli.dev/ns#submission_id";

/// `dec:candidate_registry_ref` — proposed OCI reference with digest.
pub const IRI_DEC_CANDIDATE_REGISTRY_REF: &str =
    "https://decision-cli.dev/ns#candidate_registry_ref";

/// `dec:claimed_capability_tag` — capability-tag claim (multi-valued; ≥1).
pub const IRI_DEC_CLAIMED_CAPABILITY_TAG: &str =
    "https://decision-cli.dev/ns#claimed_capability_tag";

/// `dec:claimed_compatible_role` — IRI of a `dec:Role` claim (multi-valued).
pub const IRI_DEC_CLAIMED_COMPATIBLE_ROLE: &str =
    "https://decision-cli.dev/ns#claimed_compatible_role";

/// `dec:claimed_sbom_ref` — OCI referrer URI for the SBOM attestation
/// (per FT-091).
pub const IRI_DEC_CLAIMED_SBOM_REF: &str = "https://decision-cli.dev/ns#claimed_sbom_ref";

/// `dec:claimed_signature_subject` — sigstore Fulcio certificate subject
/// (per FT-089).
pub const IRI_DEC_CLAIMED_SIGNATURE_SUBJECT: &str =
    "https://decision-cli.dev/ns#claimed_signature_subject";

/// `dec:claimed_signature_issuer` — sigstore Fulcio issuer URI (per FT-089).
pub const IRI_DEC_CLAIMED_SIGNATURE_ISSUER: &str =
    "https://decision-cli.dev/ns#claimed_signature_issuer";

/// `dec:claimed_source_repo_uri` — provenance: source repo URL.
pub const IRI_DEC_CLAIMED_SOURCE_REPO_URI: &str =
    "https://decision-cli.dev/ns#claimed_source_repo_uri";

/// `dec:claimed_source_commit_hash` — provenance: commit SHA built from.
pub const IRI_DEC_CLAIMED_SOURCE_COMMIT_HASH: &str =
    "https://decision-cli.dev/ns#claimed_source_commit_hash";

/// `dec:claimed_build_run_url` — provenance: CI run URL (GitHub Actions).
pub const IRI_DEC_CLAIMED_BUILD_RUN_URL: &str =
    "https://decision-cli.dev/ns#claimed_build_run_url";

/// `dec:submission_lifecycle_state` — `received | under-review | admitted | rejected`.
pub const IRI_DEC_SUBMISSION_LIFECYCLE_STATE: &str =
    "https://decision-cli.dev/ns#submission_lifecycle_state";

/// `dec:produced_workerimage` — edge written on admission, pointing at the
/// minted `dec:WorkerImage`.
pub const IRI_DEC_PRODUCED_WORKERIMAGE: &str =
    "https://decision-cli.dev/ns#produced_workerimage";

/// `dec:produced_feedback` — edge written on rejection, pointing at the
/// `dec:Feedback` artifact of class `submission-rejected`.
pub const IRI_DEC_PRODUCED_FEEDBACK: &str = "https://decision-cli.dev/ns#produced_feedback";

// --- Lifecycle state literals -----------------------------------------------

pub const SUBMISSION_STATE_RECEIVED: &str = "received";
pub const SUBMISSION_STATE_UNDER_REVIEW: &str = "under-review";
pub const SUBMISSION_STATE_ADMITTED: &str = "admitted";
pub const SUBMISSION_STATE_REJECTED: &str = "rejected";

#[must_use]
pub fn worker_image_submission_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WORKER_IMAGE_SUBMISSION)
}

#[must_use]
pub fn worker_image_submission_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_WORKER_IMAGE_SUBMISSION)
}

#[must_use]
pub fn submission_id_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SUBMISSION_ID)
}

#[must_use]
pub fn candidate_registry_ref_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CANDIDATE_REGISTRY_REF)
}

#[must_use]
pub fn claimed_capability_tag_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_CAPABILITY_TAG)
}

#[must_use]
pub fn claimed_compatible_role_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_COMPATIBLE_ROLE)
}

#[must_use]
pub fn claimed_sbom_ref_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_SBOM_REF)
}

#[must_use]
pub fn claimed_signature_subject_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_SIGNATURE_SUBJECT)
}

#[must_use]
pub fn claimed_signature_issuer_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_SIGNATURE_ISSUER)
}

#[must_use]
pub fn claimed_source_repo_uri_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_SOURCE_REPO_URI)
}

#[must_use]
pub fn claimed_source_commit_hash_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_SOURCE_COMMIT_HASH)
}

#[must_use]
pub fn claimed_build_run_url_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLAIMED_BUILD_RUN_URL)
}

#[must_use]
pub fn submission_lifecycle_state_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SUBMISSION_LIFECYCLE_STATE)
}

#[must_use]
pub fn produced_workerimage_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PRODUCED_WORKERIMAGE)
}

#[must_use]
pub fn produced_feedback_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PRODUCED_FEEDBACK)
}
