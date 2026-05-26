//! Parse a Sigstore Fulcio subject URI into its GitHub Actions axes
//! (FT-089 / ADR-058).
//!
//! Cosign's keyless signing flow records the OIDC-provided subject URI
//! on the issued Fulcio certificate. For GitHub Actions, that URI has
//! a stable shape:
//!
//! ```text
//! https://github.com/<owner>/<repo>/<workflow-path>@<ref>
//! ```
//!
//! Concrete examples:
//!
//! - `https://github.com/example/worker/.github/workflows/release.yml@refs/tags/v1.2.3`
//! - `https://github.com/Hafeok/decision-cli/.github/workflows/release-worker.yml@refs/heads/main`
//!
//! The reusable-workflow form looks the same: the workflow-path segment
//! points at the reusable workflow's owning repo, and the `<ref>`
//! component encodes the ref the workflow was invoked at.
//!
//! This module owns the parser; the trust-list matcher
//! ([`super::matcher`]) consumes the parsed structure.

use thiserror::Error;

/// The OIDC issuer URI Sigstore uses for GitHub Actions OIDC tokens.
/// Trust list entries from [`super::TrustOrigin::GithubActions`] are
/// matched against this issuer.
pub const GITHUB_ACTIONS_ISSUER_URI: &str = "https://token.actions.githubusercontent.com";

/// Sentinel issuer string used for local-key trust list entries
/// (ADR-058's development-only fallback). The matcher recognises it
/// as a signal to compare the candidate subject verbatim against the
/// enrolled [`super::TrustOrigin::LocalKey`] subject, instead of
/// performing the three-axis GitHub Actions decomposition.
pub const LOCAL_KEY_ISSUER_SENTINEL: &str = "local-key";

/// A parsed GitHub Actions Fulcio subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubActionsSubject {
    /// Repo slug (`owner/name`).
    pub repo: String,
    /// Workflow path relative to the repo root
    /// (e.g. `.github/workflows/release.yml`).
    pub workflow_path: String,
    /// Git ref the workflow was invoked at (e.g. `refs/tags/v1.2.3`,
    /// `refs/heads/main`).
    pub git_ref: String,
}

impl GithubActionsSubject {
    /// If the ref is a tag ref (`refs/tags/<tag>`), return `<tag>`.
    /// Otherwise return `None`. The trust list's tag-pattern axis only
    /// applies when the candidate is a tag ref; a branch-ref subject
    /// is admissible only via an entry whose tag pattern explicitly
    /// matches the branch-ref form (rare; intentional).
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        self.git_ref.strip_prefix("refs/tags/")
    }

    /// True iff the ref points at a tag.
    #[must_use]
    pub fn is_tag_ref(&self) -> bool {
        self.tag().is_some()
    }
}

/// Parse a Fulcio subject URI as a GitHub Actions identity.
///
/// Returns the decomposed structure on success, or a
/// [`SubjectParseError`] describing which expected component was
/// missing. Subjects that do not start with the GitHub HTTPS prefix
/// produce [`SubjectParseError::NotGithubActions`]; callers should
/// fall through to local-key matching in that case.
pub fn parse_github_actions_subject(
    subject: &str,
) -> Result<GithubActionsSubject, SubjectParseError> {
    const PREFIX: &str = "https://github.com/";
    let rest = subject
        .strip_prefix(PREFIX)
        .ok_or(SubjectParseError::NotGithubActions)?;

    let (path_part, ref_part) = rest
        .split_once('@')
        .ok_or(SubjectParseError::MissingRefSeparator)?;

    let mut path_segments = path_part.splitn(3, '/');
    let owner = path_segments
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(SubjectParseError::MissingOwner)?;
    let name = path_segments
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(SubjectParseError::MissingRepoName)?;
    let workflow_path = path_segments
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(SubjectParseError::MissingWorkflowPath)?;

    if ref_part.is_empty() {
        return Err(SubjectParseError::EmptyRef);
    }

    Ok(GithubActionsSubject {
        repo: format!("{owner}/{name}"),
        workflow_path: workflow_path.to_string(),
        git_ref: ref_part.to_string(),
    })
}

/// Errors produced by [`parse_github_actions_subject`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubjectParseError {
    /// Subject does not start with the GitHub HTTPS prefix — likely a
    /// local-key or non-GitHub OIDC subject. Callers should route
    /// through the local-key match path instead.
    #[error("subject is not a GitHub Actions identity (expected https://github.com/... prefix)")]
    NotGithubActions,
    /// Subject is missing the `@<ref>` separator.
    #[error("subject missing `@<ref>` separator")]
    MissingRefSeparator,
    /// Subject's path component is missing the owner segment.
    #[error("subject missing repo owner segment")]
    MissingOwner,
    /// Subject's path component is missing the repo-name segment.
    #[error("subject missing repo name segment")]
    MissingRepoName,
    /// Subject's path component is missing the workflow-path segment.
    #[error("subject missing workflow-path segment")]
    MissingWorkflowPath,
    /// Subject's `@<ref>` suffix is empty.
    #[error("subject has empty git ref after `@`")]
    EmptyRef,
}
