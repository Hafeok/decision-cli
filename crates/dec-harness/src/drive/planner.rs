//! Planner trait + error type.
//!
//! A `Planner` is a pure function from `(PlanContext, ArtifactRef)` to
//! [`Action`]. Planners do not call dispatch handlers themselves — they
//! only describe what should happen next. The driver loop owns
//! execution.

use thiserror::Error;

use super::{Action, ArtifactRef, PlanContext};

/// Trait every concrete planner implements. Slice-1 keeps planners as
/// Rust types in `features::drive::planners`; future work may swap
/// this for a graph-resident `dec:DispatchRule` lookup, but the
/// `plan(ctx, artifact) -> Action` shape stays the same.
pub trait Planner {
    /// Inspect the orchestration store via `ctx` and return the next
    /// action toward the planner's implicit goal. Errors propagate
    /// store-read failures; a `Stuck` outcome should be returned as
    /// `Ok(Action::Stuck { .. })`, not `Err`.
    fn plan(&self, ctx: &PlanContext, artifact: &ArtifactRef) -> Result<Action, PlanError>;
}

/// Planner-side error variants. Distinct from `DriveError` so the
/// driver can decide whether to retry, swallow, or propagate.
#[derive(Debug, Error)]
pub enum PlanError {
    /// Underlying store read failed.
    #[error("planner: store read failure: {detail}")]
    Store {
        /// Human-readable context.
        detail: String,
    },
    /// The artifact short id doesn't resolve to a known object.
    #[error("planner: artifact not found: {kind} {short_id}")]
    ArtifactNotFound {
        /// Artifact kind tag.
        kind: &'static str,
        /// Short id the caller supplied.
        short_id: String,
    },
    /// Catch-all internal failure.
    #[error("planner: internal error: {detail}")]
    Internal {
        /// Human-readable context.
        detail: String,
    },
}
