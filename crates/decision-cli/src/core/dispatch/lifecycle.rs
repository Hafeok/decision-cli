//! DispatchGroup lifecycle state machine (FT-021 §Behaviour, ADR-017).
//!
//! Mirrors the SHACL `sh:in` enumeration on `dec:dispatchStatus` plus the
//! transition rules per FT-021 §Outputs. The Rust API rejects bad
//! transitions at call time; SHACL remains the durable backstop.

use thiserror::Error;

use crate::core::vocab::{
    DISPATCH_STATUS_ACTION_FAILED, DISPATCH_STATUS_AWAITING_ACTION,
    DISPATCH_STATUS_AWAITING_AMENDMENT, DISPATCH_STATUS_AWAITING_INTERPRETATION,
    DISPATCH_STATUS_COMPLETE, DISPATCH_STATUS_INTERPRETATION_FAILED,
    DISPATCH_STATUS_INTERPRETATION_REJECTED, DISPATCH_STATUS_INTERPRETATION_RUNNING,
};

/// DispatchGroup lifecycle state per FT-021 §Outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchStatus {
    /// Mint-time state; action session in flight.
    AwaitingAction,
    /// Action terminated successfully; verifier not yet dispatched.
    AwaitingInterpretation,
    /// Interpretation session is running.
    InterpretationRunning,
    /// Verifier returned `rejected`; terminal.
    InterpretationRejected,
    /// Verifier returned `amendment-required`; awaiting next dispatch.
    AwaitingAmendment,
    /// Action session failed terminally (worker crash / non-zero exit).
    ActionFailed,
    /// Interpretation session failed terminally.
    InterpretationFailed,
    /// Dispatch complete: action produced + verifier approved.
    Complete,
}

impl DispatchStatus {
    /// Stable wire string used in the `dec:dispatchStatus` literal.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AwaitingAction => DISPATCH_STATUS_AWAITING_ACTION,
            Self::AwaitingInterpretation => DISPATCH_STATUS_AWAITING_INTERPRETATION,
            Self::InterpretationRunning => DISPATCH_STATUS_INTERPRETATION_RUNNING,
            Self::InterpretationRejected => DISPATCH_STATUS_INTERPRETATION_REJECTED,
            Self::AwaitingAmendment => DISPATCH_STATUS_AWAITING_AMENDMENT,
            Self::ActionFailed => DISPATCH_STATUS_ACTION_FAILED,
            Self::InterpretationFailed => DISPATCH_STATUS_INTERPRETATION_FAILED,
            Self::Complete => DISPATCH_STATUS_COMPLETE,
        }
    }

    /// Parse a status from its wire string. Returns `None` for unknown
    /// inputs (the SHACL `sh:in` constraint rejects them at commit time).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            DISPATCH_STATUS_AWAITING_ACTION => Some(Self::AwaitingAction),
            DISPATCH_STATUS_AWAITING_INTERPRETATION => Some(Self::AwaitingInterpretation),
            DISPATCH_STATUS_INTERPRETATION_RUNNING => Some(Self::InterpretationRunning),
            DISPATCH_STATUS_INTERPRETATION_REJECTED => Some(Self::InterpretationRejected),
            DISPATCH_STATUS_AWAITING_AMENDMENT => Some(Self::AwaitingAmendment),
            DISPATCH_STATUS_ACTION_FAILED => Some(Self::ActionFailed),
            DISPATCH_STATUS_INTERPRETATION_FAILED => Some(Self::InterpretationFailed),
            DISPATCH_STATUS_COMPLETE => Some(Self::Complete),
            _ => None,
        }
    }

    /// True iff the state is terminal (no outgoing lifecycle transitions).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Complete
                | Self::InterpretationRejected
                | Self::AwaitingAmendment
                | Self::ActionFailed
                | Self::InterpretationFailed
        )
    }
}

/// Events that drive the DispatchGroup state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchEvent {
    /// Action session terminated with a produced artifact.
    ActionCompleted,
    /// Action session crashed / exited non-zero.
    ActionFailed,
    /// Verifier dispatch began (interpretation session opened).
    InterpretationStarted,
    /// Verifier returned `approved`.
    VerdictApproved,
    /// Verifier returned `rejected`.
    VerdictRejected,
    /// Verifier returned `amendment-required`.
    VerdictAmendmentRequired,
    /// Verifier session failed terminally.
    InterpretationFailed,
}

/// Errors produced by [`next`] when the transition is invalid.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Cannot drive `event` from `current`; transition not in the allowed set.
    #[error("invalid DispatchGroup transition from {current:?} on {event:?}")]
    InvalidTransition {
        /// Originating status.
        current: DispatchStatus,
        /// Attempted event.
        event: DispatchEvent,
    },
}

/// Compute the next [`DispatchStatus`] for a `(current, event)` pair.
///
/// Mirrors FT-021 §Outputs / §Behaviour and the SHACL `sh:in` set.
///
/// # Errors
/// Returns [`LifecycleError::InvalidTransition`] when the event is not
/// valid from `current`.
pub fn next(
    current: DispatchStatus,
    event: DispatchEvent,
) -> Result<DispatchStatus, LifecycleError> {
    use DispatchEvent as E;
    use DispatchStatus as S;
    let target = match (current, event) {
        (S::AwaitingAction, E::ActionCompleted) => S::AwaitingInterpretation,
        (S::AwaitingAction, E::ActionFailed) => S::ActionFailed,
        (S::AwaitingInterpretation, E::InterpretationStarted) => S::InterpretationRunning,
        (S::AwaitingInterpretation, E::InterpretationFailed) => S::InterpretationFailed,
        // Verifier can return a verdict directly from `awaiting-interpretation`
        // when the interpretation session is synchronous (slice-2 default).
        (S::AwaitingInterpretation, E::VerdictApproved)
        | (S::InterpretationRunning, E::VerdictApproved) => S::Complete,
        (S::AwaitingInterpretation, E::VerdictRejected)
        | (S::InterpretationRunning, E::VerdictRejected) => S::InterpretationRejected,
        (S::AwaitingInterpretation, E::VerdictAmendmentRequired)
        | (S::InterpretationRunning, E::VerdictAmendmentRequired) => S::AwaitingAmendment,
        (S::InterpretationRunning, E::InterpretationFailed) => S::InterpretationFailed,
        // Anything else is invalid; terminal states never transition.
        _ => {
            return Err(LifecycleError::InvalidTransition { current, event });
        }
    };
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_action_then_approved() {
        let s = next(DispatchStatus::AwaitingAction, DispatchEvent::ActionCompleted)
            .expect("action ok");
        assert_eq!(s, DispatchStatus::AwaitingInterpretation);
        let s = next(s, DispatchEvent::VerdictApproved).expect("approved");
        assert_eq!(s, DispatchStatus::Complete);
        assert!(s.is_terminal());
    }

    #[test]
    fn rejected_verdict_lands_on_interpretation_rejected() {
        let s = next(
            DispatchStatus::AwaitingInterpretation,
            DispatchEvent::VerdictRejected,
        )
        .expect("rejected");
        assert_eq!(s, DispatchStatus::InterpretationRejected);
        assert!(s.is_terminal());
    }

    #[test]
    fn amendment_lands_on_awaiting_amendment() {
        let s = next(
            DispatchStatus::AwaitingInterpretation,
            DispatchEvent::VerdictAmendmentRequired,
        )
        .expect("amend");
        assert_eq!(s, DispatchStatus::AwaitingAmendment);
        assert!(s.is_terminal());
    }

    #[test]
    fn action_failed_short_circuits_verifier() {
        let s = next(DispatchStatus::AwaitingAction, DispatchEvent::ActionFailed)
            .expect("action failed");
        assert_eq!(s, DispatchStatus::ActionFailed);
        // No further transitions allowed.
        let err = next(s, DispatchEvent::VerdictApproved).expect_err("terminal");
        assert!(matches!(err, LifecycleError::InvalidTransition { .. }));
    }

    #[test]
    fn terminal_states_refuse_further_transitions() {
        for &terminal in &[
            DispatchStatus::Complete,
            DispatchStatus::InterpretationRejected,
            DispatchStatus::AwaitingAmendment,
            DispatchStatus::ActionFailed,
            DispatchStatus::InterpretationFailed,
        ] {
            assert!(terminal.is_terminal());
            for &event in &[
                DispatchEvent::ActionCompleted,
                DispatchEvent::ActionFailed,
                DispatchEvent::VerdictApproved,
                DispatchEvent::VerdictRejected,
                DispatchEvent::VerdictAmendmentRequired,
                DispatchEvent::InterpretationStarted,
                DispatchEvent::InterpretationFailed,
            ] {
                assert!(
                    next(terminal, event).is_err(),
                    "terminal {terminal:?} should refuse {event:?}"
                );
            }
        }
    }

    #[test]
    fn complete_requires_an_approved_path_event() {
        // Only VerdictApproved leads to Complete from awaiting-interpretation.
        for &e in &[
            DispatchEvent::ActionCompleted,
            DispatchEvent::ActionFailed,
        ] {
            assert!(next(DispatchStatus::AwaitingInterpretation, e).is_err());
        }
        let s = next(
            DispatchStatus::AwaitingInterpretation,
            DispatchEvent::VerdictApproved,
        )
        .expect("approved");
        assert_eq!(s, DispatchStatus::Complete);
    }

    #[test]
    fn status_round_trips_through_wire_strings() {
        for &s in &[
            DispatchStatus::AwaitingAction,
            DispatchStatus::AwaitingInterpretation,
            DispatchStatus::InterpretationRunning,
            DispatchStatus::InterpretationRejected,
            DispatchStatus::AwaitingAmendment,
            DispatchStatus::ActionFailed,
            DispatchStatus::InterpretationFailed,
            DispatchStatus::Complete,
        ] {
            assert_eq!(DispatchStatus::parse(s.as_str()), Some(s));
        }
        assert!(DispatchStatus::parse("nonsense").is_none());
    }
}
