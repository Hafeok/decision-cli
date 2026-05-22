//! DispatchGroup lifecycle state machine (FT-021 §Behaviour, ADR-017).
//!
//! Mirrors the SHACL `sh:in` enumeration on `dec:dispatchStatus` plus the
//! transition rules per FT-021 §Outputs. The Rust API rejects bad
//! transitions at call time; SHACL remains the durable backstop.

use thiserror::Error;

use crate::core::vocab::{
    DISPATCH_STATUS_ACTION_FAILED, DISPATCH_STATUS_AWAITING_ACTION,
    DISPATCH_STATUS_AWAITING_AMENDMENT, DISPATCH_STATUS_AWAITING_INTERPRETATION,
    DISPATCH_STATUS_COMPLETE, DISPATCH_STATUS_FEEDBACK_REJECTED_ACTION_BLOCKED,
    DISPATCH_STATUS_INTERPRETATION_FAILED, DISPATCH_STATUS_INTERPRETATION_REJECTED,
    DISPATCH_STATUS_INTERPRETATION_RUNNING, DISPATCH_STATUS_PAUSED_FOR_FEEDBACK,
};

/// DispatchGroup lifecycle state per FT-021 §Outputs and FT-032 §Outputs.
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
    /// FT-032 / ADR-025: a worker emitted blocking feedback. The action
    /// is structurally aborted; the dispatch refuses to advance until
    /// every blocking feedback in `dec:blockedBy` is terminal.
    PausedForFeedback,
    /// FT-032 / ADR-025: a blocking feedback was `rejected`; the
    /// dispatch parks here permanently (operator intervention required).
    FeedbackRejectedActionBlocked,
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
            Self::PausedForFeedback => DISPATCH_STATUS_PAUSED_FOR_FEEDBACK,
            Self::FeedbackRejectedActionBlocked => DISPATCH_STATUS_FEEDBACK_REJECTED_ACTION_BLOCKED,
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
            DISPATCH_STATUS_PAUSED_FOR_FEEDBACK => Some(Self::PausedForFeedback),
            DISPATCH_STATUS_FEEDBACK_REJECTED_ACTION_BLOCKED => {
                Some(Self::FeedbackRejectedActionBlocked)
            }
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
                | Self::FeedbackRejectedActionBlocked
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
    /// FT-032 / ADR-025: a worker emitted blocking feedback. The
    /// orchestrator pauses the group in `paused-for-feedback`.
    BlockingFeedbackEmitted,
    /// FT-032 / ADR-025: every blocking feedback on the group has
    /// transitioned to `addressed` (or `closed`). The group may resume.
    BlockingFeedbackAddressed,
    /// FT-032 / ADR-025: at least one blocking feedback transitioned to
    /// `rejected`; the group becomes terminal in
    /// `feedback-rejected-action-blocked`.
    BlockingFeedbackRejected,
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
        // FT-032 / ADR-025: pause on blocking feedback emission.
        (S::AwaitingAction, E::BlockingFeedbackEmitted) => S::PausedForFeedback,
        // Re-emission while already paused is a no-op: the new blocking
        // feedback is appended to `dec:blockedBy`, the dispatch stays
        // paused. The transition function returns `PausedForFeedback`
        // so callers can treat this as the same lifecycle row.
        (S::PausedForFeedback, E::BlockingFeedbackEmitted) => S::PausedForFeedback,
        // Resume path: all blocking feedback addressed → re-dispatch
        // the action (back to `awaiting-action` per FT-032 §Outputs).
        (S::PausedForFeedback, E::BlockingFeedbackAddressed) => S::AwaitingAction,
        // Rejection path: terminal failure mode.
        (S::PausedForFeedback, E::BlockingFeedbackRejected) => S::FeedbackRejectedActionBlocked,
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
        let s = next(
            DispatchStatus::AwaitingAction,
            DispatchEvent::ActionCompleted,
        )
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
            DispatchStatus::FeedbackRejectedActionBlocked,
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
                DispatchEvent::BlockingFeedbackEmitted,
                DispatchEvent::BlockingFeedbackAddressed,
                DispatchEvent::BlockingFeedbackRejected,
            ] {
                assert!(
                    next(terminal, event).is_err(),
                    "terminal {terminal:?} should refuse {event:?}"
                );
            }
        }
    }

    #[test]
    fn blocking_feedback_pauses_then_resumes_to_awaiting_action() {
        // FT-032 / TC-036: AwaitingAction + BlockingFeedbackEmitted → PausedForFeedback.
        let s = next(
            DispatchStatus::AwaitingAction,
            DispatchEvent::BlockingFeedbackEmitted,
        )
        .expect("pause");
        assert_eq!(s, DispatchStatus::PausedForFeedback);
        // PausedForFeedback is not terminal — addressed should release it.
        assert!(!s.is_terminal());

        // FT-032 / TC-037: addressed → AwaitingAction (retry).
        let s = next(s, DispatchEvent::BlockingFeedbackAddressed).expect("resume");
        assert_eq!(s, DispatchStatus::AwaitingAction);
    }

    #[test]
    fn paused_dispatch_refuses_to_advance_to_awaiting_interpretation() {
        // FT-032: while paused, no path to `awaiting-interpretation`
        // exists — the verifier must not be dispatched on a paused
        // group. The state machine refuses ActionCompleted in this
        // state.
        let err = next(
            DispatchStatus::PausedForFeedback,
            DispatchEvent::ActionCompleted,
        )
        .expect_err("paused dispatch refuses ActionCompleted");
        assert!(matches!(err, LifecycleError::InvalidTransition { .. }));
    }

    #[test]
    fn blocking_feedback_rejection_lands_on_terminal_blocked() {
        let s = next(
            DispatchStatus::AwaitingAction,
            DispatchEvent::BlockingFeedbackEmitted,
        )
        .expect("pause");
        let s = next(s, DispatchEvent::BlockingFeedbackRejected).expect("reject");
        assert_eq!(s, DispatchStatus::FeedbackRejectedActionBlocked);
        assert!(s.is_terminal());
    }

    #[test]
    fn additional_blocking_emissions_while_paused_are_a_no_op() {
        // FT-032 §Invariants: extending the blocked-by list keeps the
        // dispatch paused — the transition function returns the same
        // PausedForFeedback status.
        let s = next(
            DispatchStatus::AwaitingAction,
            DispatchEvent::BlockingFeedbackEmitted,
        )
        .expect("first pause");
        let s2 =
            next(s, DispatchEvent::BlockingFeedbackEmitted).expect("second emission stays paused");
        assert_eq!(s2, DispatchStatus::PausedForFeedback);
    }

    #[test]
    fn complete_requires_an_approved_path_event() {
        // Only VerdictApproved leads to Complete from awaiting-interpretation.
        for &e in &[DispatchEvent::ActionCompleted, DispatchEvent::ActionFailed] {
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
            DispatchStatus::PausedForFeedback,
            DispatchStatus::FeedbackRejectedActionBlocked,
        ] {
            assert_eq!(DispatchStatus::parse(s.as_str()), Some(s));
        }
        assert!(DispatchStatus::parse("nonsense").is_none());
    }
}
