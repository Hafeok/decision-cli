//! TC-198 — driver terminates with `Done` on the first iteration the
//! planner reports it. `iterations` counts non-terminal dispatches.
//!
//! Validates: FT-110.

use anyhow::Result;
use std::cell::RefCell;
use std::path::Path;

use decision_cli::core::drive::{Action, ArtifactKind, ArtifactRef, Goal, PlanContext};
use decision_cli::core::drive::planner::PlanError;
use decision_cli::drive::{
    run_with_planner_and_executor, Executor, RunArgs,
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
            .unwrap_or(Action::Done))
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
        short_id: "FT-T198".to_string(),
    }
}

fn ctx() -> PlanContext {
    PlanContext::for_test(Path::new("/tmp"))
}

#[test]
fn tc_198_driver_done_short_circuits() {
    let args = RunArgs {
        goal: Goal::Ship,
        artifact: artifact(),
        max_iter: 5,
    };

    // Test A — already-satisfied goal. Planner returns Done on first
    // call. iterations == 0 (no dispatches occurred).
    let planner = PlannerStub::new(vec![Action::Done]);
    let mut executor = ExecutorStub::default();
    let outcome = run_with_planner_and_executor(&ctx(), &args, &planner, &mut executor)
        .expect("Done should succeed");
    assert_eq!(outcome.iterations, 0, "no dispatches before Done");
    assert_eq!(outcome.history.len(), 1, "history has just the Done entry");
    assert_eq!(executor.call_count, 0, "executor never invoked");

    // Test B — Done after one fix. Planner returns DispatchImplementer
    // then Done. iterations == 1, history has two entries.
    let planner = PlannerStub::new(vec![
        Action::DispatchImplementer {
            feature_id: "FT-T198".to_string(),
        },
        Action::Done,
    ]);
    let mut executor = ExecutorStub::default();
    let outcome = run_with_planner_and_executor(&ctx(), &args, &planner, &mut executor)
        .expect("loop should converge");
    assert_eq!(outcome.iterations, 1);
    assert_eq!(outcome.history.len(), 2);
    assert_eq!(executor.call_count, 1);
    // Action tags in order: implement, done.
    assert_eq!(outcome.history[0].action.tag(), "dispatch:implementer");
    assert_eq!(outcome.history[1].action.tag(), "done");
}
