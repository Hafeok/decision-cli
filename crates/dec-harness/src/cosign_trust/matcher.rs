//! Trust-list matcher for Sigstore Fulcio identities (FT-089 / ADR-058).
//!
//! Single entry point [`match_signature_identity`] — accepts a
//! candidate [`SignatureIdentity`] (the subject + issuer pair recorded
//! on a [`super::super::ontology::worker_image_submission::WorkerImageSubmission`])
//! and a [`super::TrustList`], and returns whether the identity is on
//! the list. The verdict is structured: a match returns the index of
//! the winning entry and the parsed subject; a non-match returns a
//! reasoned failure the identity-verifier action (FT-090) can surface
//! as an `untrusted-identity` verdict rationale.

use thiserror::Error;

use super::parse::{parse_github_actions_subject, SubjectParseError, LOCAL_KEY_ISSUER_SENTINEL};
use super::trust_list::{TrustList, TrustOrigin};

/// A candidate signature identity, sourced from a Fulcio certificate
/// (or a local-key signing run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureIdentity {
    /// The Fulcio certificate subject (or local-key subject).
    pub subject: String,
    /// The OIDC issuer URI (e.g. `https://token.actions.githubusercontent.com`,
    /// or the local-key sentinel).
    pub issuer: String,
}

impl SignatureIdentity {
    /// Construct an identity from owned strings.
    #[must_use]
    pub fn new(subject: impl Into<String>, issuer: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            issuer: issuer.into(),
        }
    }
}

/// Outcome of a successful identity match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMatchOutcome {
    /// Index of the matching entry in the trust list (declaration order).
    pub entry_index: usize,
    /// Operator note from the matching entry, if any.
    pub note: Option<String>,
}

/// Reasoned failure from [`match_signature_identity`].
///
/// The variants align with the verdict classes FT-090 enumerates so
/// the identity-verifier can map a matcher error directly to a verdict
/// without extra interpretation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityMatchError {
    /// The trust list is empty — nothing is admissible.
    #[error("trust list is empty; no signature identity can be admitted")]
    EmptyTrustList,
    /// No trust-list entry's issuer matched the candidate's issuer.
    #[error("no trust list entry's issuer matches candidate issuer {issuer:?}")]
    IssuerMismatch {
        /// The candidate issuer that did not match any entry.
        issuer: String,
    },
    /// The candidate's subject did not parse as a GitHub Actions identity
    /// (and no local-key fallback entry admits it).
    #[error("candidate subject is not a GitHub Actions identity: {source}")]
    UnparseableGithubSubject {
        /// Underlying parser failure.
        #[source]
        source: SubjectParseError,
    },
    /// At least one entry's issuer matched, but no entry pinned a repo /
    /// workflow path / tag pattern that the candidate satisfies.
    #[error(
        "no trust list entry admits subject {subject:?} (issuer {issuer:?}); \
         {checked} entries with matching issuer were evaluated"
    )]
    NoEntryAdmitsSubject {
        /// The candidate subject (echoed for operator diagnosis).
        subject: String,
        /// The candidate issuer (echoed for operator diagnosis).
        issuer: String,
        /// Number of entries that matched the issuer axis but failed
        /// on at least one of the other pinned axes.
        checked: usize,
    },
}

/// Match a candidate signature identity against a trust list.
///
/// Behaviour:
///
/// 1. An empty trust list short-circuits to [`IdentityMatchError::EmptyTrustList`].
/// 2. Entries are iterated in declaration order; the first entry whose
///    issuer matches the candidate's issuer AND whose origin pins all
///    match the candidate's subject wins.
/// 3. A local-key entry (issuer = [`LOCAL_KEY_ISSUER_SENTINEL`]) matches
///    iff the candidate's issuer equals the sentinel AND the candidate's
///    subject is byte-for-byte identical to the entry's enrolled subject.
/// 4. A GitHub Actions entry matches iff the candidate's issuer equals
///    the entry's issuer, the candidate's subject parses as a GitHub
///    Actions subject, and the parsed `(repo, workflow_path, tag)`
///    each match the entry's pins.
///
/// If at least one entry's issuer matched but no entry's pins admitted
/// the subject, the error is [`IdentityMatchError::NoEntryAdmitsSubject`]
/// — the operator can then audit which entries were near misses.
/// If no entry's issuer matched at all, the error is
/// [`IdentityMatchError::IssuerMismatch`].
pub fn match_signature_identity(
    candidate: &SignatureIdentity,
    trust_list: &TrustList,
) -> Result<IdentityMatchOutcome, IdentityMatchError> {
    if trust_list.is_empty() {
        return Err(IdentityMatchError::EmptyTrustList);
    }

    let mut issuer_hits = 0_usize;
    let mut parsed_github_subject: Option<super::parse::GithubActionsSubject> = None;

    for (idx, entry) in trust_list.entries().iter().enumerate() {
        if entry.issuer != candidate.issuer {
            continue;
        }
        issuer_hits += 1;
        if entry_admits(candidate, entry, &mut parsed_github_subject)? {
            return Ok(IdentityMatchOutcome {
                entry_index: idx,
                note: entry.note.clone(),
            });
        }
    }

    if issuer_hits == 0 {
        Err(IdentityMatchError::IssuerMismatch {
            issuer: candidate.issuer.clone(),
        })
    } else {
        Err(IdentityMatchError::NoEntryAdmitsSubject {
            subject: candidate.subject.clone(),
            issuer: candidate.issuer.clone(),
            checked: issuer_hits,
        })
    }
}

/// Per-entry admission predicate. Splits the per-origin matching logic
/// out of [`match_signature_identity`] so the outer driver stays under
/// the function-length budget. Returns `Ok(true)` iff the candidate
/// satisfies every pin on `entry`; `Ok(false)` for an issuer-only
/// match that misses on another axis. Bubbles up parser errors only
/// when a GitHub Actions entry needs to read the subject and the
/// subject is unparseable as a GitHub Actions identity.
fn entry_admits(
    candidate: &SignatureIdentity,
    entry: &super::trust_list::TrustListEntry,
    parsed_cache: &mut Option<super::parse::GithubActionsSubject>,
) -> Result<bool, IdentityMatchError> {
    match &entry.origin {
        TrustOrigin::LocalKey { subject } => {
            Ok(candidate.issuer == LOCAL_KEY_ISSUER_SENTINEL && subject == &candidate.subject)
        }
        TrustOrigin::GithubActions {
            repo,
            workflow_path,
            tag_pattern,
        } => {
            if parsed_cache.is_none() {
                let parsed = parse_github_actions_subject(&candidate.subject)
                    .map_err(|source| IdentityMatchError::UnparseableGithubSubject { source })?;
                *parsed_cache = Some(parsed);
            }
            let parsed = parsed_cache.as_ref().expect("just populated");
            if &parsed.repo != repo || &parsed.workflow_path != workflow_path {
                return Ok(false);
            }
            Ok(parsed.tag().is_some_and(|t| tag_pattern.matches(t)))
        }
    }
}
