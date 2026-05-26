//! Pure-function classifier mapping identity-verification evidence to a verdict class.
//!
//! The classifier is the interpretation half of the FT-090 action-interpretation
//! pair. It consumes the structured evidence the action's pure-execution side
//! captured (`super::evidence::IdentityVerificationEvidence`) plus the operator's
//! cosign trust list (`core::cosign_trust::TrustList`) and emits one of the five
//! [`super::SignatureVerdictClass`] values FT-090 enumerates, accompanied by an
//! operator-facing rationale prose suitable for the verdict artifact's
//! `dec:verdictRationale` literal.
//!
//! Classifier precedence (highest to lowest):
//!
//! 1. `image-not-found` — if the registry probe returned 404, the signature is
//!    irrelevant; the verdict is `image-not-found` regardless of cosign /
//!    Rekor outcomes.
//! 2. `invalid-signature` — cosign verify failed cryptographically OR the
//!    registry probe encountered a non-404 error (treated as inconclusive).
//! 3. `rekor-entry-missing` — signature checks out but the Rekor inclusion
//!    proof failed.
//! 4. `untrusted-identity` — signature checks out, Rekor confirms, but the
//!    cosign-recovered identity does NOT match any trust-list entry.
//! 5. `valid` — all three pure-execution checks passed AND the identity is on
//!    the trust list.
//!
//! Every reachable combination of evidence + trust list maps to exactly one
//! class — the classifier is a total function over the input domain. This is
//! the property TC-132 exercises.

use crate::core::cosign_trust::{match_signature_identity, IdentityMatchError, TrustList};

use super::evidence::{
    CosignVerifyOutcome, IdentityVerificationEvidence, RegistryProbeOutcome, RekorLookupOutcome,
};
use super::verdict::SignatureVerdictClass;

/// Errors raised by [`classify`] before it can return a verdict class.
///
/// The classifier itself is total over its declared input domain — every
/// evidence variant maps to a verdict. The error variants below cover
/// programmer-side mistakes (e.g. an empty trust list when the operator
/// expected a `valid` outcome), not data-side conditions. They are bubbled
/// up so a misconfigured runtime is visible at audit time rather than being
/// silently collapsed into a verdict.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentityVerifierError {
    /// The classifier was asked to evaluate a "signature is cryptographically
    /// valid" path with an empty trust list. The five-class vocabulary does
    /// not cover "the operator has no trust list yet" — that's a slice-0
    /// initialisation gap and the runtime should surface it explicitly
    /// rather than render `untrusted-identity`, which implies a curated
    /// trust list that simply doesn't admit the candidate.
    #[error(
        "classifier requires a non-empty trust list to evaluate a cryptographically-valid signature; \
         empty trust list indicates an unconfigured runtime, not an untrusted identity"
    )]
    EmptyTrustListForValidSignature,
}

/// Map identity-verification evidence + the operator's trust list to a
/// `(verdict class, rationale)` pair.
///
/// The mapping follows the precedence in the module-level docs. The returned
/// rationale string is operator-facing prose suitable for the verdict
/// artifact's `dec:verdictRationale` literal — it names which specific
/// evidence shape produced the verdict so the WorkerCurator's bundle reads
/// straight from the verdict without re-walking evidence.
pub fn classify(
    evidence: &IdentityVerificationEvidence,
    trust_list: &TrustList,
) -> Result<(SignatureVerdictClass, String), IdentityVerifierError> {
    if let Some(verdict) = classify_image_layer(&evidence.registry_probe) {
        return Ok(verdict);
    }
    if let Some(verdict) = classify_cosign_layer(&evidence.cosign_verify) {
        return Ok(verdict);
    }
    // Reaching this point implies cosign verify succeeded; the layered enum
    // pattern guarantees we have an identity to evaluate.
    let identity = match &evidence.cosign_verify {
        CosignVerifyOutcome::SignatureValid { identity } => identity,
        CosignVerifyOutcome::SignatureInvalid { .. } => {
            unreachable!("classify_cosign_layer would have returned early on invalid signature")
        }
    };
    if let Some(verdict) = classify_rekor_layer(&evidence.rekor_lookup) {
        return Ok(verdict);
    }
    classify_identity_layer(identity, trust_list)
}

/// Image-layer triage: a missing or erroring registry probe short-circuits
/// the classifier. The signature is meaningless if the operator can't see the
/// image it's supposed to authenticate.
fn classify_image_layer(probe: &RegistryProbeOutcome) -> Option<(SignatureVerdictClass, String)> {
    match probe {
        RegistryProbeOutcome::Found => None,
        RegistryProbeOutcome::NotFound => Some((
            SignatureVerdictClass::ImageNotFound,
            "OCI registry returned 404 for the candidate image reference; \
             the image is not present at the digest the submission claims."
                .to_string(),
        )),
        RegistryProbeOutcome::Error { detail } => Some((
            SignatureVerdictClass::InvalidSignature,
            format!(
                "OCI registry probe failed inconclusively ({detail}); \
                 the candidate signature cannot be authenticated against an unreachable image. \
                 Classified as invalid-signature pending slice-2 expansion of the verdict vocabulary."
            ),
        )),
    }
}

/// Cosign-layer triage: a cryptographic failure renders the signature
/// unconditionally invalid; identity-trust evaluation cannot rescue it.
fn classify_cosign_layer(
    cosign: &CosignVerifyOutcome,
) -> Option<(SignatureVerdictClass, String)> {
    match cosign {
        CosignVerifyOutcome::SignatureValid { .. } => None,
        CosignVerifyOutcome::SignatureInvalid { detail } => Some((
            SignatureVerdictClass::InvalidSignature,
            format!("cosign verify failed cryptographically: {detail}."),
        )),
    }
}

/// Rekor-layer triage: a missing or malformed Rekor entry collapses to
/// `rekor-entry-missing` regardless of whether the cosign verify itself
/// succeeded. The inclusion proof is a load-bearing part of trusting a
/// keyless signature.
fn classify_rekor_layer(
    rekor: &RekorLookupOutcome,
) -> Option<(SignatureVerdictClass, String)> {
    match rekor {
        RekorLookupOutcome::Confirmed => None,
        RekorLookupOutcome::Missing { detail } => Some((
            SignatureVerdictClass::RekorEntryMissing,
            format!(
                "referenced Rekor entry is absent or does not match the candidate signature: {detail}."
            ),
        )),
        RekorLookupOutcome::Malformed { detail } => Some((
            SignatureVerdictClass::RekorEntryMissing,
            format!("referenced Rekor entry reference is malformed: {detail}."),
        )),
    }
}

/// Identity-layer triage: the signature is cryptographically valid AND the
/// Rekor inclusion proof is confirmed; the remaining question is whether the
/// recovered identity matches an entry on the operator's trust list.
fn classify_identity_layer(
    identity: &crate::core::cosign_trust::SignatureIdentity,
    trust_list: &TrustList,
) -> Result<(SignatureVerdictClass, String), IdentityVerifierError> {
    if trust_list.is_empty() {
        return Err(IdentityVerifierError::EmptyTrustListForValidSignature);
    }
    match match_signature_identity(identity, trust_list) {
        Ok(outcome) => Ok((
            SignatureVerdictClass::Valid,
            format!(
                "signature cryptographically valid, Rekor inclusion confirmed, \
                 identity matches trust-list entry #{idx}{note}.",
                idx = outcome.entry_index,
                note = outcome
                    .note
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default(),
            ),
        )),
        Err(IdentityMatchError::EmptyTrustList) => {
            // Already guarded above; the matcher returning EmptyTrustList
            // after we passed the is_empty check is an internal invariant
            // breach. Re-raise as the same configuration error so callers
            // see one diagnostic.
            Err(IdentityVerifierError::EmptyTrustListForValidSignature)
        }
        Err(err) => Ok((
            SignatureVerdictClass::UntrustedIdentity,
            format!(
                "signature cryptographically valid and Rekor inclusion confirmed, \
                 but the signing identity is not on the operator's trust list: {err}."
            ),
        )),
    }
}
