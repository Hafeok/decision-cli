//! Curator-bundle assembly with mandatory SBOM exposure (FT-091 / ADR-059).
//!
//! The WorkerCurator role (FT-092) consumes a per-Submission bundle that
//! exposes the SBOM referrer alongside the other admission fields. FT-091
//! pins one behavioural invariant on that assembly step:
//!
//! > The Curator's bundle exposes the SBOM reference; bundle assembly
//! > fails when the SBOM is declared missing on a Submission.
//!
//! "Missing" here means: empty `claimed_sbom_ref`, or a `claimed_sbom_ref`
//! that does not parse as a syntactically-correct OCI referrer descriptor
//! ([`super::validate_oci_referrer_uri`]).
//!
//! The bundle this module produces is the *SBOM-shaped slice* of the full
//! Curator bundle — the broader bundle assembly (capability-tag claims,
//! signature verdict citation, role-compatibility claims) lives in
//! FT-092's slice. By keeping the SBOM contract here, FT-092's bundle
//! assembler imports a typed value rather than re-validating the URI.

use thiserror::Error;

use crate::core::ontology::worker_image_submission::WorkerImageSubmission;

use super::uri::OciReferrerUri;
use super::validate::{validate_oci_referrer_uri, OciReferrerUriValidationError};

/// SBOM-shaped slice of the WorkerCurator's per-Submission bundle (FT-091 / FT-092).
///
/// Holds the parsed [`OciReferrerUri`] alongside the originating
/// Submission's id so the consumer can correlate without re-walking the
/// Submission's other fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratorSubmissionBundle {
    /// Originating `dec:WorkerImageSubmission` id.
    pub submission_id: String,
    /// Parsed SBOM referrer URI. Guaranteed syntactically well-formed.
    pub sbom_referrer: OciReferrerUri,
}

impl CuratorSubmissionBundle {
    /// Rebuild the canonical SBOM referrer URI for inclusion in the
    /// rendered Curator bundle body.
    #[must_use]
    pub fn sbom_referrer_uri(&self) -> String {
        self.sbom_referrer.as_uri()
    }
}

/// Failure modes for [`assemble_curator_submission_bundle`].
#[derive(Debug, Error)]
pub enum CuratorSubmissionBundleError {
    /// The Submission's `claimed_sbom_ref` is empty or whitespace-only —
    /// the SBOM was *declared missing* on the Submission, which FT-091
    /// requires the bundle assembler to refuse.
    #[error(
        "Curator bundle assembly refused: dec:WorkerImageSubmission {submission_id} declares \
         no SBOM (claimed_sbom_ref is empty); FT-091 requires every admitted Submission to \
         carry a CycloneDX SBOM as an OCI referrer"
    )]
    SbomMissing {
        /// Originating Submission id.
        submission_id: String,
    },
    /// The Submission's `claimed_sbom_ref` is non-empty but does not
    /// parse as a syntactically-correct OCI referrer descriptor.
    #[error(
        "Curator bundle assembly refused: dec:WorkerImageSubmission {submission_id} carries \
         a malformed SBOM referrer URI: {source}"
    )]
    SbomMalformed {
        /// Originating Submission id.
        submission_id: String,
        /// Wrapped syntactic-validation error.
        #[source]
        source: OciReferrerUriValidationError,
    },
}

impl CuratorSubmissionBundleError {
    /// The originating `dec:WorkerImageSubmission` id, regardless of
    /// failure mode — useful for routing the rejection Feedback (FT-092)
    /// back to the submitter.
    #[must_use]
    pub fn submission_id(&self) -> &str {
        match self {
            Self::SbomMissing { submission_id } => submission_id.as_str(),
            Self::SbomMalformed { submission_id, .. } => submission_id.as_str(),
        }
    }
}

/// Assemble the SBOM-shaped slice of the WorkerCurator's per-Submission
/// bundle.
///
/// Returns `Ok(CuratorSubmissionBundle)` when the Submission declares a
/// syntactically-correct OCI referrer descriptor for its SBOM. Returns
/// an error otherwise — without producing a partial bundle. FT-091's
/// success criterion is the *refusal*: a Submission that declared no
/// SBOM cannot reach the Curator with a half-built bundle.
pub fn assemble_curator_submission_bundle(
    submission: &WorkerImageSubmission,
) -> Result<CuratorSubmissionBundle, CuratorSubmissionBundleError> {
    let raw = submission.claimed_sbom_ref.trim();
    if raw.is_empty() {
        return Err(CuratorSubmissionBundleError::SbomMissing {
            submission_id: submission.id.clone(),
        });
    }
    let sbom_referrer =
        validate_oci_referrer_uri(raw).map_err(|source| CuratorSubmissionBundleError::SbomMalformed {
            submission_id: submission.id.clone(),
            source,
        })?;
    Ok(CuratorSubmissionBundle {
        submission_id: submission.id.clone(),
        sbom_referrer,
    })
}
