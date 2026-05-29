//! TC-197 — driver respects max-iterations and surfaces a Stuck
//! reason verbatim.
//!
//! Validates: FT-110.

use anyhow::Result;
use std::cell::RefCell;
use std::path::Path;

use decision_cli::core::drive::{Action, ArtifactKind, ArtifactRef, Goal, PlanContext};
use decision_cli::core::drive::planner::PlanError;
use decision_cli::drive::{
    run_with_planner_and_executor, DriveError, Executor, RunArgs,
};
use decision_cli::core::drive::Planner;

struct PlannerStub {
    actions: RefCell<std::collections::VecDeque<Action>>,
}

impl PlannerStub {
    fn new(actions: Vec<Action>) -> Self {
        Self {
            actions: RefCell::new(actions.into()),
        }
    }
}

impl Planner for PlannerStub {
    fn plan(&self, _: &PlanContext, _: &ArtifactRef) -> Result<Action, PlanError> {
        Ok(self
            .actions
            .borrow_mut()
            .pop_front()
            .unwrap_or(Action::Stuck {
                reason: "stub ran out of actions".to_string(),
            }))
    }
}

#[derive(Default)]
struct ExecutorStub {
    call_count: usize,
}

impl Executor for ExecutorStub {
    fn execute(&mut self, _: &PlanContext, _: &Action) -> Result<()> {
        self.call_count += 1;
        Ok(())
    }
}

fn artifact() -> ArtifactRef {
    ArtifactRef {
        kind: ArtifactKind::Feature,
        short_id: "FT-T197".to_string(),
    }
}

fn ctx() -> PlanContext {
    PlanContext::for_test(Path::new("/tmp"))
}

#[test]
fn tc_197_driver_max_iter_and_stuck() {
    // Test A — max-iter cap. Planner returns DispatchVerifier
    // indefinitely; driver should bail at max_iter = 3.
    let infinite_dispatch = vec![
        Action::DispatchVerifier {
            feature_id: "FT-T197".to_string(),
            env_id: "BNCH-002".to_string(),
        };
        10
    ];
    let planner = PlannerStub::new(infinite_dispatch);
    let mut executor = ExecutorStub::default();
    let args = RunArgs {
        goal: Goal::Ship,
        artifact: artifact(),
        max_iter: 3,
    };
    let result = run_with_planner_and_executor(&ctx(), &args, &planner, &mut executor);
    match result {
        Err(DriveError::MaxIterations { max, history }) => {
            assert_eq!(max, 3, "max should pass through unchanged");
            // History has one entry per planner call, capped at max + 1
            // (the final call that observed we'd hit the cap). The
            // executor is called max_iter times.
            assert_eq!(executor.call_count, 3, "executor called exactly max_iter times");
            assert!(
                history.len() >= 3,
                "history should record at least max_iter entries; got {}",
                history.len()
            );
        }
        other => panic!("expected MaxIterations; got {other:?}"),
    }

    // Test B — Stuck propagation. Planner returns Stuck on the first
    // call; driver returns DriveError::Stuck with the reason verbatim.
    let planner = PlannerStub::new(vec![Action::Stuck {
        reason: "synthetic-stuck-reason".to_string(),
    }]);
    let mut executor = ExecutorStub::default();
    let result = run_with_planner_and_executor(&ctx(), &args, &planner, &mut executor);
    match result {
        Err(DriveError::Stuck { reason, history }) => {
            assert_eq!(reason, "synthetic-stuck-reason", "reason preserved verbatim");
            assert_eq!(history.len(), 1, "history has exactly the Stuck entry");
            assert_eq!(executor.call_count, 0, "executor never called on Stuck");
        }
        other => panic!("expected Stuck; got {other:?}"),
    }
}
