//! TC-134 — WorkerCurator session admits a clean Submission and rejects a flawed one.
//!
//! Validates: FT-092 · ADR-060 · ADR-055 · ADR-022 · ADR-038 · ADR-040.
//! Spec: `.product/tests/TC-134-workercurator-session-admits-a-clean-submission-an.md`
//!
//! Three claims this integration test pins down end-to-end:
//!
//! 1. **Admission.** A Curator session given a complete bundle
//!    (Submission + `valid` SignatureVerdict + canonical SBOM
//!    referrer) produces a `dec:WorkerImage` with
//!    `eligibility_status=qualified` and a `dec:ConformanceAudit` of
//!    class `manual-review` linked back to the new image — both
//!    materialise atomically into an Oxigraph store. The Submission
//!    transitions `received → admitted` with the
//!    `dec:produced_workerimage` edge stamped. The admitted
//!    WorkerImage's motivational chain terminates at the Submission's
//!    BoundaryArtifact `dec:external_origin`, satisfying FT-092
//!    success criterion #3.
//!
//! 2. **Rejection.** A Curator session given a flawed bundle
//!    (Submission with an `invalid-signature` SignatureVerdict)
//!    produces a `dec:Feedback` artifact rooted at the Submission and
//!    leaves NO `dec:WorkerImage` in the catalog. The Submission
//!    transitions `received → rejected` with the
//!    `dec:produced_feedback` edge stamped.
//!
//! 3. **Refusal preconditions.** Admit refuses a Submission whose
//!    SignatureVerdict is not `valid` (FT-090 admission gate), and the
//!    bundle assembler refuses a Submission whose SignatureVerdict
//!    responds to a different Submission (no cross-talk between
//!    dispatches).

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::identity_verifier::{SignatureVerdict, SignatureVerdictClass};
use decision_cli::core::ontology::worker_image::EligibilityStatus;
use decision_cli::core::ontology::worker_image_submission::{
    SubmissionLifecycleState, WorkerImageSubmission,
};
use decision_cli::core::worker_curator::{
    assemble_curator_bundle, run_curator_session, CuratorBundleError, CuratorOutcome,
    CuratorSessionContext, CuratorSessionError, CuratorVerdict, WORKER_AUTHOR_TARGET_ROLE,
    WORKER_CURATOR_AGENT_IRI,
};
use decision_cli::vocab::{
    bundle_graph, worker_image_graph, worker_image_submission_graph,
    IRI_DEC_CONFORMANCE_AUDIT_CLASS, IRI_DEC_FEEDBACK, IRI_DEC_PRODUCED_FEEDBACK,
    IRI_DEC_PRODUCED_WORKERIMAGE, IRI_DEC_WORKER_IMAGE, IRI_DEC_WORKER_IMAGE_SUBMISSION,
};

const CANDIDATE_DIGEST: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
const SBOM_DIGEST: &str = "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";

const EXTERNAL_ORIGIN_IRI: &str = "https://decision-cli.dev/ns#external_origin";

const STREAM_IRI: &str = "https://decision-cli.dev/stream/worker-distribution-slice-1";

fn fixture_submission(id: &str) -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: id.to_string(),
        candidate_registry_ref: format!("ghcr.io/example/{id}@sha256:{CANDIDATE_DIGEST}"),
        claimed_capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref: format!("ghcr.io/example/{id}@sha256:{SBOM_DIGEST}"),
        claimed_signature_subject: format!(
            "https://github.com/example/{id}/.github/workflows/release.yml@refs/tags/v1.0.0"
        ),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: format!("https://github.com/example/{id}"),
        claimed_source_commit_hash: "abc123def4567890abcdef0123456789abcdef01".to_string(),
        claimed_build_run_url: format!("https://github.com/example/{id}/actions/runs/1"),
        lifecycle_state: SubmissionLifecycleState::Received,
        external_origin: format!(
            "brief:worker-distribution-slice-1/github-actions:example/{id}/runs/1"
        ),
        produced_workerimage: None,
        produced_feedback: None,
    }
}

fn fixture_signature_verdict(
    id: &str,
    submission_iri: NamedNode,
    class: SignatureVerdictClass,
) -> SignatureVerdict {
    SignatureVerdict {
        id: id.to_string(),
        verdict_class: class,
        rationale: "trust list match and rekor inclusion confirmed".to_string(),
        generated_by_session: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/session/identity-verifier-{id}"
        )),
        attributed_to_agent: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/agent/identity-verifier",
        ),
        generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        responds_to_submission: submission_iri,
    }
}

fn fixture_context(session_label: &str) -> CuratorSessionContext {
    CuratorSessionContext {
        session_iri: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/session/curator-{session_label}"
        )),
        agent_iri: NamedNode::new_unchecked(WORKER_CURATOR_AGENT_IRI),
        generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        in_stream: NamedNode::new_unchecked(STREAM_IRI),
        minted_image_id_override: None,
    }
}

/// Persist every artifact a Curator session emits on the admit path into
/// an in-memory Oxigraph store and assert the structural claims hold
/// after the writes.
fn drive_admit_into_store(id: &str) -> (Store, NamedNode, NamedNode, NamedNode) {
    let sub = fixture_submission(id);
    let verdict = fixture_signature_verdict("valid-001", sub.iri(), SignatureVerdictClass::Valid);
    let bundle =
        assemble_curator_bundle(sub.clone(), verdict, Vec::new()).expect("admit bundle assembly");

    let outcome = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "Identity verified (valid trust list match); SBOM referrer attached; \
                        no overlapping qualified workers in the catalog."
                .to_string(),
        },
        &fixture_context("admit"),
    )
    .expect("admit session succeeds");

    let CuratorOutcome::Admitted(admit) = outcome else {
        panic!("expected Admitted outcome");
    };

    let store = Store::new().expect("memory store");
    for q in admit.worker_image.to_quads(worker_image_graph()) {
        store.insert(&q).expect("insert worker image quad");
    }
    for q in admit.conformance_audit.to_quads(bundle_graph()) {
        store.insert(&q).expect("insert conformance audit quad");
    }
    for q in admit
        .updated_submission
        .to_quads(worker_image_submission_graph())
    {
        store.insert(&q).expect("insert updated submission quad");
    }

    (
        store,
        admit.worker_image.iri(),
        admit.conformance_audit.iri(),
        admit.updated_submission.iri(),
    )
}

/// Drive a rejection path into a store. Returns (store, feedback_iri,
/// submission_iri).
fn drive_reject_into_store(id: &str) -> (Store, NamedNode, NamedNode) {
    let sub = fixture_submission(id);
    // SignatureVerdict can be untrusted-identity — the bundle assembler
    // doesn't care about the verdict class, the session runner cares
    // only on the admit path.
    let verdict = fixture_signature_verdict(
        "untrusted-001",
        sub.iri(),
        SignatureVerdictClass::UntrustedIdentity,
    );
    let bundle =
        assemble_curator_bundle(sub.clone(), verdict, Vec::new()).expect("reject bundle assembly");

    let outcome = run_curator_session(
        &bundle,
        CuratorVerdict::Reject {
            rationale: "Untrusted signer: signing identity is not on the operator's trust list."
                .to_string(),
            disqualification_evidence:
                "SignatureVerdict class=untrusted-identity per FT-090 classification".to_string(),
        },
        &fixture_context("reject"),
    )
    .expect("reject session succeeds");

    let CuratorOutcome::Rejected(reject) = outcome else {
        panic!("expected Rejected outcome");
    };

    let store = Store::new().expect("memory store");
    for q in reject.feedback.to_quads(bundle_graph()) {
        store.insert(&q).expect("insert feedback quad");
    }
    for q in reject
        .updated_submission
        .to_quads(worker_image_submission_graph())
    {
        store.insert(&q).expect("insert updated submission quad");
    }

    (
        store,
        reject.feedback.iri.clone(),
        reject.updated_submission.iri(),
    )
}

fn count_typed_subjects(store: &Store, class_iri: &str) -> usize {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT (COUNT(DISTINCT ?s) AS ?n) WHERE {{ GRAPH ?g {{ ?s a <{class_iri}> }} }}",
    );
    let QueryResults::Solutions(mut sols) = store.query(q.as_str()).expect("query ok") else {
        panic!("expected solution stream");
    };
    let sol = sols.next().expect("at least one row").expect("solution");
    if let Some(Term::Literal(lit)) = sol.get("n") {
        lit.value().parse::<usize>().expect("integer count")
    } else {
        panic!("expected integer literal in ?n")
    }
}

fn submission_carries_external_origin(store: &Store, sub_iri: &NamedNode) -> bool {
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{sub}> <{ext}> ?o . FILTER(STR(?o) != \"\") }} }}",
        sub = sub_iri.as_str(),
        ext = EXTERNAL_ORIGIN_IRI,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

#[test]
fn admit_produces_workerimage_and_conformance_audit_in_store() {
    let (store, image_iri, audit_iri, sub_iri) = drive_admit_into_store("sub-tc-134-admit");

    // Exactly one WorkerImage, one ConformanceAudit, one Submission.
    assert_eq!(count_typed_subjects(&store, IRI_DEC_WORKER_IMAGE), 1);
    assert_eq!(
        count_typed_subjects(&store, IRI_DEC_CONFORMANCE_AUDIT_CLASS),
        1
    );
    assert_eq!(
        count_typed_subjects(&store, IRI_DEC_WORKER_IMAGE_SUBMISSION),
        1
    );

    // WorkerImage is qualified.
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s WHERE {{ GRAPH ?g {{ <{img}> dec:eligibility_status ?s }} }}",
        img = image_iri.as_str(),
    );
    let QueryResults::Solutions(mut sols) = store.query(q.as_str()).expect("query") else {
        panic!("expected solutions");
    };
    let sol = sols.next().expect("solution row").expect("solution");
    let Some(Term::Literal(lit)) = sol.get("s") else {
        panic!("expected literal");
    };
    assert_eq!(lit.value(), EligibilityStatus::Qualified.as_str());

    // ConformanceAudit's dec:audits points at the WorkerImage.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{audit}> <https://decision-cli.dev/ns#audits> <{img}> }} }}",
        audit = audit_iri.as_str(),
        img = image_iri.as_str(),
    );
    assert!(matches!(
        store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));

    // Submission lifecycle is admitted and carries produced_workerimage.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{sub}> <{pw}> <{img}> }} }}",
        sub = sub_iri.as_str(),
        pw = IRI_DEC_PRODUCED_WORKERIMAGE,
        img = image_iri.as_str(),
    );
    assert!(matches!(
        store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));

    // FT-092 success criterion #3: the Submission carries external_origin,
    // which terminates the WorkerImage's motivational chain at the
    // BoundaryArtifact per FT-071 / ADR-040.
    assert!(submission_carries_external_origin(&store, &sub_iri));
}

#[test]
fn reject_produces_feedback_and_leaves_no_workerimage_in_catalog() {
    let (store, feedback_iri, sub_iri) = drive_reject_into_store("sub-tc-134-reject");

    // Exactly one Feedback; ZERO WorkerImages and ZERO ConformanceAudits.
    assert_eq!(count_typed_subjects(&store, IRI_DEC_FEEDBACK), 1);
    assert_eq!(
        count_typed_subjects(&store, IRI_DEC_WORKER_IMAGE),
        0,
        "rejected Submission MUST NOT mint a WorkerImage"
    );
    assert_eq!(
        count_typed_subjects(&store, IRI_DEC_CONFORMANCE_AUDIT_CLASS),
        0
    );

    // Feedback's source_artifact is the rejected Submission.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{f}> <https://decision-cli.dev/ns#sourceArtifact> <{s}> }} }}",
        f = feedback_iri.as_str(),
        s = sub_iri.as_str(),
    );
    assert!(
        matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true))),
        "rejection Feedback must cite the rejected Submission via dec:sourceArtifact"
    );

    // Feedback's targetRole routes to the worker author lane.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{f}> <https://decision-cli.dev/ns#targetRole> \"{role}\" }} }}",
        f = feedback_iri.as_str(),
        role = WORKER_AUTHOR_TARGET_ROLE,
    );
    assert!(matches!(
        store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));

    // Submission lifecycle is rejected and carries produced_feedback.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{s}> <{pf}> <{f}> }} }}",
        s = sub_iri.as_str(),
        pf = IRI_DEC_PRODUCED_FEEDBACK,
        f = feedback_iri.as_str(),
    );
    assert!(matches!(
        store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));
}

#[test]
fn admit_refuses_when_signature_verdict_is_invalid() {
    let sub = fixture_submission("sub-bad-sig");
    let verdict = fixture_signature_verdict(
        "invalid-001",
        sub.iri(),
        SignatureVerdictClass::InvalidSignature,
    );
    let bundle = assemble_curator_bundle(sub, verdict, Vec::new()).expect("bundle assembly");
    let err = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "trying to admit invalid sig — should refuse".to_string(),
        },
        &fixture_context("admit-fails"),
    )
    .expect_err("Curator must refuse admission when signature verdict is not valid");
    assert!(matches!(
        err,
        CuratorSessionError::AdmissionRequiresValidSignature { .. }
    ));
}

#[test]
fn assemble_refuses_signature_verdict_for_other_submission() {
    let sub = fixture_submission("sub-001");
    let other_submission_iri =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/worker-image-submission/sub-other");
    let verdict = fixture_signature_verdict(
        "wrong-target-001",
        other_submission_iri,
        SignatureVerdictClass::Valid,
    );
    let err = assemble_curator_bundle(sub, verdict, Vec::new())
        .expect_err("verdict for a different submission must be refused");
    assert!(matches!(
        err,
        CuratorBundleError::SignatureVerdictMismatch { .. }
    ));
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-134 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs the structural claims of TC-134 end-to-end:
///
/// 1. admit produces WorkerImage (qualified) + ConformanceAudit (manual-review)
///    and transitions the Submission to admitted with produced_workerimage stamped;
/// 2. reject produces Feedback, leaves no WorkerImage, and transitions the
///    Submission to rejected with produced_feedback stamped;
/// 3. admit refuses an invalid SignatureVerdict;
/// 4. bundle assembly refuses a SignatureVerdict bound to a different Submission.
#[test]
fn tc_134_workercurator_session_admits_a_clean_submission_an() {
    // (1) Admit path.
    let (admit_store, image_iri, audit_iri, admit_sub_iri) =
        drive_admit_into_store("sub-tc-134-admit");
    assert_eq!(count_typed_subjects(&admit_store, IRI_DEC_WORKER_IMAGE), 1);
    assert_eq!(
        count_typed_subjects(&admit_store, IRI_DEC_CONFORMANCE_AUDIT_CLASS),
        1
    );
    // WorkerImage is qualified.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{img}> <https://decision-cli.dev/ns#eligibility_status> \"qualified\" }} }}",
        img = image_iri.as_str(),
    );
    assert!(matches!(
        admit_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));
    // ConformanceAudit class is manual-review.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{audit}> <https://decision-cli.dev/ns#audit_class> \"manual-review\" }} }}",
        audit = audit_iri.as_str(),
    );
    assert!(matches!(
        admit_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));
    // Submission carries produced_workerimage edge AND external_origin
    // (motivational chain termination per FT-092 success criterion #3).
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{sub}> <{pw}> <{img}> }} }}",
        sub = admit_sub_iri.as_str(),
        pw = IRI_DEC_PRODUCED_WORKERIMAGE,
        img = image_iri.as_str(),
    );
    assert!(matches!(
        admit_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));
    assert!(submission_carries_external_origin(
        &admit_store,
        &admit_sub_iri
    ));

    // The Submission lifecycle is admitted — ASK over the lifecycle literal.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{sub}> <https://decision-cli.dev/ns#submission_lifecycle_state> \"admitted\" }} }}",
        sub = admit_sub_iri.as_str(),
    );
    assert!(matches!(
        admit_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));

    // (2) Reject path.
    let (reject_store, feedback_iri, reject_sub_iri) = drive_reject_into_store("sub-tc-134-reject");
    assert_eq!(count_typed_subjects(&reject_store, IRI_DEC_FEEDBACK), 1);
    assert_eq!(
        count_typed_subjects(&reject_store, IRI_DEC_WORKER_IMAGE),
        0,
        "rejection MUST NOT leave any WorkerImage in the catalog"
    );
    // Feedback targets the worker author and cites the Submission.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{f}> <https://decision-cli.dev/ns#targetRole> \"{role}\" ; \
                            <https://decision-cli.dev/ns#sourceArtifact> <{s}> }} }}",
        f = feedback_iri.as_str(),
        role = WORKER_AUTHOR_TARGET_ROLE,
        s = reject_sub_iri.as_str(),
    );
    assert!(matches!(
        reject_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));
    // The Submission lifecycle is rejected.
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{sub}> <https://decision-cli.dev/ns#submission_lifecycle_state> \"rejected\" }} }}",
        sub = reject_sub_iri.as_str(),
    );
    assert!(matches!(
        reject_store.query(q.as_str()),
        Ok(QueryResults::Boolean(true))
    ));

    // (3) Admit refuses an invalid SignatureVerdict.
    let sub = fixture_submission("sub-bad-sig");
    let verdict = fixture_signature_verdict(
        "invalid-001",
        sub.iri(),
        SignatureVerdictClass::InvalidSignature,
    );
    let bundle = assemble_curator_bundle(sub, verdict, Vec::new()).expect("bundle assembly");
    let err = run_curator_session(
        &bundle,
        CuratorVerdict::Admit {
            rationale: "trying to admit invalid sig".to_string(),
        },
        &fixture_context("admit-fails"),
    )
    .expect_err("admit must refuse invalid sig");
    assert!(matches!(
        err,
        CuratorSessionError::AdmissionRequiresValidSignature { .. }
    ));

    // (4) Bundle assembly refuses a SignatureVerdict whose
    // `dec:respondsTo` points at a different Submission.
    let sub = fixture_submission("sub-x");
    let other =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/worker-image-submission/sub-other");
    let verdict = fixture_signature_verdict("wrong-001", other, SignatureVerdictClass::Valid);
    let err = assemble_curator_bundle(sub, verdict, Vec::new())
        .expect_err("verdict bound to other submission must be refused");
    assert!(matches!(
        err,
        CuratorBundleError::SignatureVerdictMismatch { .. }
    ));
}
