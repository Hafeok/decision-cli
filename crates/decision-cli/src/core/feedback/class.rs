//! Feedback controlled vocabulary (FT-028 / ADR-023).
//!
//! The six-value `dec:feedbackClass` vocabulary lives here as the typed
//! `FeedbackClass` enum. The IRI wire strings (matching SHACL `sh:in` in
//! `shapes.ttl`) are the source of truth — the Rust enum and the SHACL
//! `sh:in` list are kept in lockstep by the ADR-023 amendment procedure.
//!
//! `Disposition` (ADR-025) is colocated here because the class controls
//! the default disposition; FT-032 layers blocking semantics on top of
//! this primitive without redefining the per-class defaults.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-029 routing, FT-031 worker SDK, FT-032 dispatch interaction)
//! imports from this module — no consumer imports from sibling features.

use serde::{Deserialize, Serialize};

/// Default blocking disposition for an emitted feedback artifact (ADR-025).
///
/// Tagged on the class via `FeedbackClass::default_disposition`. Workers
/// may override at emission time (recorded as `dec:dispositionOverride`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// Emission pauses the emitting dispatch until the feedback is
    /// addressed or rejected (ADR-025).
    Blocking,
    /// Emission flows in parallel; the emitting dispatch proceeds.
    NonBlocking,
}

impl Disposition {
    /// Stable wire string (`blocking` / `non-blocking`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::NonBlocking => "non-blocking",
        }
    }

    /// Parse the wire string. Unknown values resolve to `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "blocking" => Some(Self::Blocking),
            "non-blocking" => Some(Self::NonBlocking),
            _ => None,
        }
    }
}

/// Controlled vocabulary for `dec:feedbackClass` per ADR-023.
///
/// Six values, enforced by SHACL `sh:in` and refused at write time.
/// Adding a class requires the ADR-023 amendment procedure: extend the
/// `sh:in` list in `shapes.ttl`, extend this enum, extend the routing
/// table ([ADR-026](ADR-026)), and update worker SDKs — all in one
/// atomic request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeedbackClass {
    /// Missing information; the bundle does not justify a confident action.
    Gap,
    /// Inconsistent inputs; two bundled artifacts disagree on a load-bearing point.
    Contradiction,
    /// Tools or inputs insufficient to produce what is asked for.
    Unimplementable,
    /// Out-of-bounds work the action proceeds without (non-blocking).
    ScopeIssue,
    /// Action self-discovered error post-production.
    Defect,
    /// Missing tool / API / artifact type that would help future work.
    CapabilityRequest,
}

impl FeedbackClass {
    /// IRI wire string for `dec:feedbackClass` literals (matches `sh:in`).
    #[must_use]
    pub const fn as_iri_value(&self) -> &'static str {
        match self {
            Self::Gap => "gap",
            Self::Contradiction => "contradiction",
            Self::Unimplementable => "unimplementable",
            Self::ScopeIssue => "scope-issue",
            Self::Defect => "defect",
            Self::CapabilityRequest => "capability-request",
        }
    }

    /// Parse from the wire string. Unknown values resolve to `None`
    /// (the SHACL `sh:in` constraint rejects them at commit time).
    #[must_use]
    pub fn from_iri_value(s: &str) -> Option<Self> {
        match s {
            "gap" => Some(Self::Gap),
            "contradiction" => Some(Self::Contradiction),
            "unimplementable" => Some(Self::Unimplementable),
            "scope-issue" => Some(Self::ScopeIssue),
            "defect" => Some(Self::Defect),
            "capability-request" => Some(Self::CapabilityRequest),
            _ => None,
        }
    }

    /// Default target role per ADR-026 §Routing table.
    ///
    /// `defect` defaults to the `verifier` role; ADR-026 documents the
    /// "self-routing" escape hatch for the case where no verifier verdict
    /// exists. The routing layer in FT-029 applies the escape hatch
    /// post-hoc; this default reflects the table's first column.
    #[must_use]
    pub const fn default_target_role(&self) -> &'static str {
        match self {
            Self::Gap | Self::Unimplementable => "spec-author",
            Self::Contradiction | Self::CapabilityRequest => "architect",
            Self::ScopeIssue => "slice-curator",
            Self::Defect => "verifier",
        }
    }

    /// Default blocking disposition per ADR-023 / ADR-025.
    #[must_use]
    pub const fn default_disposition(&self) -> Disposition {
        match self {
            Self::Gap | Self::Contradiction | Self::Unimplementable => Disposition::Blocking,
            Self::ScopeIssue | Self::Defect | Self::CapabilityRequest => Disposition::NonBlocking,
        }
    }

    /// Every class — useful for iteration, seeding, and exhaustive tests.
    #[must_use]
    pub const fn all() -> &'static [FeedbackClass] {
        &[
            Self::Gap,
            Self::Contradiction,
            Self::Unimplementable,
            Self::ScopeIssue,
            Self::Defect,
            Self::CapabilityRequest,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{Disposition, FeedbackClass};

    #[test]
    fn iri_values_round_trip() {
        for c in FeedbackClass::all() {
            assert_eq!(
                FeedbackClass::from_iri_value(c.as_iri_value()),
                Some(*c),
                "round-trip failed for {:?}",
                c
            );
        }
    }

    #[test]
    fn all_six_iri_values_are_present() {
        let expected = ["gap", "contradiction", "unimplementable",
                        "scope-issue", "defect", "capability-request"];
        let actual: Vec<&'static str> = FeedbackClass::all()
            .iter()
            .map(|c| c.as_iri_value())
            .collect();
        for value in expected {
            assert!(
                actual.contains(&value),
                "expected {value} in FeedbackClass::all()"
            );
        }
        assert_eq!(actual.len(), expected.len());
    }

    #[test]
    fn unknown_iri_value_resolves_to_none() {
        assert!(FeedbackClass::from_iri_value("unknown").is_none());
        assert!(FeedbackClass::from_iri_value("").is_none());
        // Underscore variant of "scope-issue" must not match — kebab-case is the wire form.
        assert!(FeedbackClass::from_iri_value("scope_issue").is_none());
    }

    #[test]
    fn default_target_role_matches_adr_026() {
        assert_eq!(FeedbackClass::Gap.default_target_role(), "spec-author");
        assert_eq!(FeedbackClass::Contradiction.default_target_role(), "architect");
        assert_eq!(FeedbackClass::Unimplementable.default_target_role(), "spec-author");
        assert_eq!(FeedbackClass::ScopeIssue.default_target_role(), "slice-curator");
        assert_eq!(FeedbackClass::Defect.default_target_role(), "verifier");
        assert_eq!(
            FeedbackClass::CapabilityRequest.default_target_role(),
            "architect"
        );
    }

    #[test]
    fn default_disposition_matches_adr_023_and_025() {
        for blocking in [
            FeedbackClass::Gap,
            FeedbackClass::Contradiction,
            FeedbackClass::Unimplementable,
        ] {
            assert_eq!(
                blocking.default_disposition(),
                Disposition::Blocking,
                "{blocking:?} must default to blocking"
            );
        }
        for non_blocking in [
            FeedbackClass::ScopeIssue,
            FeedbackClass::Defect,
            FeedbackClass::CapabilityRequest,
        ] {
            assert_eq!(
                non_blocking.default_disposition(),
                Disposition::NonBlocking,
                "{non_blocking:?} must default to non-blocking"
            );
        }
    }

    #[test]
    fn disposition_round_trips() {
        for d in [Disposition::Blocking, Disposition::NonBlocking] {
            assert_eq!(Disposition::parse(d.as_str()), Some(d));
        }
        assert!(Disposition::parse("paused").is_none());
    }

    #[test]
    fn all_returns_six_entries() {
        assert_eq!(FeedbackClass::all().len(), 6);
    }
}
