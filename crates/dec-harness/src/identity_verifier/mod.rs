//! Identity-verifier action — classifies a candidate worker image's signature evidence.
//!
//! FT-090 introduces the `identity-verifier` action role (ADR-017 pure-execution +
//! interpretation pair). The action's pure-execution side performs three side-
//! effectful checks against a `dec:WorkerImageSubmission`:
//!
//! 1. probe the OCI registry for the candidate image (per the
//!    `candidate_registry_ref` digest),
//! 2. run `cosign verify` against the recorded signature using the operator's
//!    trust list (`core::cosign_trust`),
//! 3. resolve the recorded Rekor entry and confirm log inclusion + match.
//!
//! Each check terminates in a structured outcome captured on
//! [`IdentityVerificationEvidence`]. The interpretation side — this module's
//! [`classify`] — maps the evidence (plus the operator's trust list) to one of
//! the five [`SignatureVerdictClass`] values FT-090 enumerates:
//!
//! - `valid`               — all three checks succeeded AND identity is on the trust list.
//! - `invalid-signature`   — cosign verify failed cryptographically.
//! - `untrusted-identity`  — signature checked out cryptographically, but identity is off-list.
//! - `image-not-found`     — registry probe came back 404 for the candidate ref.
//! - `rekor-entry-missing` — referenced Rekor entry is absent or doesn't match.
//!
//! The classifier is a pure total function — every reachable evidence
//! combination maps to exactly one verdict — so TC-132 can drive it through
//! all five outcome conditions without any network call. Network plumbing is
//! the responsibility of the action's runtime (slice-1 scope: synchronous
//! `cosign` subprocess invocation; not implemented here).

mod classifier;
mod evidence;
mod verdict;

#[cfg(test)]
mod tests;

pub use classifier::{classify, IdentityVerifierError};
pub use evidence::{
    CosignVerifyOutcome, IdentityVerificationEvidence, RegistryProbeOutcome, RekorLookupOutcome,
};
pub use verdict::{SignatureVerdict, SignatureVerdictClass};
