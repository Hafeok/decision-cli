//! Lift `(WorkerManifest + ReleaseBuildOutputs) → SubmissionPayloadFields` (FT-093).
//!
//! The reusable release workflow's `submit` step is responsible for
//! POSTing a `WorkerImageSubmission` payload to pipeline-cli's
//! submission endpoint (FT-094). To keep the workflow's curl invocation
//! trivial and to give the test surface ONE source of truth for the
//! field-mapping rule, this module:
//!
//! 1. Defines [`ReleaseBuildOutputs`] — the values the workflow's
//!    earlier steps produce (registry ref of the pushed image, SBOM
//!    referrer URI, sigstore identity, provenance).
//! 2. Defines [`SubmissionPayloadFields`] — a plain-data struct whose
//!    field names mirror the JSON request body the
//!    `features::submissions` HTTP handler accepts. Serializable with
//!    serde so the workflow's POST body and decision-cli's own typed
//!    payload share exactly one wire shape.
//! 3. Defines [`assemble_submission_payload`] — the lifting rule.
//!
//! `core::worker_manifest` MUST NOT depend on `features::submissions`
//! (per ADR-016's slice-level SDP). Equivalence between this struct and
//! `submissions::SubmissionPayload` is preserved through serde JSON
//! round-trip: a `SubmissionPayloadFields` serialised to JSON
//! deserialises into a `SubmissionPayload` and vice versa. The TC-135
//! checkpoint test enforces this round-trip equivalence so a future
//! drift on either side surfaces as a test failure rather than silent
//! disagreement.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::{Capabilities, RuntimeKind, WorkerManifest};

/// The values the release workflow produces during build / push / sign
/// that must be threaded onto the Submission alongside the manifest's
/// declared claims.
///
/// Every field is mandatory at this layer — the workflow either
/// computed it (digest from `buildx`, sbom ref from `cosign attach`,
/// identity from the FT-089 signing step's outputs, provenance from
/// GitHub Actions env vars) or the surrounding job failed before
/// reaching `submit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseBuildOutputs {
    /// Digest-pinned OCI reference of the pushed image
    /// (`<registry>/<repo>@sha256:<hex>`).
    pub registry_ref: String,
    /// Digest-pinned OCI referrer URI for the attached CycloneDX SBOM.
    /// In the FT-091 contract this is identical to `registry_ref` by
    /// digest, but we record it separately so a future split (separate
    /// SBOM registry, separate referrer hosting) is a one-field change.
    pub sbom_ref: String,
    /// Sigstore Fulcio certificate subject — the GitHub Actions
    /// workflow run identity, captured by the FT-089 signing primitive
    /// workflow.
    pub signature_subject: String,
    /// Sigstore Fulcio issuer URI; for keyless OIDC signing this is
    /// `https://token.actions.githubusercontent.com`.
    pub signature_issuer: String,
    /// Source repository URI (e.g. `https://github.com/example/worker`).
    pub source_repo_uri: String,
    /// Git commit SHA the image was built from.
    pub source_commit_hash: String,
    /// CI run URL (the public link to the GitHub Actions run).
    pub build_run_url: String,
}

/// Plain-data submission payload shape — exact field-name parity with
/// `features::submissions::SubmissionPayload` so the workflow's JSON
/// body and decision-cli's typed payload share one wire contract.
///
/// `id` and `external_origin` are omitted on output (left to the
/// handler to fill via UUID and CI run URL respectively) — the
/// workflow does not mint client-side ids in slice 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionPayloadFields {
    /// OCI reference of the pushed image, digest-pinned.
    pub candidate_registry_ref: String,
    /// Capability-tag claims, in manifest declaration order.
    pub claimed_capability_tags: Vec<String>,
    /// `dec:Role` IRIs the candidate image claims compatibility with.
    /// Empty vector when the manifest's `[capabilities].compatible_roles`
    /// was empty or omitted.
    pub claimed_compatible_roles: Vec<String>,
    /// SBOM OCI referrer URI.
    pub claimed_sbom_ref: String,
    /// Sigstore Fulcio certificate subject.
    pub claimed_signature_subject: String,
    /// Sigstore Fulcio issuer URI.
    pub claimed_signature_issuer: String,
    /// Source repository URI.
    pub claimed_source_repo_uri: String,
    /// Source commit SHA.
    pub claimed_source_commit_hash: String,
    /// CI run URL.
    pub claimed_build_run_url: String,
}

/// Failure modes for [`assemble_submission_payload`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssembleSubmissionError {
    /// The manifest's runtime kind is `invoked` (reserved for a future
    /// Dagger landing per ADR-065). Slice 1 only ships the subscribed
    /// shape, so the assembler refuses to lift an `invoked` manifest
    /// onto a Submission.
    #[error(
        "FT-093 / ADR-065: worker.toml declares runtime.kind = \"invoked\" which is reserved for a future Dagger landing; slice-1 release flow accepts only `subscribed`"
    )]
    UnsupportedRuntime,
    /// The manifest declares zero capability tags. The Submission's
    /// SHACL shape (FT-087) requires `claimed_capability_tag` ≥ 1; the
    /// assembler refuses early so the workflow surfaces the defect
    /// before the POST round-trip.
    #[error(
        "FT-093: worker.toml [capabilities].tags is empty; every worker MUST claim at least one capability tag per FT-088 / FT-087"
    )]
    NoCapabilityTags,
    /// Required build output is empty.
    #[error("FT-093: ReleaseBuildOutputs.{field} is empty")]
    MissingBuildOutput {
        /// Field name (e.g. `"registry_ref"`).
        field: &'static str,
    },
}

/// Combine a parsed [`WorkerManifest`] with the workflow's
/// [`ReleaseBuildOutputs`] into the JSON-shaped payload the FT-094
/// submission endpoint accepts.
///
/// This is the single source of truth for which manifest field lands on
/// which Submission field. Changes to the mapping land here (and pull
/// the TC-135 checkpoint test along), not in the YAML workflow's
/// curl command.
pub fn assemble_submission_payload(
    manifest: &WorkerManifest,
    outputs: &ReleaseBuildOutputs,
) -> Result<SubmissionPayloadFields, AssembleSubmissionError> {
    if manifest.runtime.kind == RuntimeKind::Invoked {
        return Err(AssembleSubmissionError::UnsupportedRuntime);
    }
    let Capabilities {
        tags,
        compatible_roles,
    } = manifest.capabilities.clone();
    if tags.is_empty() {
        return Err(AssembleSubmissionError::NoCapabilityTags);
    }
    check_non_empty(&outputs.registry_ref, "registry_ref")?;
    check_non_empty(&outputs.sbom_ref, "sbom_ref")?;
    check_non_empty(&outputs.signature_subject, "signature_subject")?;
    check_non_empty(&outputs.signature_issuer, "signature_issuer")?;
    check_non_empty(&outputs.source_repo_uri, "source_repo_uri")?;
    check_non_empty(&outputs.source_commit_hash, "source_commit_hash")?;
    check_non_empty(&outputs.build_run_url, "build_run_url")?;
    Ok(SubmissionPayloadFields {
        candidate_registry_ref: outputs.registry_ref.clone(),
        claimed_capability_tags: tags,
        claimed_compatible_roles: compatible_roles,
        claimed_sbom_ref: outputs.sbom_ref.clone(),
        claimed_signature_subject: outputs.signature_subject.clone(),
        claimed_signature_issuer: outputs.signature_issuer.clone(),
        claimed_source_repo_uri: outputs.source_repo_uri.clone(),
        claimed_source_commit_hash: outputs.source_commit_hash.clone(),
        claimed_build_run_url: outputs.build_run_url.clone(),
    })
}

fn check_non_empty(value: &str, field: &'static str) -> Result<(), AssembleSubmissionError> {
    if value.trim().is_empty() {
        return Err(AssembleSubmissionError::MissingBuildOutput { field });
    }
    Ok(())
}
