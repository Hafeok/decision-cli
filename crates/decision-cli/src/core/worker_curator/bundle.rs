//! Curator bundle assembly (FT-092 §Scope "Bundle assembly query").
//!
//! Per the FT-092 scope, the WorkerCurator consumes a structured
//! bundle that includes:
//!
//! - the focal `dec:WorkerImageSubmission` (FT-087);
//! - the paired `dec:SignatureVerdict` (FT-090);
//! - the SBOM reference (FT-091) — surfaced as the SBOM-shaped slice
//!   the FT-091 bundle assembler produces;
//! - existing `dec:WorkerImage`s with overlapping capability tags
//!   (for comparison against duplicates / supersession candidates).
//!
//! Refusal modes:
//!
//! - `MissingSignatureVerdict` — the supplied verdict does not respond
//!   to the focal Submission (FT-090 dispatch had a different
//!   Submission as its motivational origin).
//! - `Sbom { source }` — the FT-091 SBOM bundle assembler refused the
//!   Submission (empty / malformed `claimed_sbom_ref`).
//!
//! "Current orchestration policy" (capacity, capability-tag coverage,
//! preferred provenance constraints) the FT-092 spec mentions is
//! deferred to slice-2's policy artifact — the slice-1 Curator runs at
//! Level-0 autonomy, so policy is human-mediated. The bundle carries
//! the existing-WorkerImage comparison set; the Curator's prose
//! rationale captures the policy-style judgment.

use thiserror::Error;

use crate::core::identity_verifier::SignatureVerdict;
use crate::core::ontology::worker_image::WorkerImage;
use crate::core::ontology::worker_image_submission::WorkerImageSubmission;
use crate::core::sbom_referrer::{
    assemble_curator_submission_bundle, CuratorSubmissionBundle, CuratorSubmissionBundleError,
};

/// Per-Submission Curator bundle (FT-092).
///
/// Carries every artifact the WorkerCurator needs to render a verdict
/// without further graph queries:
///
/// - [`Self::submission`] — focal Submission (FT-087).
/// - [`Self::signature_verdict`] — paired SignatureVerdict (FT-090).
/// - [`Self::sbom`] — SBOM-shaped slice (FT-091).
/// - [`Self::existing_workers_with_overlapping_tags`] — already-admitted
///   WorkerImages sharing at least one capability tag with the
///   candidate. Always supplied by the caller (the existing read API
///   [`crate::core::ontology::worker_image::query_by_capability_tag`]
///   is the natural source); the slice-1 bundle is content-only —
///   policy is the Curator's judgment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratorBundle {
    /// Focal `dec:WorkerImageSubmission` (FT-087).
    pub submission: WorkerImageSubmission,
    /// Paired `dec:SignatureVerdict` (FT-090).
    pub signature_verdict: SignatureVerdict,
    /// FT-091 SBOM-shaped slice. Guaranteed syntactically-correct
    /// referrer URI because the assembler refuses missing / malformed
    /// claims at construction.
    pub sbom: CuratorSubmissionBundle,
    /// Existing WorkerImages with at least one overlapping capability
    /// tag with the candidate Submission. Caller-supplied (typically
    /// from `query_by_capability_tag`).
    pub existing_workers_with_overlapping_tags: Vec<WorkerImage>,
}

impl CuratorBundle {
    /// Convenience: the focal Submission's id.
    #[must_use]
    pub fn submission_id(&self) -> &str {
        self.submission.id.as_str()
    }

    /// Convenience: the focal Submission's `external_origin` literal.
    /// This is the FT-092 success criterion's terminal motivational
    /// citation (`brief:worker-distribution-slice-1` in the production
    /// pipeline) for the admitted WorkerImage's chain.
    #[must_use]
    pub fn external_origin(&self) -> &str {
        self.submission.external_origin.as_str()
    }
}

/// Refusal modes for [`assemble_curator_bundle`].
#[derive(Debug, Error)]
pub enum CuratorBundleError {
    /// The supplied SignatureVerdict does not motivationally respond to
    /// the focal Submission (its `dec:respondsTo` points elsewhere).
    /// Caller must supply the matching verdict.
    #[error(
        "Curator bundle assembly refused: SignatureVerdict {verdict_id} responds to <{verdict_subject}>, \
         not to focal Submission <{submission_iri}>"
    )]
    SignatureVerdictMismatch {
        /// The SignatureVerdict's id.
        verdict_id: String,
        /// The IRI the verdict's `dec:respondsTo` actually points at.
        verdict_subject: String,
        /// The focal Submission's IRI.
        submission_iri: String,
    },
    /// FT-091 SBOM bundle assembler refused the Submission.
    #[error("Curator bundle assembly refused: {source}")]
    Sbom {
        /// Wrapped FT-091 refusal.
        #[source]
        source: CuratorSubmissionBundleError,
    },
}

/// Assemble the per-Submission [`CuratorBundle`].
///
/// Validates two structural preconditions before bundle construction:
///
/// 1. The supplied [`SignatureVerdict`]'s `dec:respondsTo` matches the
///    focal Submission's IRI. A verdict drawn from the wrong dispatch
///    cannot be the FT-090 evidence for *this* Submission.
/// 2. The Submission carries a syntactically-correct
///    `claimed_sbom_ref`. Delegates to
///    [`assemble_curator_submission_bundle`] (FT-091).
///
/// On success the returned bundle is content-only — the assembler does
/// not touch the orchestration store, does not write any artifacts,
/// and does not mutate the supplied inputs.
pub fn assemble_curator_bundle(
    submission: WorkerImageSubmission,
    signature_verdict: SignatureVerdict,
    existing_workers_with_overlapping_tags: Vec<WorkerImage>,
) -> Result<CuratorBundle, CuratorBundleError> {
    let submission_iri = submission.iri();
    if signature_verdict.responds_to_submission != submission_iri {
        return Err(CuratorBundleError::SignatureVerdictMismatch {
            verdict_id: signature_verdict.id.clone(),
            verdict_subject: signature_verdict
                .responds_to_submission
                .as_str()
                .to_string(),
            submission_iri: submission_iri.as_str().to_string(),
        });
    }
    let sbom = assemble_curator_submission_bundle(&submission)
        .map_err(|source| CuratorBundleError::Sbom { source })?;
    Ok(CuratorBundle {
        submission,
        signature_verdict,
        sbom,
        existing_workers_with_overlapping_tags,
    })
}
