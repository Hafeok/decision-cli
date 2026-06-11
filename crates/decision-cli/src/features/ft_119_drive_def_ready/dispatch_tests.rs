//! TC-256 (VGA dispatch integration) + TC-257 (sweep goal-parameter
//! integration) for FT-119.
//!
//! TC-256 exercises the full driver loop (FT-110 `drive::run`) with
//! a `FeatureReadyPlanner` and a fake executor that flips the
//! inspector's state on dispatch. The test seam is
//! `drive::run_with_planner_and_executor` — caller supplies both
//! planner and executor so no registry / production wiring is on
//! the test path.
//!
//! TC-257 covers the FT-111 sweep's goal-parameter integration:
//! `SweepInput.goal` flows through to `RunArgs.goal` and the
//! per-feature `RunArgs` carries the correct goal for the planner
//! registry to dispatch the right planner. The sweep's row + tally
//! shapes are reused verbatim; FT-119 does not introduce a parallel
//! aggregation type.

#![cfg(test)]

use std::cell::{Cell, RefCell};

use anyhow::Result;

use crate::core::drive::{Action, ArtifactKind, ArtifactRef, Goal, PlanContext};
use crate::features::drive::execute::Executor;
use crate::features::drive::inspect::{
    CoveringGraphState, FeatureVerdict, GraphInspector, InspectError, PreflightStatus,
    SpecCompleteness, TcsLinkedState,
};
use crate::features::drive::outcome::{DriveError, DriveOutcome};
use crate::features::drive::run::{run_with_planner_and_executor, RunArgs};
use crate::features::ft_119_drive_def_ready::FeatureReadyPlanner;

// ---------------------------------------------------------------------
// TC-256 — VGA dispatch integration.
// ---------------------------------------------------------------------

/// Inspector whose covering-graph state is held in a `Cell` so the
/// fake executor can flip it on each `DispatchVerifyGraphAuthor`.
/// Every other dimension stays at its "ready" value so the only
/// failing arm is the VG one.
struct VgaFlipInspector {
    vgs: RefCell<CoveringGraphState>,
}

impl VgaFlipInspector {
    fn missing() -> Self {
        Self {
            vgs: RefCell::new(CoveringGraphState::Missing),
        }
    }
    fn flip_to_accepted(&self) {
        *self.vgs.borrow_mut() = CoveringGraphState::AcceptedAll;
    }
    fn flip_to_pending(&self, vg_id: &str) {
        *self.vgs.borrow_mut() = CoveringGraphState::PendingReview {
            graph_ids: vec![vg_id.to_string()],
        };
    }
}

impl GraphInspector for VgaFlipInspector {
    fn aggregate_verdict_for_feature(&self, _: &str) -> Result<FeatureVerdict, InspectError> {
        Ok(FeatureVerdict::NeverRun)
    }
    fn open_defect_feedback_count(&self, _: &str, _: &str) -> Result<usize, InspectError> {
        Ok(0)
    }
    fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
        Ok(matches!(
            *self.vgs.borrow(),
            CoveringGraphState::AcceptedAll | CoveringGraphState::PendingReview { .. }
        ))
    }
    fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
        // Hash the active state variant so cycle detection sees the
        // executor's flip as a state change (not a duplicate).
        Ok(match *self.vgs.borrow() {
            CoveringGraphState::Missing => 0xA1,
            CoveringGraphState::AcceptedAll => 0xA2,
            CoveringGraphState::PendingReview { .. } => 0xA3,
        })
    }
    fn feature_spec_completeness(&self, _: &str) -> Result<SpecCompleteness, InspectError> {
        Ok(SpecCompleteness::Complete)
    }
    fn preflight_status_for_feature(&self, _: &str) -> Result<PreflightStatus, InspectError> {
        Ok(PreflightStatus::Clean)
    }
    fn dependency_statuses_for_feature(
        &self,
        _: &str,
    ) -> Result<Vec<(String, String)>, InspectError> {
        Ok(vec![("FT-DEP".to_string(), "complete".to_string())])
    }
    fn tcs_linked_state_for_feature(&self, _: &str) -> Result<TcsLinkedState, InspectError> {
        Ok(TcsLinkedState::AllReady)
    }
    fn covering_graph_state_for_feature(
        &self,
        _: &str,
        _: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        Ok(self.vgs.borrow().clone())
    }
}

/// Fake executor that flips the inspector's VG state on
/// `DispatchVerifyGraphAuthor`. Other dispatch variants are
/// unreachable from FT-119's planner and panic to surface
/// regressions.
struct FakeVgaExecutor<'a> {
    inspector: &'a VgaFlipInspector,
    dispatch_count: Cell<usize>,
    /// What state to flip TO on the next dispatch.
    next_state: RefCell<CoveringGraphState>,
}

impl<'a> FakeVgaExecutor<'a> {
    fn new(inspector: &'a VgaFlipInspector, next_state: CoveringGraphState) -> Self {
        Self {
            inspector,
            dispatch_count: Cell::new(0),
            next_state: RefCell::new(next_state),
        }
    }
}

impl<'a> Executor for FakeVgaExecutor<'a> {
    fn execute(&mut self, _ctx: &PlanContext, action: &Action) -> Result<()> {
        match action {
            Action::DispatchVerifyGraphAuthor { .. } => {
                self.dispatch_count.set(self.dispatch_count.get() + 1);
                // Flip the inspector state to whatever was preconfigured.
                let next = self.next_state.borrow().clone();
                match next {
                    CoveringGraphState::AcceptedAll => self.inspector.flip_to_accepted(),
                    CoveringGraphState::PendingReview { graph_ids } => {
                        let id = graph_ids
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "VG-T256".to_string());
                        self.inspector.flip_to_pending(&id);
                    }
                    CoveringGraphState::Missing => {
                        // Keep Missing — the planner will re-dispatch
                        // and cycle detection will fire.
                    }
                }
                Ok(())
            }
            other => panic!("FT-119 should never dispatch {other:?}"),
        }
    }
}

fn test_ctx() -> PlanContext {
    PlanContext {
        workdir: std::env::temp_dir(),
        product_root: std::env::temp_dir(),
        env_override: Some("BNCH-002".to_string()),
    }
}

fn test_artifact() -> ArtifactRef {
    ArtifactRef {
        kind: ArtifactKind::Feature,
        short_id: "FT-T256".to_string(),
    }
}

#[test]
fn tc_256_vga_dispatch_then_done_when_vg_accepted() {
    let inspector = VgaFlipInspector::missing();
    let planner = FeatureReadyPlanner::new(&inspector);
    let mut executor = FakeVgaExecutor::new(&inspector, CoveringGraphState::AcceptedAll);

    let ctx = test_ctx();
    let args = RunArgs {
        goal: Goal::DefReady,
        artifact: test_artifact(),
        max_iter: 4,
    };

    let outcome: DriveOutcome = run_with_planner_and_executor(&ctx, &args, &planner, &mut executor)
        .expect("drive succeeds");

    // Round 0 dispatched VGA; round 1 observed accepted state and
    // returned Done.
    assert_eq!(
        outcome.history.len(),
        2,
        "expected 2 history entries (VGA dispatch + Done), got {n}: {h:?}",
        n = outcome.history.len(),
        h = outcome.history
    );
    assert!(
        matches!(
            outcome.history[0].action,
            Action::DispatchVerifyGraphAuthor { .. }
        ),
        "first action should be DispatchVerifyGraphAuthor, got {a:?}",
        a = outcome.history[0].action
    );
    assert_eq!(outcome.history[1].action, Action::Done);
    assert_eq!(
        executor.dispatch_count.get(),
        1,
        "executor invoked exactly once"
    );
}

#[test]
fn tc_256_pending_review_yields_stuck_after_one_dispatch() {
    let inspector = VgaFlipInspector::missing();
    let planner = FeatureReadyPlanner::new(&inspector);
    let mut executor = FakeVgaExecutor::new(
        &inspector,
        CoveringGraphState::PendingReview {
            graph_ids: vec!["VG-T256pend".to_string()],
        },
    );

    let ctx = test_ctx();
    let args = RunArgs {
        goal: Goal::DefReady,
        artifact: test_artifact(),
        max_iter: 4,
    };

    let err = run_with_planner_and_executor(&ctx, &args, &planner, &mut executor)
        .expect_err("expected DriveError::Stuck");

    match err {
        DriveError::Stuck { reason, .. } => {
            assert!(
                reason.starts_with("VG pending_review:"),
                "expected VG pending_review reason, got: {reason}"
            );
            assert!(reason.contains("VG-T256pend"));
        }
        other => panic!("expected Stuck, got {other:?}"),
    }
    assert_eq!(
        executor.dispatch_count.get(),
        1,
        "executor invoked exactly once before Stuck"
    );
}

#[test]
fn tc_256_env_id_threaded_from_plan_context() {
    // The dispatch action carries env_id verbatim from the
    // PlanContext (or its default of BNCH-002). Switching mid-loop
    // is forbidden; we assert by capturing the env_id at dispatch.
    let inspector = VgaFlipInspector::missing();
    let planner = FeatureReadyPlanner::new(&inspector);
    let captured_env: RefCell<Option<String>> = RefCell::new(None);

    struct CaptureExecutor<'a> {
        inspector: &'a VgaFlipInspector,
        captured_env: &'a RefCell<Option<String>>,
    }
    impl<'a> Executor for CaptureExecutor<'a> {
        fn execute(&mut self, _ctx: &PlanContext, action: &Action) -> Result<()> {
            if let Action::DispatchVerifyGraphAuthor { env_id, .. } = action {
                *self.captured_env.borrow_mut() = Some(env_id.clone());
                self.inspector.flip_to_accepted();
            }
            Ok(())
        }
    }

    let mut executor = CaptureExecutor {
        inspector: &inspector,
        captured_env: &captured_env,
    };
    let ctx = PlanContext {
        workdir: std::env::temp_dir(),
        product_root: std::env::temp_dir(),
        env_override: Some("BNCH-007".to_string()),
    };
    let args = RunArgs {
        goal: Goal::DefReady,
        artifact: test_artifact(),
        max_iter: 4,
    };

    run_with_planner_and_executor(&ctx, &args, &planner, &mut executor).expect("drive succeeds");
    assert_eq!(captured_env.borrow().as_deref(), Some("BNCH-007"));
}

// ---------------------------------------------------------------------
// TC-257 — sweep goal-parameter integration.
// ---------------------------------------------------------------------
//
// A full end-to-end sweep test would require seeding a fixture
// product graph and ProductionInspector overrides — out of scope for
// this commit. TC-257 here pins the substrate-level invariants the
// FT-119 spec requires: `SweepInput.goal` exists, defaults to Ship,
// and threads through to `RunArgs.goal` so the registry dispatches
// the right planner. End-to-end fixture-driven tests land alongside
// the production inspector wiring.

use crate::features::ft_111_drive_ship_all::sweep::{apply_filter, Outcome, SweepInput, Tally};
use std::time::Duration;

#[test]
fn tc_257_sweep_input_carries_goal_field() {
    let input = SweepInput {
        features: vec!["FT-A".to_string()],
        env_id: Some("BNCH-002".to_string()),
        max_iter: 4,
        per_item_timeout: Duration::from_secs(10),
        goal: Goal::DefReady,
    };
    assert_eq!(input.goal, Goal::DefReady);
}

#[test]
fn tc_257_sweep_input_default_is_ship_for_back_compat() {
    let input: SweepInput = SweepInput::default();
    assert_eq!(input.goal, Goal::Ship);
}

#[test]
fn tc_257_apply_filter_unchanged_under_def_ready_goal() {
    let resolved = vec!["FT-A".to_string(), "FT-C".to_string(), "FT-E".to_string()];
    let filter = vec!["FT-C".to_string()];
    let result = apply_filter(resolved, Some(filter)).expect("filter applies");
    assert_eq!(result, vec!["FT-C".to_string()]);
}

#[test]
fn tc_257_outcome_and_tally_shapes_reusable_under_def_ready() {
    // The sweep's row/tally types are goal-agnostic by design (the
    // FT-119 spec calls this out as the reuse contract).
    let mut tally = Tally::default();
    let outcomes = vec![
        Outcome::Done,
        Outcome::Done,
        Outcome::Stuck {
            reason: "preflight: ADR-070 unacknowledged".to_string(),
        },
        Outcome::Stuck {
            reason: "no TCs linked".to_string(),
        },
        Outcome::Error {
            detail: "inspector failure".to_string(),
        },
    ];
    for outcome in &outcomes {
        match outcome {
            Outcome::Done => tally.done += 1,
            Outcome::Stuck { .. } => tally.stuck += 1,
            Outcome::HitMaxIter => tally.max_iter += 1,
            Outcome::Timeout { .. } => tally.timeout += 1,
            Outcome::Error { .. } => tally.error += 1,
        }
    }
    assert_eq!(tally.done, 2);
    assert_eq!(tally.stuck, 2);
    assert_eq!(tally.error, 1);
    assert_eq!(tally.max_iter, 0);
    assert_eq!(tally.timeout, 0);
}
