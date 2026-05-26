//! FT-090 / ADR-017 / ADR-018 — `dec:SignatureVerdict` vocabulary.
//!
//! A SignatureVerdict is the interpretation artifact produced by the
//! `identity-verifier` action (FT-090). It classifies the outcome of
//! verifying a `dec:WorkerImageSubmission`'s cosign signature against
//! the operator's trust list AND its Rekor inclusion proof. The verdict
//! feeds the WorkerCurator's bundle (FT-092); a `valid` verdict is
//! required (but not sufficient) for admission.
//!
//! The five outcome classes are disjoint and exhaustive over the
//! identity-verification action's terminal evidence — see
//! `core::identity_verifier::classifier` for the mapping.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

/// Class IRI for `dec:SignatureVerdict` (FT-090).
pub const IRI_DEC_SIGNATURE_VERDICT: &str = "https://decision-cli.dev/ns#SignatureVerdict";

/// IRI prefix for minted SignatureVerdict artifacts:
/// `https://decision-cli.dev/ns/signature-verdict/<id>`.
pub const IRI_DEC_SIGNATURE_VERDICT_PREFIX: &str =
    "https://decision-cli.dev/ns/signature-verdict/";

/// `dec:signatureVerdictClass` — one of the five outcome literals below.
pub const IRI_DEC_SIGNATURE_VERDICT_CLASS: &str =
    "https://decision-cli.dev/ns#signatureVerdictClass";

/// `dec:verdictRationale` — operator-facing free-form rationale.
pub const IRI_DEC_VERDICT_RATIONALE: &str = "https://decision-cli.dev/ns#verdictRationale";

/// `dec:verifiedSubmission` — motivational edge: SignatureVerdict → WorkerImageSubmission.
///
/// The verdict exists because the named submission needed identity
/// verification; per ADR-038/ADR-039 the predicate is a `wasDerivedFrom`
/// sub-property declared in the embedded motivational vocabulary
/// (FT-070). This module re-uses the existing `dec:respondsTo`
/// motivational predicate IRI rather than minting a new one; declaring
/// the constant here keeps the SignatureVerdict module self-contained
/// for callers that don't otherwise import the motivational vocabulary.
pub const IRI_DEC_RESPONDS_TO: &str = "https://decision-cli.dev/ns#respondsTo";

// --- Outcome-class literals (the five verdict classes, FT-090 §Scope) -------

/// Signature checks, identity on trust list, Rekor inclusion confirmed.
pub const SIGNATURE_VERDICT_VALID: &str = "valid";

/// `cosign verify` failed cryptographically.
pub const SIGNATURE_VERDICT_INVALID_SIGNATURE: &str = "invalid-signature";

/// Signature valid but signer not on the operator's trust list.
pub const SIGNATURE_VERDICT_UNTRUSTED_IDENTITY: &str = "untrusted-identity";

/// Registry returned 404 for the candidate ref.
pub const SIGNATURE_VERDICT_IMAGE_NOT_FOUND: &str = "image-not-found";

/// Referenced Rekor entry doesn't exist or doesn't match.
pub const SIGNATURE_VERDICT_REKOR_ENTRY_MISSING: &str = "rekor-entry-missing";

#[must_use]
pub fn signature_verdict_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SIGNATURE_VERDICT)
}

#[must_use]
pub fn signature_verdict_class_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SIGNATURE_VERDICT_CLASS)
}

#[must_use]
pub fn verdict_rationale_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERDICT_RATIONALE)
}

#[must_use]
pub fn responds_to_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RESPONDS_TO)
}
