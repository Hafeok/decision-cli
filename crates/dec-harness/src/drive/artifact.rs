//! `ArtifactRef` + `ArtifactKind` parser.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Closed enumeration of artifact kinds the orchestrator can drive. New
/// kinds append here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    /// `FT-NNN` feature spec.
    Feature,
    /// `TC-NNN` test criterion.
    TestCriterion,
    /// `VG-NNN[-suffix]` verification graph.
    VerificationGraph,
    /// `BNCH-NNN[-suffix]` verification environment.
    Environment,
    /// `ADR-NNN` architectural decision record.
    Adr,
}

impl ArtifactKind {
    /// Human-renderable tag used in the planner-registry hint.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feature => "Feature",
            Self::TestCriterion => "TestCriterion",
            Self::VerificationGraph => "VerificationGraph",
            Self::Environment => "Environment",
            Self::Adr => "Adr",
        }
    }
}

/// Parsed reference to a concrete artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// Which artifact-class this reference is for.
    pub kind: ArtifactKind,
    /// The supplied short id verbatim (e.g. `FT-019` or
    /// `BNCH-001-ephemeral-cli`).
    pub short_id: String,
}

/// Parser errors. Stable enough that the driver's error renderer can
/// downcast.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    /// Supplied string was empty.
    #[error("artifact id is empty; expected a prefix like FT-, TC-, VG-, ENV-, ADR-")]
    Empty,
    /// String didn't carry a recognised prefix.
    #[error("unsupported artifact prefix in {input:?}; supported: FT-, TC-, VG-, ENV-, ADR-")]
    UnknownPrefix {
        /// The raw input string.
        input: String,
    },
    /// Prefix was right but the part after the dash failed validation.
    #[error("malformed artifact id {input:?} ({why})")]
    Malformed {
        /// The raw input string.
        input: String,
        /// Why it's malformed.
        why: &'static str,
    },
}

impl ArtifactRef {
    /// Parse a short id like `FT-019` or `BNCH-001-ephemeral-cli` into
    /// the typed reference.
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        // The prefix is everything up to and including the first `-`.
        // We support the small fixed set; anything else is rejected.
        let kind = if s.starts_with("FT-") {
            ArtifactKind::Feature
        } else if s.starts_with("TC-") {
            ArtifactKind::TestCriterion
        } else if s.starts_with("VG-") {
            ArtifactKind::VerificationGraph
        } else if s.starts_with("ENV-") || s.starts_with("BNCH-") {
            // FT-112 renamed verification ENV-NNN to BNCH-NNN; both parse
            // so pre-rename ids in stores and scripts stay addressable.
            ArtifactKind::Environment
        } else if s.starts_with("ADR-") {
            ArtifactKind::Adr
        } else {
            return Err(ParseError::UnknownPrefix {
                input: s.to_string(),
            });
        };
        // Body after the prefix must be non-empty and start with a digit.
        let dash_idx = s.find('-').expect("prefix matched ⇒ at least one dash");
        let body = &s[dash_idx + 1..];
        if body.is_empty() {
            return Err(ParseError::Malformed {
                input: s.to_string(),
                why: "missing numeric body after prefix",
            });
        }
        if !body
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return Err(ParseError::Malformed {
                input: s.to_string(),
                why: "body after prefix must start with a digit",
            });
        }
        Ok(Self {
            kind,
            short_id: s.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_prefixes() {
        for (s, expected) in [
            ("FT-019", ArtifactKind::Feature),
            ("TC-027", ArtifactKind::TestCriterion),
            ("VG-100", ArtifactKind::VerificationGraph),
            ("ENV-002", ArtifactKind::Environment),
            ("BNCH-001-ephemeral-cli", ArtifactKind::Environment),
            ("ADR-066", ArtifactKind::Adr),
        ] {
            let r = ArtifactRef::parse(s).expect(s);
            assert_eq!(r.kind, expected, "{s}");
            assert_eq!(r.short_id, s);
        }
    }

    #[test]
    fn rejects_unknown_prefix() {
        assert!(matches!(
            ArtifactRef::parse("XX-000"),
            Err(ParseError::UnknownPrefix { .. })
        ));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(ArtifactRef::parse(""), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_missing_dash() {
        assert!(matches!(
            ArtifactRef::parse("FT019"),
            Err(ParseError::UnknownPrefix { .. })
        ));
    }

    #[test]
    fn rejects_missing_body() {
        assert!(matches!(
            ArtifactRef::parse("FT-"),
            Err(ParseError::Malformed { .. })
        ));
    }

    #[test]
    fn rejects_non_digit_body() {
        assert!(matches!(
            ArtifactRef::parse("FT-abc"),
            Err(ParseError::Malformed { .. })
        ));
    }

    #[test]
    fn rejects_lowercase() {
        assert!(matches!(
            ArtifactRef::parse("feature-019"),
            Err(ParseError::UnknownPrefix { .. })
        ));
    }
}
