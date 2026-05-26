//! Unit tests for the identity-verifier classifier.
//!
//! These are smoke tests over the precedence rules in
//! `super::classifier`; the end-to-end TC-132 acceptance test
//! (in `crates/decision-cli/tests/`) exercises the full evidence-to-
//! verdict round-trip including the RDF serialisation surface.

use crate::core::cosign_trust::{
    SignatureIdentity, TagPattern, TrustList, TrustListEntry, TrustOrigin,
    GITHUB_ACTIONS_ISSUER_URI,
};

use super::classifier::{classify, IdentityVerifierError};
use super::evidence::{
    CosignVerifyOutcome, IdentityVerificationEvidence, RegistryProbeOutcome, RekorLookupOutcome,
};
use super::verdict::SignatureVerdictClass;

const WORKFLOW_PATH: &str = ".github/workflows/release-worker.yml";

fn admitted_identity() -> SignatureIdentity {
    SignatureIdentity::new(
        format!(
            "https://github.com/Hafeok/decision-cli-worker-implementer/{WORKFLOW_PATH}@refs/tags/implementer-v1.2.3"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    )
}

fn populated_trust_list() -> TrustList {
    TrustList::from_entries(vec![TrustListEntry {
        origin: TrustOrigin::GithubActions {
            repo: "Hafeok/decision-cli-worker-implementer".to_string(),
            workflow_path: WORKFLOW_PATH.to_string(),
            tag_pattern: TagPattern::parse("implementer-v*.*.*")
                .expect("test tag pattern must parse"),
        },
        issuer: GITHUB_ACTIONS_ISSUER_URI.to_string(),
        note: Some("test entry".to_string()),
    }])
    .expect("trust list construction must succeed")
}

fn evidence(
    probe: RegistryProbeOutcome,
    cosign: CosignVerifyOutcome,
    rekor: RekorLookupOutcome,
) -> IdentityVerificationEvidence {
    IdentityVerificationEvidence::new(probe, cosign, rekor)
}

#[test]
fn empty_trust_list_for_valid_signature_is_a_configuration_error() {
    let result = classify(
        &evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &TrustList::empty(),
    );
    assert!(matches!(
        result,
        Err(IdentityVerifierError::EmptyTrustListForValidSignature)
    ));
}

#[test]
fn image_layer_takes_precedence_over_everything_else() {
    // Even with a perfectly-signed Rekor-confirmed identity, a missing image
    // collapses straight to `image-not-found`.
    let (class, rationale) = classify(
        &evidence(
            RegistryProbeOutcome::NotFound,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
    )
    .expect("classifier must produce a verdict");
    assert_eq!(class, SignatureVerdictClass::ImageNotFound);
    assert!(rationale.contains("404"));
}

#[test]
fn cosign_failure_short_circuits_before_rekor_or_identity() {
    let (class, rationale) = classify(
        &evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureInvalid {
                detail: "cert chain unverified".to_string(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
    )
    .expect("classifier must produce a verdict");
    assert_eq!(class, SignatureVerdictClass::InvalidSignature);
    assert!(rationale.contains("cert chain unverified"));
}

#[test]
fn rekor_failure_overrides_identity_check() {
    let (class, _) = classify(
        &evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Missing {
                detail: "404 from rekor.sigstore.dev".to_string(),
            },
        ),
        &populated_trust_list(),
    )
    .expect("classifier must produce a verdict");
    assert_eq!(class, SignatureVerdictClass::RekorEntryMissing);
}

#[test]
fn off_trust_list_identity_collapses_to_untrusted_identity() {
    let (class, _) = classify(
        &evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: SignatureIdentity::new(
                    format!(
                        "https://github.com/attacker/fork/{WORKFLOW_PATH}@refs/tags/implementer-v9.9.9"
                    ),
                    GITHUB_ACTIONS_ISSUER_URI,
                ),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
    )
    .expect("classifier must produce a verdict");
    assert_eq!(class, SignatureVerdictClass::UntrustedIdentity);
}

#[test]
fn happy_path_produces_valid() {
    let (class, rationale) = classify(
        &evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
    )
    .expect("classifier must produce a verdict");
    assert_eq!(class, SignatureVerdictClass::Valid);
    assert!(rationale.contains("trust-list entry #0"));
}

#[test]
fn verdict_class_round_trips_via_as_str() {
    for c in [
        SignatureVerdictClass::Valid,
        SignatureVerdictClass::InvalidSignature,
        SignatureVerdictClass::UntrustedIdentity,
        SignatureVerdictClass::ImageNotFound,
        SignatureVerdictClass::RekorEntryMissing,
    ] {
        assert_eq!(SignatureVerdictClass::parse(c.as_str()), Some(c));
    }
    assert_eq!(SignatureVerdictClass::parse("nope"), None);
}
