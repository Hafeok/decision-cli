//! `reasoning_effort` stakes mapping for `configurable_effort` capabilities per FT-063 / ADR-035.
//!
//! Pure function `compute_reasoning_effort(stakes, configurable_effort)` plus
//! the closed-vocabulary [`ReasoningEffort`] enum. The dispatcher's payload
//! assembly point (FT-061 / FT-062 dispatch loop) consumes this function and
//! injects the resulting wire string at `parameters.reasoning_effort` only
//! when the resolved capability has `configurable_effort = true`. Workers
//! see the value verbatim via the FT-060 `CallParams` shape; capabilities
//! without `configurable_effort` see the field absent and ignore it.
//!
//! The mapping table (ADR-035 §"Mapping"):
//!
//! | `bundle.stakes` | `reasoning_effort` |
//! |---|---|
//! | `routine`      | `low`    |
//! | `elevated`     | `medium` |
//! | `foundational` | `high`   |
//! | (reserved)     | `none`   — not currently produced by `compute_reasoning_effort`. |

use dec_graph::bundle::Stakes;

/// Closed reasoning-effort vocabulary mirrored from the Scaleway API
/// (`none` | `low` | `medium` | `high`). `None_` is reserved in the
/// vocabulary for explicit "skip reasoning" bindings; the FT-063 stakes
/// mapping never produces it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReasoningEffort {
    /// `"none"` — reserved; not produced by [`compute_reasoning_effort`].
    None_,
    /// `"low"` — produced for `Stakes::Routine`.
    Low,
    /// `"medium"` — produced for `Stakes::Elevated`.
    Medium,
    /// `"high"` — produced for `Stakes::Foundational`.
    High,
}

impl ReasoningEffort {
    /// Wire string accepted by the Scaleway `chat.completions.create`
    /// `reasoning_effort` kwarg (PRD §14 resolution).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None_ => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Compute the dispatch payload's `reasoning_effort` from the bundle's
/// stakes and the resolved capability's `configurable_effort` flag.
///
/// Returns `None` when the capability does not accept `reasoning_effort`,
/// regardless of stakes. Workers consuming the dispatch payload skip the
/// parameter when absent — Anthropic capabilities (which set
/// `configurable_effort = false`) therefore never see it on the wire.
///
/// The function is pure: no graph reads, no I/O, exhaustively closed over
/// [`Stakes`] (Rust's exhaustiveness check refuses to compile if a new
/// stakes variant is added without extending this match).
#[must_use]
pub const fn compute_reasoning_effort(
    stakes: Stakes,
    configurable_effort: bool,
) -> Option<ReasoningEffort> {
    if !configurable_effort {
        return None;
    }
    Some(match stakes {
        Stakes::Routine => ReasoningEffort::Low,
        Stakes::Elevated => ReasoningEffort::Medium,
        Stakes::Foundational => ReasoningEffort::High,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_renders_canonical_lower_case_literals() {
        assert_eq!(ReasoningEffort::None_.as_str(), "none");
        assert_eq!(ReasoningEffort::Low.as_str(), "low");
        assert_eq!(ReasoningEffort::Medium.as_str(), "medium");
        assert_eq!(ReasoningEffort::High.as_str(), "high");
    }

    #[test]
    fn non_configurable_capability_returns_none_for_every_stake() {
        for s in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
            assert_eq!(compute_reasoning_effort(s, false), None);
        }
    }

    #[test]
    fn configurable_capability_maps_stakes_per_adr_035_table() {
        assert_eq!(
            compute_reasoning_effort(Stakes::Routine, true),
            Some(ReasoningEffort::Low)
        );
        assert_eq!(
            compute_reasoning_effort(Stakes::Elevated, true),
            Some(ReasoningEffort::Medium)
        );
        assert_eq!(
            compute_reasoning_effort(Stakes::Foundational, true),
            Some(ReasoningEffort::High)
        );
    }

    #[test]
    fn compute_is_referentially_transparent() {
        for s in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
            for flag in [false, true] {
                let a = compute_reasoning_effort(s, flag);
                let b = compute_reasoning_effort(s, flag);
                assert_eq!(a, b);
            }
        }
    }
}
