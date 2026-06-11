//! Feedback lifecycle state machine (FT-027 / ADR-024).
//!
//! Encodes the seven-state machine from ADR-024 as a typed `LifecycleState`
//! enum plus a small validator. The transition table is the single source
//! of truth: SHACL `sh:in` and `sh:sparql` constraints in `shapes.ttl`
//! mirror the wire strings here, and [`StreamWriter`] consults
//! [`validate_transition`] before persisting any mutation that updates
//! `dec:lifecycleState` on an existing `dec:Feedback`.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-029 routing, FT-031 worker harness, FT-033 CLI close) imports
//! from this module — no consumer imports from sibling features.

use thiserror::Error;

/// One of the seven `dec:lifecycleState` values per ADR-024.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    /// Just emitted by an action session; not yet routed.
    Produced,
    /// Orchestrator has matched the feedback to a target role.
    Routed,
    /// Target role has begun work on the feedback.
    Received,
    /// Target role produced an artifact intended to resolve the feedback.
    Addressed,
    /// Emitter (or closer) confirmed the addressing artifact resolves. Terminal.
    Closed,
    /// Target role determined the feedback is invalid. Terminal.
    Rejected,
    /// A later feedback artifact subsumes this one. Terminal.
    Superseded,
}

impl LifecycleState {
    /// Stable wire string mirroring the SHACL `sh:in` list.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Produced => "produced",
            Self::Routed => "routed",
            Self::Received => "received",
            Self::Addressed => "addressed",
            Self::Closed => "closed",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }

    /// Parse from the wire string. Unknown values return `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "produced" => Some(Self::Produced),
            "routed" => Some(Self::Routed),
            "received" => Some(Self::Received),
            "addressed" => Some(Self::Addressed),
            "closed" => Some(Self::Closed),
            "rejected" => Some(Self::Rejected),
            "superseded" => Some(Self::Superseded),
            _ => None,
        }
    }

    /// True if this state is terminal (closed / rejected / superseded).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed | Self::Rejected | Self::Superseded)
    }

    /// Every valid lifecycle state — useful for SHACL `sh:in` generation
    /// and exhaustive tests.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Produced,
            Self::Routed,
            Self::Received,
            Self::Addressed,
            Self::Closed,
            Self::Rejected,
            Self::Superseded,
        ]
    }
}

/// Transition errors surfaced by [`validate_transition`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TransitionError {
    /// Caller asked for a transition that is not in the next-states set.
    #[error("invalid lifecycle transition: {} → {}", from.as_str(), to.as_str())]
    InvalidTransition {
        /// State the feedback was in prior to the attempted mutation.
        from: LifecycleState,
        /// State the mutation tried to move it to.
        to: LifecycleState,
    },
    /// Caller attempted a transition out of a terminal state.
    #[error("feedback is in terminal state {}; no further transitions allowed", state.as_str())]
    TerminalState {
        /// The terminal state.
        state: LifecycleState,
    },
    /// Target state requires a companion field that is missing.
    #[error("transition to {} requires field {field}", state.as_str())]
    MissingField {
        /// The state being transitioned to.
        state: LifecycleState,
        /// The missing field name (e.g. `dec:addressingArtifact`).
        field: &'static str,
    },
    /// Wire-string did not match a known lifecycle state.
    #[error("unknown lifecycle state: {0:?}")]
    UnknownState(String),
}

/// Return the set of states `from` may transition to. Terminal states
/// yield an empty slice. Source of truth — mirrors the ADR-024 table.
#[must_use]
pub fn next_states(from: LifecycleState) -> &'static [LifecycleState] {
    use LifecycleState::{Addressed, Closed, Received, Rejected, Routed, Superseded};
    match from {
        LifecycleState::Produced => &[Routed, Superseded],
        LifecycleState::Routed => &[Received, Superseded],
        LifecycleState::Received => &[Addressed, Rejected],
        LifecycleState::Addressed => &[Closed, Rejected],
        // Terminal states.
        LifecycleState::Closed | LifecycleState::Rejected | LifecycleState::Superseded => &[],
    }
}

/// Validate a transition `from → to` against the ADR-024 table.
///
/// Returns `Ok(())` if `to` is in `next_states(from)`. Returns
/// `TransitionError::TerminalState` if `from` is terminal (yielding an
/// empty next-states set), and `TransitionError::InvalidTransition`
/// otherwise.
pub fn validate_transition(
    from: LifecycleState,
    to: LifecycleState,
) -> Result<(), TransitionError> {
    let allowed = next_states(from);
    if allowed.is_empty() {
        return Err(TransitionError::TerminalState { state: from });
    }
    if allowed.contains(&to) {
        Ok(())
    } else {
        Err(TransitionError::InvalidTransition { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions_validate() {
        for (from, to) in [
            (LifecycleState::Produced, LifecycleState::Routed),
            (LifecycleState::Routed, LifecycleState::Received),
            (LifecycleState::Received, LifecycleState::Addressed),
            (LifecycleState::Addressed, LifecycleState::Closed),
        ] {
            validate_transition(from, to).expect("happy-path transition");
        }
    }

    #[test]
    fn rejection_paths_validate() {
        validate_transition(LifecycleState::Received, LifecycleState::Rejected)
            .expect("received → rejected");
        validate_transition(LifecycleState::Addressed, LifecycleState::Rejected)
            .expect("addressed → rejected");
    }

    #[test]
    fn supersession_paths_validate() {
        validate_transition(LifecycleState::Produced, LifecycleState::Superseded)
            .expect("produced → superseded");
        validate_transition(LifecycleState::Routed, LifecycleState::Superseded)
            .expect("routed → superseded");
    }

    #[test]
    fn invalid_reverse_transitions_fail() {
        // Reverse transitions are explicitly forbidden by ADR-024.
        let err = validate_transition(LifecycleState::Routed, LifecycleState::Produced)
            .expect_err("reverse transition must fail");
        assert!(matches!(err, TransitionError::InvalidTransition { .. }));
    }

    #[test]
    fn jumping_states_is_refused() {
        let err = validate_transition(LifecycleState::Produced, LifecycleState::Addressed)
            .expect_err("produced → addressed skips routed/received");
        assert!(matches!(err, TransitionError::InvalidTransition { .. }));
    }

    #[test]
    fn terminal_states_have_no_next_states() {
        for terminal in [
            LifecycleState::Closed,
            LifecycleState::Rejected,
            LifecycleState::Superseded,
        ] {
            assert!(next_states(terminal).is_empty());
            assert!(terminal.is_terminal());
            let err = validate_transition(terminal, LifecycleState::Produced)
                .expect_err("terminal must refuse any outgoing transition");
            assert!(matches!(err, TransitionError::TerminalState { state } if state == terminal));
        }
    }

    #[test]
    fn parse_round_trips() {
        for s in LifecycleState::all() {
            assert_eq!(LifecycleState::parse(s.as_str()), Some(*s));
        }
        assert!(LifecycleState::parse("done").is_none());
    }
}
