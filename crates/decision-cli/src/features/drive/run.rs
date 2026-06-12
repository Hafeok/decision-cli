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
use super::progress::{NullProgressSink, ProgressSink, StderrProgressSink};
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

/// Public entry point — uses the production executor and live stderr
/// progress (FT-135). `quiet` suppresses the narration.
pub fn run(ctx: &PlanContext, args: &RunArgs) -> Result<DriveOutcome, DriveError> {
    run_quiet_aware(ctx, args, false)
}

/// FT-135: entry point carrying the `--quiet` resolution.
pub fn run_quiet_aware(
    ctx: &PlanContext,
    args: &RunArgs,
    quiet: bool,
) -> Result<DriveOutcome, DriveError> {
    let mut executor = ProductionExecutor;
    let progress = StderrProgressSink::new(quiet);
    run_with_executor_and_progress(ctx, args, &mut executor, &progress)
}

/// Loop body with an injectable executor; production callers use
/// [`run`], tests substitute a stub so the loop can be exercised
/// without spawning workers.
pub fn run_with_executor(
    ctx: &PlanContext,
    args: &RunArgs,
    executor: &mut dyn Executor,
) -> Result<DriveOutcome, DriveError> {
    run_with_executor_and_progress(ctx, args, executor, &NullProgressSink)
}

/// FT-135: executor + progress sink injection (sweeps mux per-feature
/// progress into one shared sink).
pub fn run_with_executor_and_progress(
    ctx: &PlanContext,
    args: &RunArgs,
    executor: &mut dyn Executor,
    progress: &dyn ProgressSink,
) -> Result<DriveOutcome, DriveError> {
    let planner =
        planner_for(args.artifact.kind, args.goal, ctx).ok_or(DriveError::NoPlannerRegistered {
            kind: args.artifact.kind.as_str(),
            goal: args.goal.as_str(),
        })?;
    run_with_planner_executor_and_progress(ctx, args, planner.as_ref(), executor, progress)
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
    run_with_planner_executor_and_progress(ctx, args, planner, executor, &NullProgressSink)
}

/// FT-135: the instrumented loop body — every iteration narrates plan,
/// exec bracket, and terminal outcome through the sink.
pub fn run_with_planner_executor_and_progress(
    ctx: &PlanContext,
    args: &RunArgs,
    planner: &dyn Planner,
    executor: &mut dyn Executor,
    progress: &dyn ProgressSink,
) -> Result<DriveOutcome, DriveError> {
    let feature = args.artifact.short_id.clone();
    let started = std::time::Instant::now();
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
        progress.on_plan(&feature, i, &action);

        match action {
            Action::Done => {
                progress.on_outcome(
                    &feature,
                    &format!(
                        "Done iter={iterations} elapsed={:.1}s",
                        started.elapsed().as_secs_f64()
                    ),
                );
                return Ok(DriveOutcome {
                    iterations,
                    history,
                });
            }
            Action::Stuck { reason } => {
                progress.on_outcome(&feature, &format!("Stuck reason={reason:?}"));
                return Err(DriveError::Stuck { reason, history });
            }
            other => {
                if iterations >= args.max_iter {
                    progress.on_outcome(&feature, &format!("MaxIter max={}", args.max_iter));
                    return Err(DriveError::MaxIterations {
                        max: args.max_iter,
                        history,
                    });
                }
                let tag = other.tag();
                progress.on_exec_start(&feature, i, tag);
                let exec_started = std::time::Instant::now();
                let result = executor.execute(ctx, &other);
                let elapsed = exec_started.elapsed().as_secs_f64();
                match result {
                    Ok(()) => progress.on_exec_end(&feature, i, tag, elapsed, None),
                    Err(e) => {
                        let detail = format!("{e:#}");
                        progress.on_exec_end(&feature, i, tag, elapsed, Some(&detail));
                        progress.on_outcome(&feature, &format!("Error err={detail:?}"));
                        return Err(DriveError::Execute {
                            iteration: i,
                            action_tag: tag,
                            detail,
                            history,
                        });
                    }
                }
                iterations += 1;
            }
        }
    }
    progress.on_outcome(&feature, &format!("MaxIter max={}", args.max_iter));
    Err(DriveError::MaxIterations {
        max: args.max_iter,
        history,
    })
}
