//! Trust list of permitted Fulcio signing identities (FT-089 / ADR-058).
//!
//! Operator-curated, manually edited in slice 1 (ADR-058 explicitly defers
//! automatic management). Each entry pins three axes a candidate identity
//! must match: the GitHub repo, the workflow path, and the tag pattern.
//! A `TrustOrigin::LocalKey` entry is the development-only fallback —
//! ADR-058 admits local-key identities only if they are explicitly
//! enrolled by the operator.

use thiserror::Error;

/// Where the signing identity originated. Drives the axes the matcher
/// uses to compare a candidate identity against this entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrustOrigin {
    /// A GitHub Actions workflow run, identified by the OIDC subject
    /// the Sigstore Fulcio CA encodes onto the certificate. The matcher
    /// decomposes the subject into (repo, workflow path, ref) and
    /// matches each axis against this entry's pins.
    GithubActions {
        /// Repo slug in `owner/name` form. Case-sensitive (mirrors how
        /// GitHub encodes the subject URI).
        repo: String,
        /// Workflow path relative to the repo root. Typically
        /// `.github/workflows/release.yml`, or a reusable-workflow ref
        /// like `Hafeok/decision-cli/.github/workflows/release-worker.yml`.
        workflow_path: String,
        /// Tag pattern that the candidate's ref must match. See
        /// [`TagPattern`] for the supported wildcard semantics.
        tag_pattern: TagPattern,
    },
    /// Development-only fallback: a locally generated signing key whose
    /// subject is matched verbatim against the entry's `subject` field.
    /// The matcher ignores tag/workflow axes for this variant.
    LocalKey {
        /// The exact Fulcio subject the operator enrolled.
        subject: String,
    },
}

/// One entry in the trust list: an origin plus the expected issuer URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustListEntry {
    /// Origin pins (repo + workflow + tag, or local-key subject).
    pub origin: TrustOrigin,
    /// Expected Sigstore OIDC issuer URI. For GitHub Actions this is
    /// `https://token.actions.githubusercontent.com`; for local keys
    /// the operator chooses a stable sentinel (see
    /// [`super::LOCAL_KEY_ISSUER_SENTINEL`]).
    pub issuer: String,
    /// Optional operator note (free-form). Surfaced verbatim in
    /// match-outcome diagnostics so the audit trail records *why* an
    /// entry exists.
    pub note: Option<String>,
}

/// Ordered, immutable trust list. The matcher iterates entries in
/// declaration order; the first matching entry wins. Duplicate entries
/// are tolerated but redundant — `validate` flags them so the operator
/// can prune.
#[derive(Debug, Clone, Default)]
pub struct TrustList {
    entries: Vec<TrustListEntry>,
}

impl TrustList {
    /// Construct an empty trust list. Convenience for tests; production
    /// callers seed via [`Self::from_entries`].
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build a trust list from a sequence of entries. Returns an error
    /// if any entry is malformed (e.g. empty issuer URI, empty repo).
    /// Duplicates are accepted but produce a warning at validate time.
    pub fn from_entries(entries: Vec<TrustListEntry>) -> Result<Self, TrustListError> {
        for (idx, entry) in entries.iter().enumerate() {
            validate_entry(idx, entry)?;
        }
        Ok(Self { entries })
    }

    /// Borrow the underlying entry slice (declaration order preserved).
    #[must_use]
    pub fn entries(&self) -> &[TrustListEntry] {
        &self.entries
    }

    /// Number of entries in the trust list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True iff the trust list has no entries. An empty trust list
    /// admits no signature identity — every candidate is
    /// `untrusted-identity`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Tag-pattern matcher. ADR-058's third axis. Supports a single
/// trailing or interior `*` wildcard per dotted segment; the wildcard
/// matches zero-or-more characters within that segment. Examples:
///
/// - `implementer-v*.*.*` matches `implementer-v1.2.3`, `implementer-v0.10.0`,
///   but not `implementer-v1` (insufficient segments).
/// - `v1.*` matches `v1.0`, `v1.10`, but not `v2.0`.
/// - `release` (no wildcard) matches only the literal tag `release`.
///
/// The matcher is intentionally simple — full regex support would
/// invite over-permissive entries. If a worker needs richer matching,
/// the answer is to split the entry into two narrower ones.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagPattern {
    raw: String,
    segments: Vec<TagSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TagSegment {
    /// Literal-text segment (no wildcards).
    Literal(String),
    /// Pattern segment with one or more wildcards. Stored as the
    /// segment's pieces split on `*`; matches by checking prefix,
    /// each middle piece in order, and suffix.
    Glob {
        prefix: String,
        middles: Vec<String>,
        suffix: String,
    },
}

impl TagPattern {
    /// Parse a tag pattern string. Returns an error for empty patterns
    /// or patterns containing unsupported characters.
    pub fn parse(raw: impl Into<String>) -> Result<Self, TagPatternError> {
        let raw_string = raw.into();
        if raw_string.is_empty() {
            return Err(TagPatternError::Empty);
        }
        let segments = raw_string
            .split('.')
            .map(parse_segment)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            raw: raw_string,
            segments,
        })
    }

    /// Return the pattern's raw textual form (useful for diagnostics).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Check whether a candidate tag (the value past `refs/tags/`)
    /// matches this pattern.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        let candidate_segments: Vec<&str> = candidate.split('.').collect();
        if candidate_segments.len() != self.segments.len() {
            return false;
        }
        self.segments
            .iter()
            .zip(candidate_segments.iter())
            .all(|(pat, cand)| segment_matches(pat, cand))
    }
}

fn parse_segment(raw: &str) -> Result<TagSegment, TagPatternError> {
    if raw.is_empty() {
        return Err(TagPatternError::EmptySegment);
    }
    if !raw.contains('*') {
        return Ok(TagSegment::Literal(raw.to_string()));
    }
    let pieces: Vec<&str> = raw.split('*').collect();
    // pieces.len() ≥ 2 (because the segment contains at least one '*').
    let prefix = pieces[0].to_string();
    let suffix = pieces[pieces.len() - 1].to_string();
    let middles: Vec<String> = pieces[1..pieces.len() - 1]
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| (*p).to_string())
        .collect();
    Ok(TagSegment::Glob {
        prefix,
        middles,
        suffix,
    })
}

fn segment_matches(pat: &TagSegment, cand: &str) -> bool {
    match pat {
        TagSegment::Literal(lit) => lit == cand,
        TagSegment::Glob {
            prefix,
            middles,
            suffix,
        } => {
            if !cand.starts_with(prefix.as_str()) {
                return false;
            }
            if !cand.ends_with(suffix.as_str()) {
                return false;
            }
            // Ensure prefix and suffix do not overlap.
            if cand.len() < prefix.len() + suffix.len() {
                return false;
            }
            let mut cursor = &cand[prefix.len()..cand.len() - suffix.len()];
            for middle in middles {
                match cursor.find(middle.as_str()) {
                    Some(pos) => {
                        cursor = &cursor[pos + middle.len()..];
                    }
                    None => return false,
                }
            }
            true
        }
    }
}

fn validate_entry(idx: usize, entry: &TrustListEntry) -> Result<(), TrustListError> {
    if entry.issuer.trim().is_empty() {
        return Err(TrustListError::EmptyIssuer { index: idx });
    }
    match &entry.origin {
        TrustOrigin::GithubActions {
            repo,
            workflow_path,
            ..
        } => {
            if repo.trim().is_empty() {
                return Err(TrustListError::EmptyRepo { index: idx });
            }
            if !repo.contains('/') {
                return Err(TrustListError::MalformedRepo {
                    index: idx,
                    repo: repo.clone(),
                });
            }
            if workflow_path.trim().is_empty() {
                return Err(TrustListError::EmptyWorkflowPath { index: idx });
            }
        }
        TrustOrigin::LocalKey { subject } => {
            if subject.trim().is_empty() {
                return Err(TrustListError::EmptyLocalKeySubject { index: idx });
            }
        }
    }
    Ok(())
}

/// Errors raised while constructing a [`TrustList`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrustListError {
    /// Entry has an empty `issuer` URI.
    #[error("trust list entry {index}: issuer URI must not be empty")]
    EmptyIssuer {
        /// Zero-based index of the offending entry.
        index: usize,
    },
    /// GitHub Actions entry has an empty `repo` field.
    #[error("trust list entry {index}: GitHub repo must not be empty")]
    EmptyRepo {
        /// Zero-based index of the offending entry.
        index: usize,
    },
    /// GitHub Actions entry has a `repo` field missing the `owner/name` separator.
    #[error("trust list entry {index}: GitHub repo {repo:?} must be in `owner/name` form")]
    MalformedRepo {
        /// Zero-based index of the offending entry.
        index: usize,
        /// The malformed repo value (echoed for operator diagnosis).
        repo: String,
    },
    /// GitHub Actions entry has an empty `workflow_path` field.
    #[error("trust list entry {index}: workflow path must not be empty")]
    EmptyWorkflowPath {
        /// Zero-based index of the offending entry.
        index: usize,
    },
    /// Local-key entry has an empty `subject` field.
    #[error("trust list entry {index}: local-key subject must not be empty")]
    EmptyLocalKeySubject {
        /// Zero-based index of the offending entry.
        index: usize,
    },
}

/// Errors raised while parsing a [`TagPattern`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TagPatternError {
    /// Pattern string was empty.
    #[error("tag pattern must not be empty")]
    Empty,
    /// A dotted segment was empty (e.g. `v1..3` or trailing dot).
    #[error("tag pattern segment must not be empty")]
    EmptySegment,
}
