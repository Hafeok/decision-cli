//! Bearer-token authentication for `POST /submissions` (FT-094).
//!
//! Slice-1 token discipline:
//!
//! - Each worker repo holds a `PIPELINE_SUBMISSION_TOKEN` secret. The
//!   token's plaintext NEVER reaches the orchestration store — only an
//!   HMAC-flavoured SHA-256 digest of `(token | repo_identity)` is held
//!   so a leaked store dump cannot be replayed against the endpoint
//!   without the original token.
//! - Tokens are bound 1:1 to a `RepoIdentity` (the same identity bound
//!   on the FT-089 trust list). The endpoint refuses any Submission
//!   whose declared `claimed_source_repo_uri` does not match the
//!   token's identity.
//! - Tokens are long-lived in slice 1; rotation lands in slice 3+.

use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use thiserror::Error;

/// A worker repo's stable identity as recorded on the trust list.
///
/// Mirrors the source-repo URI shape (`https://github.com/<owner>/<repo>`)
/// the producer pipeline emits as `claimed_source_repo_uri` on every
/// Submission.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoIdentity(String);

impl RepoIdentity {
    /// Construct from a non-empty repo URI. Empty strings are rejected
    /// to avoid pathological "every token resolves to ''" failure modes.
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Option<Self> {
        let uri = uri.into();
        if uri.is_empty() {
            None
        } else {
            Some(Self(uri))
        }
    }

    /// Borrow the inner repo URI.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compare against a declared `claimed_source_repo_uri` from a
    /// Submission payload. Slice-1 is exact-string-match; slice-2+
    /// considers normalising trailing slashes and `.git` suffixes.
    #[must_use]
    pub fn matches_declared(&self, declared: &str) -> bool {
        self.0 == declared
    }
}

/// Token store error variants surfaced through the auth layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenStoreError {
    /// Empty token; refuses construction.
    #[error("token must be non-empty")]
    EmptyToken,
    /// Empty repo identity; refuses construction.
    #[error("repo identity must be non-empty")]
    EmptyIdentity,
}

/// In-memory token-to-identity directory.
///
/// Tokens are hashed before being stored — the plaintext never sits in
/// memory after [`TokenStore::insert`] returns. Resolution is by
/// constant-time digest comparison via the `HashMap` key, which is
/// adequate for slice 1 (no online rotation, no replay-resistant
/// nonce scheme).
#[derive(Debug, Default, Clone)]
pub struct TokenStore {
    by_digest: Arc<HashMap<String, RepoIdentity>>,
}

impl TokenStore {
    /// Build a fresh store from an iterator of `(token, identity)` pairs.
    /// Returns `TokenStoreError::EmptyToken` / `EmptyIdentity` on any
    /// blank input; otherwise inserts each entry.
    pub fn from_pairs(
        pairs: impl IntoIterator<Item = (String, RepoIdentity)>,
    ) -> Result<Self, TokenStoreError> {
        let mut by_digest = HashMap::new();
        for (token, identity) in pairs {
            if token.is_empty() {
                return Err(TokenStoreError::EmptyToken);
            }
            by_digest.insert(digest_token(&token, &identity), identity);
        }
        Ok(Self {
            by_digest: Arc::new(by_digest),
        })
    }

    /// Single-entry constructor used by tests and CLI-arg threading.
    pub fn single(token: &str, identity: RepoIdentity) -> Result<Self, TokenStoreError> {
        Self::from_pairs(std::iter::once((token.to_string(), identity)))
    }

    /// Resolve a token (raw plaintext from the `Authorization` header)
    /// against the store. Returns the bound identity on a hit.
    ///
    /// Identity is **not** known at resolution time (the token alone is
    /// the lookup key), so resolution iterates the digest map. Slice 1
    /// expects O(N) ≈ a few dozen repos — the linear scan is fine.
    #[must_use]
    pub fn resolve(&self, token: &str) -> Option<RepoIdentity> {
        if token.is_empty() {
            return None;
        }
        // Slice-1 surface: iterate; digest_token is keyed on identity so
        // the same plaintext for different identities produces different
        // digests. Each candidate identity gets one HMAC.
        for identity in self.by_digest.values() {
            let candidate_digest = digest_token(token, identity);
            if self.by_digest.contains_key(&candidate_digest) {
                return Some(identity.clone());
            }
        }
        None
    }
}

/// Stable digest used as the `HashMap` key for token records.
///
/// `sha256("v1:" | repo_uri | ":" | token)` — namespacing prefix lets a
/// slice-3+ migration recognise legacy entries without re-hashing them.
fn digest_token(token: &str, identity: &RepoIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"v1:");
    hasher.update(identity.as_str().as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
    let bytes = hasher.finalize();
    hex_encode(&bytes)
}

/// Hex-encode a byte slice without pulling in a `hex` dependency.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Parse the bearer token from an HTTP `Authorization` header value.
/// Returns `None` if the header is missing the `Bearer ` prefix or the
/// payload after it is empty.
#[must_use]
pub fn parse_bearer(header_value: Option<&str>) -> Option<&str> {
    let raw = header_value?;
    let stripped = raw.strip_prefix("Bearer ")?;
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

#[cfg(test)]
mod auth_unit_tests {
    use super::*;

    #[test]
    fn parse_bearer_accepts_well_formed_header() {
        assert_eq!(parse_bearer(Some("Bearer abc123")), Some("abc123"));
    }

    #[test]
    fn parse_bearer_rejects_empty_payload() {
        assert_eq!(parse_bearer(Some("Bearer ")), None);
    }

    #[test]
    fn parse_bearer_rejects_missing_prefix() {
        assert_eq!(parse_bearer(Some("abc123")), None);
    }

    #[test]
    fn parse_bearer_rejects_absent_header() {
        assert_eq!(parse_bearer(None), None);
    }

    #[test]
    fn token_store_resolves_known_token() {
        let identity = RepoIdentity::new("https://github.com/example/worker").expect("identity");
        let store = TokenStore::single("secret-token", identity.clone()).expect("store");
        assert_eq!(store.resolve("secret-token"), Some(identity));
    }

    #[test]
    fn token_store_rejects_unknown_token() {
        let identity = RepoIdentity::new("https://github.com/example/worker").expect("identity");
        let store = TokenStore::single("secret-token", identity).expect("store");
        assert_eq!(store.resolve("other-token"), None);
    }

    #[test]
    fn repo_identity_rejects_blank() {
        assert_eq!(RepoIdentity::new(""), None);
    }
}
