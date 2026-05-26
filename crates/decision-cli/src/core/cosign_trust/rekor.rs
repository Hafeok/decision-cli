//! Rekor transparency-log entry reference (FT-089 / ADR-058).
//!
//! Cosign's keyless flow records every signature in the Rekor public
//! transparency log. The resulting log entry's UUID (and, optionally,
//! a log-index integer) is the reference the WorkerImageSubmission
//! carries forward so the identity-verifier action (FT-090) can
//! confirm log inclusion before trusting the signature.
//!
//! This module owns the on-the-wire shape of that reference: a
//! `(rekor_url, entry_uuid)` pair, with a syntactic validator that
//! rejects obviously malformed values up-front. Actual network
//! resolution of the Rekor entry — fetching the entry, verifying
//! inclusion proof, checking the signature against the cert chain —
//! lives in FT-090. This module exists so the submission validator
//! (called inside the submission's SHACL stage and the FT-094 admission
//! endpoint) can refuse a submission whose Rekor reference is obviously
//! malformed *without pulling*, in the same spirit as
//! `core::oci_manifest::validate_worker_oci_manifest`.

use thiserror::Error;

/// Default Sigstore-operated Rekor instance URL. Operators with a
/// private Rekor deployment override this on a per-submission basis.
pub const DEFAULT_REKOR_URL: &str = "https://rekor.sigstore.dev";

/// A reference to a Rekor transparency log entry. Carried on the
/// `WorkerImageSubmission` so the verifier can resolve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekorEntryRef {
    /// Rekor instance base URL (e.g. `https://rekor.sigstore.dev`).
    pub rekor_url: String,
    /// Rekor entry UUID (the SHA-256 of the canonicalised entry body,
    /// rendered as 64 lowercase hex digits — cosign reports it via
    /// `cosign sign --output-rekor-log-entry-uuid`).
    pub entry_uuid: String,
    /// Optional integer log index (cosign reports this alongside the
    /// UUID; storing it speeds up downstream lookups but is not load-
    /// bearing for verification).
    pub log_index: Option<u64>,
}

impl RekorEntryRef {
    /// Construct a reference against the default Sigstore Rekor.
    #[must_use]
    pub fn sigstore_default(entry_uuid: impl Into<String>, log_index: Option<u64>) -> Self {
        Self {
            rekor_url: DEFAULT_REKOR_URL.to_string(),
            entry_uuid: entry_uuid.into(),
            log_index,
        }
    }
}

/// Validate the syntactic shape of a Rekor entry reference.
///
/// Refuses references with:
///
/// - empty `rekor_url`,
/// - a `rekor_url` lacking the `https://` scheme (Rekor entries MUST
///   be fetched over TLS for the inclusion proof to mean anything),
/// - an `entry_uuid` that is not exactly 64 lowercase hex characters.
///
/// This is *not* a network check — it does not contact Rekor. FT-090's
/// identity-verifier action is what fetches the entry and confirms its
/// inclusion proof; this validator just refuses obviously-malformed
/// references early so the verifier action does not spend a network
/// round-trip on a typo.
pub fn validate_rekor_entry_ref(r: &RekorEntryRef) -> Result<(), RekorRefError> {
    if r.rekor_url.trim().is_empty() {
        return Err(RekorRefError::EmptyRekorUrl);
    }
    if !r.rekor_url.starts_with("https://") {
        return Err(RekorRefError::InsecureRekorUrl {
            url: r.rekor_url.clone(),
        });
    }
    if r.entry_uuid.len() != 64 {
        return Err(RekorRefError::MalformedEntryUuid {
            uuid: r.entry_uuid.clone(),
            reason: format!("expected 64 hex chars, got {}", r.entry_uuid.len()),
        });
    }
    if !r
        .entry_uuid
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(RekorRefError::MalformedEntryUuid {
            uuid: r.entry_uuid.clone(),
            reason: "must be 64 lowercase hex digits".to_string(),
        });
    }
    Ok(())
}

/// Errors raised by [`validate_rekor_entry_ref`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RekorRefError {
    /// The `rekor_url` was empty.
    #[error("Rekor URL must not be empty")]
    EmptyRekorUrl,
    /// The `rekor_url` did not start with `https://`.
    #[error("Rekor URL {url:?} must start with https:// (TLS required for inclusion proof trust)")]
    InsecureRekorUrl {
        /// The offending URL (echoed for operator diagnosis).
        url: String,
    },
    /// The `entry_uuid` was not 64 lowercase hex characters.
    #[error("Rekor entry UUID {uuid:?} is malformed: {reason}")]
    MalformedEntryUuid {
        /// The malformed UUID value (echoed for operator diagnosis).
        uuid: String,
        /// Human-readable explanation of which UUID rule failed.
        reason: String,
    },
}
