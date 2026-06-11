//! Structured evidence captured by the identity-verifier action's pure-execution side.
//!
//! Per FT-090, the action runs three side-effectful checks against a
//! `dec:WorkerImageSubmission`: registry probe, `cosign verify`, and Rekor entry
//! resolution. Each check terminates in a typed outcome and the three are
//! aggregated into [`IdentityVerificationEvidence`], which the interpretation
//! side consumes as input. This split keeps the classifier deterministic and
//! testable: TC-132 fabricates evidence for each of the five outcome classes
//! without invoking cosign or talking to a registry.

use crate::cosign_trust::SignatureIdentity;

/// Aggregate of the three pure-execution checks the identity-verifier runs.
///
/// All three outcome fields are populated by the action runtime; absent /
/// unknown signals must be encoded as the relevant enum variant, never via
/// `Option`. The classifier ([`super::classify`]) treats this struct as the
/// complete record of what was observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityVerificationEvidence {
    /// Result of probing the OCI registry for `candidate_registry_ref`.
    pub registry_probe: RegistryProbeOutcome,
    /// Result of running `cosign verify` against the candidate image.
    pub cosign_verify: CosignVerifyOutcome,
    /// Result of resolving the Rekor entry referenced on the submission.
    pub rekor_lookup: RekorLookupOutcome,
}

impl IdentityVerificationEvidence {
    /// Construct an evidence aggregate. Each outcome is required because the
    /// pure-execution side runs every probe, recording the result even when an
    /// earlier check has already failed (so the verdict rationale can cite the
    /// full picture rather than collapsing to the first failure).
    #[must_use]
    pub const fn new(
        registry_probe: RegistryProbeOutcome,
        cosign_verify: CosignVerifyOutcome,
        rekor_lookup: RekorLookupOutcome,
    ) -> Self {
        Self {
            registry_probe,
            cosign_verify,
            rekor_lookup,
        }
    }
}

/// Outcome of the OCI registry probe (HEAD on the candidate ref's digest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryProbeOutcome {
    /// Registry responded 2xx for the candidate ref.
    Found,
    /// Registry responded 404 (or equivalent absence signal) for the candidate ref.
    NotFound,
    /// Registry probe failed in a way that is not a clean 404 (TLS error,
    /// network timeout, 5xx). The classifier treats this as an inconclusive
    /// outcome and short-circuits to `invalid-signature` semantically — but
    /// per FT-090 the five-class vocabulary doesn't have a dedicated
    /// "registry-error" class, so the rationale carries the diagnostic. Slice
    /// 2+ may revisit this if operators want a sixth class.
    Error {
        /// Operator-facing diagnostic (e.g. "connection refused", "TLS handshake failed").
        detail: String,
    },
}

/// Outcome of the `cosign verify` cryptographic check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CosignVerifyOutcome {
    /// `cosign verify` succeeded; the returned identity is the certificate
    /// subject + issuer the operator can then match against the trust list.
    SignatureValid {
        /// Identity recovered from the verified Fulcio certificate (or the
        /// local-key sentinel for fallback signing).
        identity: SignatureIdentity,
    },
    /// `cosign verify` failed cryptographically — the signature does not
    /// validate against the embedded certificate chain or the certificate
    /// itself failed verification. Includes any diagnostic cosign emitted.
    SignatureInvalid {
        /// Operator-facing diagnostic.
        detail: String,
    },
}

/// Outcome of resolving the Rekor transparency-log entry referenced on the
/// submission and confirming it covers the candidate signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RekorLookupOutcome {
    /// Rekor returned the entry, the entry covers the candidate signature
    /// payload, and the inclusion proof checks out.
    Confirmed,
    /// The reference is syntactically valid but no such entry exists in Rekor
    /// (404 from the GET) OR the entry exists but doesn't match the candidate
    /// signature payload. Both map to `rekor-entry-missing` per FT-090.
    Missing {
        /// Operator-facing diagnostic.
        detail: String,
    },
    /// The reference is syntactically malformed (caught up-front by
    /// `core::cosign_trust::validate_rekor_entry_ref`) so no network call was
    /// made. Treated as `Missing` by the classifier — the verdict class is
    /// the same — but carrying the distinction lets the rationale say
    /// "malformed reference" instead of "Rekor returned 404".
    Malformed {
        /// Operator-facing diagnostic.
        detail: String,
    },
}
