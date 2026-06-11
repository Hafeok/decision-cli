//! TC-310 — FT-131 PAT-002 cycle detection for author-judge oscillation.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightStatus, TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;
use std::cell::RefCell;

struct OscillatingInspector {
    iteration: RefCell<usize>,
}

impl GraphInspector for OscillatingInspector {
    fn aggregate_verdict_for_feature(
        &self,
        _: &str,
    ) -> Result<decision_cli::features::drive::inspect::FeatureVerdict, InspectError> {
        Ok(decision_cli::features::drive::inspect::FeatureVerdict::NeverRun)
    }
    fn open_defect_feedback_count(&self, _: &str, _: &str) -> Result<usize, InspectError> {
        Ok(0)
    }
    fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
        Ok(true)
    }
    fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
        Ok(0)
    }
    fn dependency_statuses_for_feature(
        &self,
        _: &str,
    ) -> Result<Vec<(String, String)>, InspectError> {
        Ok(vec![])
    }
    fn tcs_linked_state_for_feature(&self, _: &str) -> Result<TcsLinkedState, InspectError> {
        // Always return the same unready state, simulating a stuck loop
        Ok(TcsLinkedState::SomeUnready {
            problem_tc: "TC-STUCK".to_string(),
            reason: "always failing".to_string(),
        })
    }
    fn covering_graph_state_for_feature(
        &self,
        _: &str,
        _: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        Ok(CoveringGraphState::AcceptedAll)
    }
    fn preflight_status_for_feature(&self, _: &str) -> Result<PreflightStatus, InspectError> {
        Ok(PreflightStatus::Clean)
    }
    fn tc_quality_verdicts_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(0)
    }
    fn pending_proposals_count(&self, _: &str) -> Result<usize, InspectError> {
        // Oscillate between 0 and 1 to simulate author->judge->author loop
        *self.iteration.borrow_mut() += 1;
        Ok(if *self.iteration.borrow() % 2 == 0 {
            0
        } else {
            1
        })
    }
}

#[test]
fn cycle_detected_on_author_judge_oscillation() {
    let mock = OscillatingInspector {
        iteration: RefCell::new(0),
    };
    let planner = FeatureReadyOrchestratorPlanner::new(&mock, false);

    // Run multiple iterations
    let max_iter = 10;
    let mut stuck_on_cycle = false;

    for i in 0..max_iter {
        let action = planner.classify("FT-131", "ENV-001").unwrap();
        match action {
            Action::Stuck { reason } if reason.contains("cycle:") => {
                // Cycle detected!
                assert!(i < max_iter - 1, "cycle should be detected before max_iter");
                stuck_on_cycle = true;
                break;
            }
            Action::Stuck { reason } if reason.contains("did not change state") => {
                // Pairwise no-progress detected
                assert!(
                    i < max_iter - 1,
                    "no-progress should be detected before max_iter"
                );
                stuck_on_cycle = true;
                break;
            }
            _ => {
                // Continue iterating
            }
        }
    }

    assert!(
        stuck_on_cycle,
        "cycle detection should have triggered before max_iter"
    );
}
