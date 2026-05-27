//! The 20-line driver loop (FT-110 §Behaviour).
//!
//! Inputs: parsed `ArtifactRef`, `Goal`, `PlanContext`, an optional
//! `max_iter`. The driver looks up the planner, then loops:
//!   * Call `planner.plan(...)` → `Action`.
//!   * Append to history.
//!   * If terminal (`Done` / `Stuck`) → return.
//!   * Else execute the action via the supplied `Executor` and
//!     continue.
//! Bails with `DriveError::MaxIterations` when the cap is hit.

use crate::core::drive::{Action, ArtifactRef, Goal, PlanContext, Planner};

use super::execute::{Executor, ProductionExecutor};
use super::outcome::{DriveError, DriveOutcome, HistoryEntry};
use super::registry::planner_for;

/// CLI-shaped arguments for the driver.
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// Goal to drive toward.
    pub goal: Goal,
    /// Parsed artifact reference.
    pub artifact: ArtifactRef,
    /// Bail-out limit on planner iterations. Default 5.
    pub max_iter: usize,
}

/// Default `max_iter` when the operator doesn't specify one.
pub const DEFAULT_MAX_ITER: usize = 5;

/// Public entry point — uses the production executor.
pub fn run(ctx: &PlanContext, args: &RunArgs) -> Result<DriveOutcome, DriveError> {
    let mut executor = ProductionExecutor;
    run_with_executor(ctx, args, &mut executor)
}

/// Loop body with an injectable executor; production callers use
/// [`run`], tests substitute a stub so the loop can be exercised
/// without spawning workers.
pub fn run_with_executor(
    ctx: &PlanContext,
    args: &RunArgs,
    executor: &mut dyn Executor,
) -> Result<DriveOutcome, DriveError> {
    let planner = planner_for(args.artifact.kind, args.goal, ctx).ok_or(
        DriveError::NoPlannerRegistered {
            kind: args.artifact.kind.as_str(),
            goal: args.goal.as_str(),
        },
    )?;
    run_with_planner_and_executor(ctx, args, planner.as_ref(), executor)
}

/// Test seam: caller supplies both planner and executor directly.
/// Used by TC-197 / TC-198 so the driver loop can be exercised
/// independently of the registry.
pub fn run_with_planner_and_executor(
    ctx: &PlanContext,
    args: &RunArgs,
    planner: &dyn Planner,
    executor: &mut dyn Executor,
) -> Result<DriveOutcome, DriveError> {
    let mut history: Vec<HistoryEntry> = Vec::new();
    let mut iterations: usize = 0;

    for i in 0..args.max_iter.saturating_add(1) {
        let action = planner
            .plan(ctx, &args.artifact)
            .map_err(|e| DriveError::Planner {
                detail: format!("{e}"),
            })?;
        history.push(HistoryEntry {
            iteration: i,
            action: action.clone(),
        });

        match action {
            Action::Done => {
                return Ok(DriveOutcome {
                    iterations,
                    history,
                });
            }
            Action::Stuck { reason } => {
                return Err(DriveError::Stuck { reason, history });
            }
            other => {
                if iterations >= args.max_iter {
                    return Err(DriveError::MaxIterations {
                        max: args.max_iter,
                        history,
                    });
                }
                if let Err(e) = executor.execute(ctx, &other) {
                    return Err(DriveError::Execute {
                        iteration: i,
                        action_tag: other.tag(),
                        detail: format!("{e:#}"),
                        history,
                    });
                }
                iterations += 1;
            }
        }
    }
    Err(DriveError::MaxIterations {
        max: args.max_iter,
        history,
    })
}
