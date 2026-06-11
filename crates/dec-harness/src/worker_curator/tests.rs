//! Unit tests for the WorkerCurator core module (FT-092).

use oxigraph::model::NamedNode;

use crate::identity_verifier::{SignatureVerdict, SignatureVerdictClass};
use dec_graph::ontology::worker_image::EligibilityStatus;
use dec_graph::ontology::worker_image_submission::{
    SubmissionLifecycleState, WorkerImageSubmission,
};

use super::{
    assemble_curator_bundle, run_curator_session, CuratorBundleError, CuratorOutcome,
    CuratorSessionContext, CuratorSessionError, CuratorVerdict, WORKER_AUTHOR_TARGET_ROLE,
    WORKER_CURATOR_AGENT_IRI,
};

const CANONICAL_DIGEST: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
const CANONICAL_SBOM_DIGEST: &str =
    "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";

fn fixture_submission(id: &str) -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: id.to_string(),
        candidate_registry_ref: format!("ghcr.io/example/{id}@sha256:{CANONICAL_DIGEST}"),
        claimed_capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref: format!("ghcr.io/example/{id}@sha256:{CANONICAL_SBOM_DIGEST}"),
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

fn fixture_signature_verdict(
    submission_iri: NamedNode,
    class: SignatureVerdictClass,
) -> SignatureVerdict {
    SignatureVerdict {
        id: "verdict-001".to_string(),
        verdict_class: class,
        rationale: "trust list match and rekor inclusion confirmed".to_string(),
        generated_by_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/session/identity-verifier-001",
        ),
        attributed_to_agent: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/agent/identity-verifier",
        ),
        generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        responds_to_submission: submission_iri,
    }
}

fn fixture_context() -> CuratorSessionContext {
    CuratorSessionContext {
        session_iri: NamedNode::new_unchecked("https://decision-cli.dev/ns/session/curator-001"),
        agent_iri: NamedNode::new_unchecked(WORKER_CURATOR_AGENT_IRI),
        generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        in_stream: NamedNode::new_unchecked(
            "https://decision-cli.dev/stream/worker-distribution-slice-1",
        ),
        minted_image_id_override: None,
    }
}

#[test]
fn assemble_refuses_mismatched_signature_verdict() {
    let sub = fixture_submission("sub-001");
    let other =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/worker-image-submission/sub-999");
    let verdict = fixture_signature_verdict(other, SignatureVerdictClass::Valid);
    let err = assemble_curator_bundle(sub, verdict, Vec::new())
        .expect_err("mismatched verdict must be refused");
    assert!(matches!(
        err,
        CuratorBundleError::SignatureVerdictMismatch { .. }
    ));
}

#[test]
fn assemble_refuses_submission_with_empty_sbom() {
    let mut sub = fixture_submission("sub-001");
    sub.claimed_sbom_ref = String::new();
    let verdict = fixture_signature_verdict(sub.iri(), SignatureVerdictClass::Valid);
    let err = assemble_curator_bundle(sub, verdict, Vec::new())
        .expect_err("empty SBOM ref must be refused via FT-091");
    assert!(matches!(err, CuratorBundleError::Sbom { .. }));
}

#[test]
fn admit_produces_workerimage_and_audit() {
    let sub = fixture_submission("sub-001");
    let verdict = fixture_signature_verdict(sub.iri(), SignatureVerdictClass::Valid);
    let bundle =
        assemble_curator_bundle(sub.clone(), verdict, Vec::new()).expect("bundle assembly");

    let outcome = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "Identity verified, SBOM present, no overlapping qualified workers."
                .to_string(),
        },
        &fixture_context(),
    )
    .expect("admit succeeds");

    let CuratorOutcome::Admitted(admit) = outcome else {
        panic!("expected Admitted outcome");
    };

    // WorkerImage is qualified, carries SBOM + signature claims, and
    // references the ConformanceAudit.
    assert_eq!(admit.worker_image.id, sub.id);
    assert_eq!(
        admit.worker_image.eligibility_status,
        EligibilityStatus::Qualified
    );
    assert_eq!(admit.worker_image.sbom_ref, sub.claimed_sbom_ref);
    assert!(admit
        .worker_image
        .conformance_audits
        .contains(&admit.conformance_audit.iri()));

    // ConformanceAudit audits the minted WorkerImage and carries the
    // Curator's rationale verbatim.
    assert_eq!(
        admit.conformance_audit.audits_image,
        admit.worker_image.iri()
    );
    assert!(admit.conformance_audit.notes.contains("Identity verified"));

    // Submission lifecycle transitioned to admitted and carries the
    // produced_workerimage edge.
    assert_eq!(
        admit.updated_submission.lifecycle_state,
        SubmissionLifecycleState::Admitted
    );
    assert_eq!(
        admit.updated_submission.produced_workerimage.as_ref(),
        Some(&admit.worker_image.iri())
    );
}

#[test]
fn admit_refuses_when_signature_verdict_is_not_valid() {
    let sub = fixture_submission("sub-001");
    let verdict = fixture_signature_verdict(sub.iri(), SignatureVerdictClass::InvalidSignature);
    let bundle = assemble_curator_bundle(sub, verdict, Vec::new()).expect("bundle assembly");

    let err = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "trying to admit invalid sig".to_string(),
        },
        &fixture_context(),
    )
    .expect_err("admit with invalid sig must refuse");
    assert!(matches!(
        err,
        CuratorSessionError::AdmissionRequiresValidSignature { .. }
    ));
}

#[test]
fn reject_produces_feedback_targeting_worker_author() {
    let sub = fixture_submission("sub-001");
    let verdict = fixture_signature_verdict(sub.iri(), SignatureVerdictClass::UntrustedIdentity);
    let bundle =
        assemble_curator_bundle(sub.clone(), verdict, Vec::new()).expect("bundle assembly");

    let outcome = run_curator_session(
        &bundle,
        CuratorVerdict::Reject {
            rationale: "Untrusted signer; identity not on trust list.".to_string(),
            disqualification_evidence: "SignatureVerdict class=untrusted-identity".to_string(),
        },
        &fixture_context(),
    )
    .expect("reject succeeds");

    let CuratorOutcome::Rejected(reject) = outcome else {
        panic!("expected Rejected outcome");
    };

    assert_eq!(reject.feedback.target_role, WORKER_AUTHOR_TARGET_ROLE);
    assert_eq!(
        reject.feedback.source_artifact.as_ref(),
        Some(&sub.iri()),
        "rejection feedback must point at the rejected Submission"
    );
    assert!(reject.feedback.evidence.contains("untrusted-identity"));
    assert!(reject
        .feedback
        .recommendation
        .as_deref()
        .unwrap_or("")
        .contains("Untrusted signer"));

    assert_eq!(
        reject.updated_submission.lifecycle_state,
        SubmissionLifecycleState::Rejected
    );
    assert_eq!(
        reject.updated_submission.produced_feedback,
        Some(reject.feedback.iri.clone())
    );
}

#[test]
fn empty_rationale_is_refused() {
    let sub = fixture_submission("sub-001");
    let verdict = fixture_signature_verdict(sub.iri(), SignatureVerdictClass::Valid);
    let bundle = assemble_curator_bundle(sub, verdict, Vec::new()).expect("bundle assembly");

    let err = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "   ".to_string(),
        },
        &fixture_context(),
    )
    .expect_err("empty rationale must refuse");
    assert!(matches!(
        err,
        CuratorSessionError::EmptyField {
            field: "Admit.rationale"
        }
    ));
}
