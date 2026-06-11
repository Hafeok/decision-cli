//! WorkerCurator session materialisation (FT-092).
//!
//! Turns a `(bundle, verdict, context)` triple into a `CuratorOutcome`
//! whose constituent artifacts can be persisted atomically through the
//! [`dec_graph::stream_writer::StreamWriter`] chokepoint. The session
//! runner does NOT write to a store — it returns the typed artifacts so
//! the caller (slice 2's `dec worker curator` CLI surface or FT-092's
//! integration test) can compose them into a single transaction.
//!
//! On Admit the runner produces:
//! - a `dec:WorkerImage` derived from the Submission's claims, with
//!   `eligibility_status=qualified` (slice-1 admission stance per
//!   ADR-055);
//! - a `dec:ConformanceAudit` of class `manual-review` (ADR-060)
//!   whose `dec:audits` predicate points at the new WorkerImage and
//!   whose `dec:audit_notes` captures the Curator's rationale;
//! - an updated Submission carrying `lifecycle_state=admitted` and the
//!   `dec:produced_workerimage` edge to the minted WorkerImage's IRI.
//!
//! On Reject the runner produces:
//! - a `dec:Feedback` artifact (ADR-022) emitted by the Curator session,
//!   targeting the worker author role (so the rejection routes back to
//!   the producer), carrying both `dec:sourceArtifact` → the rejected
//!   Submission and `dec:evidence` → the disqualification rationale;
//! - an updated Submission carrying `lifecycle_state=rejected` and the
//!   `dec:produced_feedback` edge to the Feedback's IRI.
//!
//! ## A note on the Feedback class
//!
//! ADR-023 enumerates six feedback classes — `submission-rejected` is
//! not one of them. FT-092 §Scope's "class `submission-rejected`" wording
//! is a typed-Rust intent we model via the [`CuratorOutcome::Rejected`]
//! variant; the wire-level Feedback artifact uses the closest existing
//! class — [`FeedbackClass::Defect`] — since the Submission is found
//! defective. Routing is overridden to the
//! [`WORKER_AUTHOR_TARGET_ROLE`] so the rejection reaches the worker
//! author, not the default `defect → verifier` lane. This keeps the
//! controlled vocabulary closed without losing the FT-092 semantic.

use thiserror::Error;

use oxigraph::model::NamedNode;

use crate::feedback::{Feedback, FeedbackClass, Severity};
use dec_graph::ontology::conformance_audit::{ConformanceAudit, ConformanceAuditClass};
use dec_graph::ontology::worker_image::{EligibilityStatus, WorkerImage};
use dec_graph::ontology::worker_image_submission::{
    SubmissionLifecycleState, WorkerImageSubmission,
};

use super::bundle::CuratorBundle;
use super::verdict::CuratorVerdict;

/// Stable `dec:roleId` literal for the WorkerCurator role catalog
/// entry. The slice-1 catalog seed (FT-092) writes this id; the FT-092
/// integration test uses it to assemble the agent IRI.
pub const WORKER_CURATOR_ROLE_ID: &str = "worker-curator";

/// Canonical agent IRI minted for the WorkerCurator role
/// (`prov:wasAttributedTo` target on every artifact produced by a
/// Curator session). Mirrors the `identity-verifier` agent IRI shape
/// (FT-090).
pub const WORKER_CURATOR_AGENT_IRI: &str = "https://decision-cli.dev/ns/agent/worker-curator";

/// Default routing target for rejection Feedback. ADR-026's table maps
/// `defect → verifier`; for Curator rejections we route to the worker
/// author so the feedback reaches the producer of the rejected
/// Submission. Captured on the Feedback's `dec:targetRole`.
pub const WORKER_AUTHOR_TARGET_ROLE: &str = "worker-author";

/// Per-session context the runner threads through every produced
/// artifact's mechanical-provenance block (ADR-038). The caller assembles
/// this from the live Session record (slice-1: hand-constructed by
/// FT-092's integration test; slice-2+: emitted by the harness via the
/// FT-001 GraphWriter chokepoint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratorSessionContext {
    /// IRI of the Curator's `dec:Session` artifact — target of
    /// `prov:wasGeneratedBy` on every produced artifact.
    pub session_iri: NamedNode,
    /// IRI of the Curator's `dec:Agent` — target of
    /// `prov:wasAttributedTo` on every produced artifact. Defaults to
    /// [`WORKER_CURATOR_AGENT_IRI`] for the slice-1 catalog.
    pub agent_iri: NamedNode,
    /// RFC3339 emission timestamp.
    pub generated_at_time: String,
    /// `dec:inStream` IRI for any produced Feedback (ADR-005).
    pub in_stream: NamedNode,
    /// Curator-chosen id base for the minted WorkerImage (Admit path)
    /// and ConformanceAudit. When `None` the runner derives a stable
    /// id from the Submission's id and timestamp.
    pub minted_image_id_override: Option<String>,
}

/// Outcome of a Curator session. Carries the typed artifacts (or the
/// Feedback) the caller persists through the StreamWriter chokepoint.
///
/// Only [`Debug`] / [`Clone`] — the inner `RejectionOutcome` carries a
/// [`Feedback`] artifact that intentionally does NOT derive `Eq` (it
/// holds substring fields with no canonicalisation), so this enum
/// matches.
#[derive(Debug, Clone)]
pub enum CuratorOutcome {
    /// Admit path — every artifact is materialised; the Submission is
    /// transitioned to `admitted` and carries the
    /// `dec:produced_workerimage` edge.
    Admitted(AdmissionOutcome),
    /// Reject path — the Submission is transitioned to `rejected` and
    /// carries the `dec:produced_feedback` edge. No WorkerImage.
    Rejected(RejectionOutcome),
}

/// Materialised Admit outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionOutcome {
    /// Minted `dec:WorkerImage` (FT-086) with `eligibility_status=qualified`
    /// and `conformance_audits = [audit.iri()]`.
    pub worker_image: WorkerImage,
    /// `dec:ConformanceAudit` of class `manual-review` (ADR-060)
    /// auditing the minted WorkerImage.
    pub conformance_audit: ConformanceAudit,
    /// Submission with `lifecycle_state=admitted` and
    /// `produced_workerimage = Some(worker_image.iri())`.
    pub updated_submission: WorkerImageSubmission,
}

/// Materialised Reject outcome.
#[derive(Debug, Clone)]
pub struct RejectionOutcome {
    /// `dec:Feedback` artifact carrying the rejection rationale + evidence.
    pub feedback: Feedback,
    /// Submission with `lifecycle_state=rejected` and
    /// `produced_feedback = Some(feedback.iri.clone())`.
    pub updated_submission: WorkerImageSubmission,
}

/// Refusal modes for [`run_curator_session`].
#[derive(Debug, Error)]
pub enum CuratorSessionError {
    /// The Submission's `claimed_signature_subject` /
    /// `claimed_signature_issuer` / `claimed_source_*` claims do not
    /// pass the slice-1 admission preflight required for an admit.
    /// Currently the runner trusts the Curator's verdict here — the
    /// `signature_verdict` in the bundle is the authoritative check.
    /// This variant is reserved for future preflight failures.
    #[error("Curator preflight failed for Submission {submission_id}: {detail}")]
    AdmissionPreflightFailed {
        /// Originating Submission id.
        submission_id: String,
        /// What failed.
        detail: String,
    },
    /// Admit refused because the SignatureVerdict's class is not
    /// `valid`. The Curator MUST NOT admit a Submission whose signature
    /// did not pass FT-090's identity verification.
    #[error(
        "Curator refuses to admit Submission {submission_id}: SignatureVerdict class is {verdict_class}, expected 'valid' (FT-090)"
    )]
    AdmissionRequiresValidSignature {
        /// Originating Submission id.
        submission_id: String,
        /// The verdict class the bundle carried.
        verdict_class: &'static str,
    },
    /// One of the Curator's prose fields is empty. SHACL refuses such
    /// artifacts at write time anyway; surfacing the failure here keeps
    /// the caller's error path local.
    #[error("Curator verdict carries empty {field}; refusing to materialise outcome")]
    EmptyField {
        /// Which prose field was empty.
        field: &'static str,
    },
}

/// Materialise a Curator session's outcome from a bundle and the
/// Curator's verdict.
pub fn run_curator_session(
    bundle: &CuratorBundle,
    verdict: CuratorVerdict,
    context: &CuratorSessionContext,
) -> Result<CuratorOutcome, CuratorSessionError> {
    match verdict {
        CuratorVerdict::Admit { rationale } => {
            run_admit(bundle, rationale, context).map(CuratorOutcome::Admitted)
        }
        CuratorVerdict::Reject {
            rationale,
            disqualification_evidence,
        } => run_reject(bundle, rationale, disqualification_evidence, context)
            .map(CuratorOutcome::Rejected),
    }
}

fn run_admit(
    bundle: &CuratorBundle,
    rationale: String,
    context: &CuratorSessionContext,
) -> Result<AdmissionOutcome, CuratorSessionError> {
    require_nonempty(&rationale, "Admit.rationale")?;
    require_valid_signature_verdict(bundle)?;

    let mint_id = minted_image_id(bundle, context);
    let worker_image = build_worker_image(bundle, &mint_id);
    let audit_id = format!("{mint_id}-audit-001");
    let conformance_audit = build_conformance_audit(&audit_id, &worker_image, rationale, context);
    let worker_image_with_audit = attach_audit_link(worker_image, conformance_audit.iri());

    let mut updated_submission = bundle.submission.clone();
    updated_submission.lifecycle_state = SubmissionLifecycleState::Admitted;
    updated_submission.produced_workerimage = Some(worker_image_with_audit.iri());

    Ok(AdmissionOutcome {
        worker_image: worker_image_with_audit,
        conformance_audit,
        updated_submission,
    })
}

fn run_reject(
    bundle: &CuratorBundle,
    rationale: String,
    disqualification_evidence: String,
    context: &CuratorSessionContext,
) -> Result<RejectionOutcome, CuratorSessionError> {
    require_nonempty(&rationale, "Reject.rationale")?;
    require_nonempty(
        &disqualification_evidence,
        "Reject.disqualification_evidence",
    )?;

    let feedback_iri = NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/feedback/curator/{id}",
        id = bundle.submission.id,
    ));
    let feedback = Feedback {
        iri: feedback_iri.clone(),
        // FT-092 uses the typed CuratorOutcome::Rejected variant to
        // carry "submission rejected" semantics; the wire-level class
        // stays in the closed ADR-023 vocabulary.
        class: FeedbackClass::Defect.as_iri_value().to_string(),
        severity: Severity::Error,
        target_role: WORKER_AUTHOR_TARGET_ROLE.to_string(),
        evidence: disqualification_evidence,
        recommendation: Some(rationale),
        lifecycle_state: "produced".to_string(),
        source_session: context.session_iri.clone(),
        source_artifact: Some(bundle.submission.iri()),
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: context.in_stream.clone(),
    };

    let mut updated_submission = bundle.submission.clone();
    updated_submission.lifecycle_state = SubmissionLifecycleState::Rejected;
    updated_submission.produced_feedback = Some(feedback_iri);

    Ok(RejectionOutcome {
        feedback,
        updated_submission,
    })
}

fn minted_image_id(bundle: &CuratorBundle, context: &CuratorSessionContext) -> String {
    if let Some(id) = &context.minted_image_id_override {
        return id.clone();
    }
    bundle.submission.id.clone()
}

fn build_worker_image(bundle: &CuratorBundle, id: &str) -> WorkerImage {
    let s = &bundle.submission;
    WorkerImage {
        id: id.to_string(),
        name: format!("worker-{id}"),
        version: "1.0.0".to_string(),
        registry_ref: s.candidate_registry_ref.clone(),
        capability_tags: s.claimed_capability_tags.clone(),
        compatible_roles: s.claimed_compatible_roles.clone(),
        signed_by_subject: s.claimed_signature_subject.clone(),
        signed_by_issuer: s.claimed_signature_issuer.clone(),
        sbom_ref: s.claimed_sbom_ref.clone(),
        // Populated below by `attach_audit_link` once the audit IRI is
        // known.
        conformance_audits: Vec::new(),
        eligibility_status: EligibilityStatus::Qualified,
        source_repo_uri: s.claimed_source_repo_uri.clone(),
        source_commit_hash: s.claimed_source_commit_hash.clone(),
        build_run_url: s.claimed_build_run_url.clone(),
    }
}

fn build_conformance_audit(
    id: &str,
    image: &WorkerImage,
    notes: String,
    context: &CuratorSessionContext,
) -> ConformanceAudit {
    ConformanceAudit {
        id: id.to_string(),
        audit_class: ConformanceAuditClass::ManualReview,
        audits_image: image.iri(),
        notes,
        generated_by_session: context.session_iri.clone(),
        attributed_to_agent: context.agent_iri.clone(),
        generated_at_time: context.generated_at_time.clone(),
    }
}

fn attach_audit_link(mut image: WorkerImage, audit_iri: NamedNode) -> WorkerImage {
    if !image.conformance_audits.iter().any(|n| n == &audit_iri) {
        image.conformance_audits.push(audit_iri);
    }
    image
}

fn require_nonempty(value: &str, field: &'static str) -> Result<(), CuratorSessionError> {
    if value.trim().is_empty() {
        Err(CuratorSessionError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn require_valid_signature_verdict(bundle: &CuratorBundle) -> Result<(), CuratorSessionError> {
    use crate::identity_verifier::SignatureVerdictClass;
    if bundle.signature_verdict.verdict_class != SignatureVerdictClass::Valid {
        return Err(CuratorSessionError::AdmissionRequiresValidSignature {
            submission_id: bundle.submission.id.clone(),
            verdict_class: bundle.signature_verdict.verdict_class.as_str(),
        });
    }
    Ok(())
}
