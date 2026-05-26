//! Unit tests for the cosign keyless trust list (FT-089 / ADR-058).

use super::*;

fn ghactions_entry(repo: &str, workflow: &str, tag: &str) -> TrustListEntry {
    TrustListEntry {
        origin: TrustOrigin::GithubActions {
            repo: repo.to_string(),
            workflow_path: workflow.to_string(),
            tag_pattern: TagPattern::parse(tag).expect("valid tag pattern in test"),
        },
        issuer: GITHUB_ACTIONS_ISSUER_URI.to_string(),
        note: None,
    }
}

fn local_key_entry(subject: &str) -> TrustListEntry {
    TrustListEntry {
        origin: TrustOrigin::LocalKey {
            subject: subject.to_string(),
        },
        issuer: LOCAL_KEY_ISSUER_SENTINEL.to_string(),
        note: None,
    }
}

fn keyless_subject(repo: &str, workflow: &str, git_ref: &str) -> String {
    format!("https://github.com/{repo}/{workflow}@{git_ref}")
}

// ---------- TagPattern ----------

#[test]
fn tag_pattern_literal_matches_only_exact_tag() {
    let p = TagPattern::parse("v1.2.3").unwrap();
    assert!(p.matches("v1.2.3"));
    assert!(!p.matches("v1.2.4"));
    assert!(!p.matches("v1.2"));
}

#[test]
fn tag_pattern_wildcard_in_segment_matches_zero_or_more_chars() {
    let p = TagPattern::parse("implementer-v*.*.*").unwrap();
    assert!(p.matches("implementer-v1.2.3"));
    assert!(p.matches("implementer-v10.20.30"));
    assert!(p.matches("implementer-v0.0.0"));
    // Insufficient segments.
    assert!(!p.matches("implementer-v1.2"));
    // Wrong prefix.
    assert!(!p.matches("verifier-v1.2.3"));
}

#[test]
fn tag_pattern_rejects_extra_segments() {
    let p = TagPattern::parse("v*.*").unwrap();
    assert!(p.matches("v1.2"));
    assert!(!p.matches("v1.2.3"));
}

#[test]
fn tag_pattern_empty_rejected() {
    let err = TagPattern::parse("").unwrap_err();
    assert_eq!(err, TagPatternError::Empty);
}

#[test]
fn tag_pattern_empty_segment_rejected() {
    let err = TagPattern::parse("v1..3").unwrap_err();
    assert_eq!(err, TagPatternError::EmptySegment);
}

// ---------- parse_github_actions_subject ----------

#[test]
fn parses_well_formed_tag_subject() {
    let subj = keyless_subject(
        "example/worker",
        ".github/workflows/release.yml",
        "refs/tags/v1.2.3",
    );
    let parsed = parse_github_actions_subject(&subj).unwrap();
    assert_eq!(parsed.repo, "example/worker");
    assert_eq!(parsed.workflow_path, ".github/workflows/release.yml");
    assert_eq!(parsed.git_ref, "refs/tags/v1.2.3");
    assert_eq!(parsed.tag(), Some("v1.2.3"));
    assert!(parsed.is_tag_ref());
}

#[test]
fn parses_well_formed_branch_subject() {
    let subj = keyless_subject(
        "example/worker",
        ".github/workflows/release.yml",
        "refs/heads/main",
    );
    let parsed = parse_github_actions_subject(&subj).unwrap();
    assert_eq!(parsed.tag(), None);
    assert!(!parsed.is_tag_ref());
}

#[test]
fn rejects_subject_without_github_prefix() {
    let err = parse_github_actions_subject("user@example.com").unwrap_err();
    assert_eq!(err, SubjectParseError::NotGithubActions);
}

#[test]
fn rejects_subject_missing_at_separator() {
    let err =
        parse_github_actions_subject("https://github.com/example/worker/.github/workflows/release.yml")
            .unwrap_err();
    assert_eq!(err, SubjectParseError::MissingRefSeparator);
}

#[test]
fn rejects_subject_missing_workflow_path() {
    let err =
        parse_github_actions_subject("https://github.com/example/worker@refs/tags/v1").unwrap_err();
    assert_eq!(err, SubjectParseError::MissingWorkflowPath);
}

#[test]
fn rejects_subject_with_empty_ref() {
    let err = parse_github_actions_subject(
        "https://github.com/example/worker/.github/workflows/release.yml@",
    )
    .unwrap_err();
    assert_eq!(err, SubjectParseError::EmptyRef);
}

// ---------- TrustList construction ----------

#[test]
fn rejects_entry_with_empty_issuer() {
    let mut e = ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    );
    e.issuer = String::new();
    let err = TrustList::from_entries(vec![e]).unwrap_err();
    assert!(matches!(err, TrustListError::EmptyIssuer { index: 0 }));
}

#[test]
fn rejects_entry_with_malformed_repo() {
    let e = TrustListEntry {
        origin: TrustOrigin::GithubActions {
            repo: "no-slash-here".to_string(),
            workflow_path: ".github/workflows/release.yml".to_string(),
            tag_pattern: TagPattern::parse("v*").unwrap(),
        },
        issuer: GITHUB_ACTIONS_ISSUER_URI.to_string(),
        note: None,
    };
    let err = TrustList::from_entries(vec![e]).unwrap_err();
    assert!(matches!(err, TrustListError::MalformedRepo { index: 0, .. }));
}

#[test]
fn rejects_local_key_entry_with_empty_subject() {
    let e = local_key_entry("");
    let err = TrustList::from_entries(vec![e]).unwrap_err();
    assert!(matches!(
        err,
        TrustListError::EmptyLocalKeySubject { index: 0 }
    ));
}

// ---------- match_signature_identity ----------

#[test]
fn empty_trust_list_rejects_every_candidate() {
    let tl = TrustList::empty();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.2.3",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert_eq!(err, IdentityMatchError::EmptyTrustList);
}

#[test]
fn admits_matching_github_actions_identity() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.2.3",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let outcome = match_signature_identity(&cand, &tl).expect("must match");
    assert_eq!(outcome.entry_index, 0);
}

#[test]
fn rejects_identity_from_wrong_repo() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "attacker/fork",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.2.3",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::NoEntryAdmitsSubject { .. }));
}

#[test]
fn rejects_identity_from_wrong_workflow_path() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/sneaky.yml",
            "refs/tags/implementer-v1.2.3",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::NoEntryAdmitsSubject { .. }));
}

#[test]
fn rejects_identity_with_non_matching_tag_pattern() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/verifier-v1.2.3",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::NoEntryAdmitsSubject { .. }));
}

#[test]
fn rejects_branch_ref_when_entry_requires_tag_pattern() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/heads/main",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::NoEntryAdmitsSubject { .. }));
}

#[test]
fn rejects_identity_with_wrong_issuer() {
    let tl = TrustList::from_entries(vec![ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    )])
    .unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.2.3",
        ),
        "https://attacker.example/issuer",
    );
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::IssuerMismatch { .. }));
}

#[test]
fn admits_local_key_identity_when_subject_matches_exactly() {
    let tl = TrustList::from_entries(vec![local_key_entry("dev@example.com")]).unwrap();
    let cand = SignatureIdentity::new("dev@example.com", LOCAL_KEY_ISSUER_SENTINEL);
    let outcome = match_signature_identity(&cand, &tl).expect("local key match");
    assert_eq!(outcome.entry_index, 0);
}

#[test]
fn rejects_local_key_identity_with_different_subject() {
    let tl = TrustList::from_entries(vec![local_key_entry("dev@example.com")]).unwrap();
    let cand = SignatureIdentity::new("attacker@example.com", LOCAL_KEY_ISSUER_SENTINEL);
    let err = match_signature_identity(&cand, &tl).unwrap_err();
    assert!(matches!(err, IdentityMatchError::NoEntryAdmitsSubject { .. }));
}

#[test]
fn matcher_short_circuits_on_first_matching_entry() {
    // First entry matches; second entry is broader but never reached.
    let first = ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    );
    let second_broader = ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "*",
    );
    let tl = TrustList::from_entries(vec![first, second_broader]).unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.0.0",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let outcome = match_signature_identity(&cand, &tl).unwrap();
    assert_eq!(outcome.entry_index, 0, "first match wins");
}

#[test]
fn note_is_propagated_into_match_outcome() {
    let mut entry = ghactions_entry(
        "example/worker",
        ".github/workflows/release.yml",
        "implementer-v*.*.*",
    );
    entry.note = Some("operator-curated".to_string());
    let tl = TrustList::from_entries(vec![entry]).unwrap();
    let cand = SignatureIdentity::new(
        keyless_subject(
            "example/worker",
            ".github/workflows/release.yml",
            "refs/tags/implementer-v1.0.0",
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    );
    let outcome = match_signature_identity(&cand, &tl).unwrap();
    assert_eq!(outcome.note.as_deref(), Some("operator-curated"));
}

// ---------- Rekor entry reference ----------

#[test]
fn rekor_default_is_sigstore_public_instance() {
    let r = RekorEntryRef::sigstore_default(
        "0".repeat(64),
        Some(1234),
    );
    assert_eq!(r.rekor_url, "https://rekor.sigstore.dev");
    assert_eq!(r.log_index, Some(1234));
    validate_rekor_entry_ref(&r).expect("default ref must validate");
}

#[test]
fn rekor_rejects_insecure_url() {
    let r = RekorEntryRef {
        rekor_url: "http://rekor.example".to_string(),
        entry_uuid: "0".repeat(64),
        log_index: None,
    };
    let err = validate_rekor_entry_ref(&r).unwrap_err();
    assert!(matches!(err, RekorRefError::InsecureRekorUrl { .. }));
}

#[test]
fn rekor_rejects_short_uuid() {
    let r = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "abc".to_string(),
        log_index: None,
    };
    let err = validate_rekor_entry_ref(&r).unwrap_err();
    assert!(matches!(err, RekorRefError::MalformedEntryUuid { .. }));
}

#[test]
fn rekor_rejects_non_hex_uuid() {
    let r = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "z".repeat(64),
        log_index: None,
    };
    let err = validate_rekor_entry_ref(&r).unwrap_err();
    assert!(matches!(err, RekorRefError::MalformedEntryUuid { .. }));
}

#[test]
fn rekor_rejects_uppercase_uuid() {
    let r = RekorEntryRef {
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        entry_uuid: "A".repeat(64),
        log_index: None,
    };
    let err = validate_rekor_entry_ref(&r).unwrap_err();
    assert!(matches!(err, RekorRefError::MalformedEntryUuid { .. }));
}
