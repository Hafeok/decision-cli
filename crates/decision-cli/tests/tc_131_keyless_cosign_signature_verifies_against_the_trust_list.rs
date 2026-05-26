//! TC-131 — Keyless cosign signature verifies against the trust list.
//!
//! Validates: FT-089 · ADR-058.
//! Spec: `.product/tests/TC-131-keyless-cosign-signature-verifies-against-the-trus.md`
//!
//! Pins down the end-to-end behaviour of the cosign trust-list matcher
//! shipped under `core::cosign_trust`, against the three success
//! criteria the FT-089 feature_spec declares:
//!
//! 1. **A release workflow signs an image keyless and the resulting
//!    Submission carries Fulcio identity + Rekor entry pointers that the
//!    verifier resolves and accepts.** Modelled here as: a
//!    `SignatureIdentity` whose `(subject, issuer)` matches a trust list
//!    entry pinning the expected `(repo, workflow path, tag pattern)` is
//!    admitted; a Rekor entry reference with the cosign-canonical
//!    `(https URL, 64 lowercase hex UUID)` shape validates.
//! 2. **A signature from an identity NOT on the trust list is rejected**
//!    — the matcher's error maps directly to the FT-090 verdict class
//!    `untrusted-identity`.
//! 3. **A submission claiming a Rekor entry that doesn't exist or doesn't
//!    match is rejected** with verdict `rekor-entry-missing` — this test
//!    pins the *syntactic* refusal path (insecure URL, malformed UUID);
//!    the network-side existence check is FT-090's responsibility.
//!
//! Per ADR-013 / ADR-014 we keep this file under the function/file
//! limits by routing through small per-claim helpers and consolidating
//! every success-criterion check into the single named test the
//! product-cli runner identifies as the TC-131 entry point.

use decision_cli::core::cosign_trust::{
    match_signature_identity, parse_github_actions_subject, validate_rekor_entry_ref,
    IdentityMatchError, RekorEntryRef, RekorRefError, SignatureIdentity, SubjectParseError,
    TagPattern, TrustList, TrustListEntry, TrustOrigin, GITHUB_ACTIONS_ISSUER_URI,
    LOCAL_KEY_ISSUER_SENTINEL,
};

const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release-worker.yml";

fn implementer_release_entry() -> TrustListEntry {
    TrustListEntry {
        origin: TrustOrigin::GithubActions {
            repo: "Hafeok/decision-cli-worker-implementer".to_string(),
            workflow_path: RELEASE_WORKFLOW_PATH.to_string(),
            tag_pattern: TagPattern::parse("implementer-v*.*.*").unwrap(),
        },
        issuer: GITHUB_ACTIONS_ISSUER_URI.to_string(),
        note: Some("implementer worker release line".to_string()),
    }
}

fn dev_local_key_entry() -> TrustListEntry {
    TrustListEntry {
        origin: TrustOrigin::LocalKey {
            subject: "operator@decision-cli.local".to_string(),
        },
        issuer: LOCAL_KEY_ISSUER_SENTINEL.to_string(),
        note: Some("operator dev key (slice 1 fallback)".to_string()),
    }
}

fn admitted_identity() -> SignatureIdentity {
    SignatureIdentity::new(
        format!(
            "https://github.com/Hafeok/decision-cli-worker-implementer/{RELEASE_WORKFLOW_PATH}@refs/tags/implementer-v1.2.3"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    )
}

fn well_formed_rekor_ref() -> RekorEntryRef {
    // 64 lowercase hex digits — the cosign-canonical Rekor entry UUID shape.
    RekorEntryRef::sigstore_default(
        "a".repeat(64),
        Some(987_654_321),
    )
}

/// Success criterion 1: matching identity + well-formed Rekor reference
/// together constitute the "admit" path the WorkerImageSubmission flow
/// follows when a release workflow signed the image keyless.
fn admits_well_formed_keyless_signature() {
    let trust = TrustList::from_entries(vec![implementer_release_entry()])
        .expect("trust list construction must succeed");

    let outcome = match_signature_identity(&admitted_identity(), &trust)
        .expect("well-formed keyless signature must match the trust list");
    assert_eq!(outcome.entry_index, 0);
    assert_eq!(outcome.note.as_deref(), Some("implementer worker release line"));

    validate_rekor_entry_ref(&well_formed_rekor_ref())
        .expect("well-formed Rekor entry reference must validate");

    // Round-trip: the OIDC subject we admit must parse back into the
    // axes the trust list entry pinned — the matcher must agree with
    // the standalone parser.
    let parsed = parse_github_actions_subject(&admitted_identity().subject)
        .expect("admitted subject must parse as GitHub Actions identity");
    assert_eq!(parsed.repo, "Hafeok/decision-cli-worker-implementer");
    assert_eq!(parsed.workflow_path, RELEASE_WORKFLOW_PATH);
    assert_eq!(parsed.tag(), Some("implementer-v1.2.3"));
}

/// Success criterion 2: any axis mismatch (repo, workflow path, tag,
/// issuer) maps to the `untrusted-identity` verdict shape FT-090 will
/// emit. We exercise each axis once.
fn rejects_signature_from_off_list_identity() {
    let trust = TrustList::from_entries(vec![implementer_release_entry()])
        .expect("trust list construction must succeed");

    // Wrong repo.
    let wrong_repo = SignatureIdentity::new(
        format!(
            "https://github.com/attacker/fork/{RELEASE_WORKFLOW_PATH}@refs/tags/implementer-v1.2.3"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    assert_no_admission(match_signature_identity(&wrong_repo, &trust));

    // Wrong workflow path.
    let wrong_workflow = SignatureIdentity::new(
        "https://github.com/Hafeok/decision-cli-worker-implementer/.github/workflows/sneaky.yml@refs/tags/implementer-v1.2.3".to_string(),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    assert_no_admission(match_signature_identity(&wrong_workflow, &trust));

    // Wrong tag pattern.
    let wrong_tag = SignatureIdentity::new(
        format!(
            "https://github.com/Hafeok/decision-cli-worker-implementer/{RELEASE_WORKFLOW_PATH}@refs/tags/verifier-v1.2.3"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    assert_no_admission(match_signature_identity(&wrong_tag, &trust));

    // Branch ref (no tag) — entry requires a tag pattern; branch is
    // structurally inadmissible.
    let branch_ref = SignatureIdentity::new(
        format!(
            "https://github.com/Hafeok/decision-cli-worker-implementer/{RELEASE_WORKFLOW_PATH}@refs/heads/main"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    assert_no_admission(match_signature_identity(&branch_ref, &trust));

    // Wrong issuer — this is the IssuerMismatch path: matcher discards
    // every entry before evaluating any pin, and reports the issuer
    // axis specifically so the operator can see it was the issuer
    // that failed.
    let wrong_issuer = SignatureIdentity::new(
        admitted_identity().subject,
        "https://attacker.example/issuer",
    );
    match match_signature_identity(&wrong_issuer, &trust) {
        Err(IdentityMatchError::IssuerMismatch { issuer }) => {
            assert_eq!(issuer, "https://attacker.example/issuer");
        }
        other => panic!("expected IssuerMismatch, got {other:?}"),
    }
}

fn assert_no_admission(result: Result<impl std::fmt::Debug, IdentityMatchError>) {
    match result {
        Err(IdentityMatchError::NoEntryAdmitsSubject { .. })
        | Err(IdentityMatchError::IssuerMismatch { .. }) => {}
        Err(other) => panic!("expected admission failure, got unexpected error: {other:?}"),
        Ok(outcome) => panic!("expected admission failure, got outcome: {outcome:?}"),
    }
}

/// Success criterion 3 (syntactic half): a Rekor entry reference that
/// is obviously malformed is refused before any network call, so the
/// FT-090 verifier never spends a round-trip on a typo. The verdict
/// class the orchestrator then renders is `rekor-entry-missing`.
fn rejects_malformed_rekor_entry_references() {
    // Empty URL.
    let empty_url = RekorEntryRef {
        rekor_url: String::new(),
        entry_uuid: "0".repeat(64),
        log_index: None,
    };
    assert!(matches!(
        validate_rekor_entry_ref(&empty_url),
        Err(RekorRefError::EmptyRekorUrl)
    ));

    // Non-TLS URL — Rekor inclusion proofs only mean anything over TLS.
    let http_url = RekorEntryRef {
        rekor_url: "http://rekor.example".to_string(),
        entry_uuid: "0".repeat(64),
        log_index: None,
    };
    assert!(matches!(
        validate_rekor_entry_ref(&http_url),
        Err(RekorRefError::InsecureRekorUrl { .. })
    ));

    // Wrong-length UUID.
    let short_uuid = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "abc123".to_string(),
        log_index: None,
    };
    assert!(matches!(
        validate_rekor_entry_ref(&short_uuid),
        Err(RekorRefError::MalformedEntryUuid { .. })
    ));

    // Non-hex UUID.
    let non_hex = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "z".repeat(64),
        log_index: None,
    };
    assert!(matches!(
        validate_rekor_entry_ref(&non_hex),
        Err(RekorRefError::MalformedEntryUuid { .. })
    ));

    // Uppercase-hex UUID — cosign emits lowercase; uppercase is a
    // transcription error worth catching early.
    let upper_hex = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "A".repeat(64),
        log_index: None,
    };
    assert!(matches!(
        validate_rekor_entry_ref(&upper_hex),
        Err(RekorRefError::MalformedEntryUuid { .. })
    ));
}

/// Local-key fallback path (ADR-058 §"Local key-based signing remains a
/// supported fallback"): an operator-enrolled local-key entry admits the
/// matching subject and refuses anything else.
fn local_key_fallback_is_admitted_only_when_explicitly_enrolled() {
    let trust = TrustList::from_entries(vec![dev_local_key_entry()])
        .expect("trust list with local-key entry must construct");

    let enrolled = SignatureIdentity::new("operator@decision-cli.local", LOCAL_KEY_ISSUER_SENTINEL);
    let outcome = match_signature_identity(&enrolled, &trust)
        .expect("enrolled local key identity must match");
    assert_eq!(outcome.entry_index, 0);

    let stranger = SignatureIdentity::new("intruder@decision-cli.local", LOCAL_KEY_ISSUER_SENTINEL);
    assert_no_admission(match_signature_identity(&stranger, &trust));
}

/// An empty trust list rejects every candidate — the EmptyTrustList
/// variant exists so an operator running with no enrolled identities
/// (e.g. a fresh `.dec/` bootstrap before policy is curated) gets a
/// structured failure rather than a silent admit.
fn empty_trust_list_admits_nothing() {
    let trust = TrustList::empty();
    let result = match_signature_identity(&admitted_identity(), &trust);
    assert!(matches!(result, Err(IdentityMatchError::EmptyTrustList)));
}

/// The standalone parser refuses obviously non-GitHub-Actions subjects
/// up-front, so the trust-list matcher can route them through the
/// local-key fallback path without paying a parse cost.
fn non_github_subject_is_unparseable_as_github_actions() {
    let err = parse_github_actions_subject("operator@example.com")
        .expect_err("non-GitHub subject must not parse as GitHub Actions identity");
    assert_eq!(err, SubjectParseError::NotGithubActions);
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-131 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs each per-claim helper so this one function reproduces
/// the exit-criterion end-to-end and the runner sees a single test
/// target it can name in `runner-args`.
#[test]
fn tc_131_keyless_cosign_signature_verifies_against_the_trust_list() {
    admits_well_formed_keyless_signature();
    rejects_signature_from_off_list_identity();
    rejects_malformed_rekor_entry_references();
    local_key_fallback_is_admitted_only_when_explicitly_enrolled();
    empty_trust_list_admits_nothing();
    non_github_subject_is_unparseable_as_github_actions();
}
