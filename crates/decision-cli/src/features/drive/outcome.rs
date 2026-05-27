//! Driver outcome + error types + history projection.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::drive::Action;

/// One entry in the per-invocation history. Captures the action the
/// planner returned plus the iteration number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// 0-based iteration index.
    pub iteration: usize,
    /// Action the planner returned. Tagged by [`Action::tag`] for
    /// quick text rendering.
    pub action: Action,
}

/// Successful terminal outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveOutcome {
    /// Number of `Dispatch*` actions executed (`Done` does not count
    /// as an iteration). For an already-satisfied goal returns 0.
    pub iterations: usize,
    /// Replay of the planner's decisions.
    pub history: Vec<HistoryEntry>,
}

/// Error variants surfaced from the driver loop.
#[derive(Debug, Error)]
pub enum DriveError {
    /// Iteration cap exceeded — planner kept returning dispatch actions
    /// without reaching `Done`.
    #[error("drive: max iterations ({max}) exceeded; history captured")]
    MaxIterations {
        /// The cap that was hit.
        max: usize,
        /// Replay of every action that fired before the cap.
        history: Vec<HistoryEntry>,
    },
    /// Planner explicitly returned `Stuck`.
    #[error("drive: stuck: {reason}")]
    Stuck {
        /// Planner-supplied reason (rendered verbatim).
        reason: String,
        /// Replay including the terminal `Stuck` entry.
        history: Vec<HistoryEntry>,
    },
    /// Planner returned a `PlanError` rather than an `Action::Stuck`.
    #[error("drive: planner error: {detail}")]
    Planner {
        /// Renderable planner error message.
        detail: String,
    },
    /// Action-executor failed (dispatched handler returned an error).
    #[error("drive: action {action_tag} failed at iteration {iteration}: {detail}")]
    Execute {
        /// Iteration index that errored.
        iteration: usize,
        /// Action tag for the rendering.
        action_tag: &'static str,
        /// Renderable error message.
        detail: String,
        /// Replay including the failed action.
        history: Vec<HistoryEntry>,
    },
    /// No planner registered for the requested `(kind, goal)`.
    #[error("drive: no planner registered for ({kind}, {goal})")]
    NoPlannerRegistered {
        /// Artifact-kind tag.
        kind: &'static str,
        /// Goal wire string.
        goal: &'static str,
    },
}
