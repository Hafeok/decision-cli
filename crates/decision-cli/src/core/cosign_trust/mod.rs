//! Cosign keyless signing trust list (FT-089 / ADR-058).
//!
//! Worker releases are signed keyless via `cosign sign` against
//! Sigstore's Fulcio CA, using the ambient GitHub Actions OIDC token.
//! No private key material ever lives in worker repos. The identity
//! recorded on the resulting signature is the Fulcio certificate's
//! `subject` (the workflow run identity, e.g.
//! `https://github.com/example/worker/.github/workflows/release.yml@refs/tags/v1.2.3`)
//! plus its `issuer` (e.g. `https://token.actions.githubusercontent.com`).
//!
//! Per ADR-058, the orchestration system maintains a **trust list** of
//! permitted Fulcio identities, matched along three orthogonal axes:
//!
//! - **GitHub repo** (`owner/name`)
//! - **Workflow path** (e.g. `.github/workflows/release.yml`, or a
//!   specific reusable-workflow ref)
//! - **Tag pattern** (e.g. `implementer-v*.*.*`)
//!
//! A candidate signature identity is admissible iff at least one trust
//! list entry matches all three axes. The matcher also supports a
//! local-key fallback path (development workflows outside GitHub
//! Actions): an entry whose `issuer` is the literal local-key sentinel
//! is admitted by exact subject match, without GitHub-style axis
//! decomposition.
//!
//! This module is intentionally substrate-only — it operates on plain
//! string values and a list of trust entries. It does not call out to
//! a registry, fetch a Rekor entry, or talk to the graph. The
//! identity-verifier action (FT-090) consumes this module to render
//! `untrusted-identity` verdicts before any cosign cryptographic check
//! runs.

mod matcher;
mod parse;
mod rekor;
mod trust_list;

#[cfg(test)]
mod tests;

pub use matcher::{
    match_signature_identity, IdentityMatchError, IdentityMatchOutcome, SignatureIdentity,
};
pub use parse::{
    parse_github_actions_subject, GithubActionsSubject, SubjectParseError,
    GITHUB_ACTIONS_ISSUER_URI, LOCAL_KEY_ISSUER_SENTINEL,
};
pub use rekor::{validate_rekor_entry_ref, RekorEntryRef, RekorRefError};
pub use trust_list::{
    TagPattern, TagPatternError, TrustList, TrustListEntry, TrustListError, TrustOrigin,
};
