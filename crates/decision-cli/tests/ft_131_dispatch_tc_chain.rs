//! TC-308 — FT-131 dispatches tc-author then tc-quality.
//!
//! Simulates the full authoring chain for TCs.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightStatus, TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;
use std::cell::RefCell;

struct MockInspector {
    tcs_state: RefCell<TcsLinkedState>,
    tc_verdicts_count: RefCell<usize>,
    pending_proposals: RefCell<usize>,
}

impl GraphInspector for MockInspector {
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
        Ok(self.tcs_state.borrow().clone())
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
        Ok(*self.tc_verdicts_count.borrow())
    }
    fn pending_proposals_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(*self.pending_proposals.borrow())
    }
}

#[test]
fn tc_chain_dispatches_author_then_quality() {
    let mock = MockInspector {
        tcs_state: RefCell::new(TcsLinkedState::NoneLinked),
        tc_verdicts_count: RefCell::new(0),
        pending_proposals: RefCell::new(0),
    };
    let planner = FeatureReadyOrchestratorPlanner::new(&mock, false);

    // Step 1: no TCs → dispatch author
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchTcAuthor { feature_id, .. } => {
            assert_eq!(feature_id, "FT-131");
        }
        _ => panic!("expected DispatchTcAuthor, got {:?}", action),
    }

    // Simulate: tc-author completed, proposal pending
    *mock.tcs_state.borrow_mut() = TcsLinkedState::SomeUnready {
        problem_tc: "TC-NEW".to_string(),
        reason: "pending review".to_string(),
    };
    *mock.pending_proposals.borrow_mut() = 1;

    // Step 2: pending proposal → dispatch quality judge
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchTcQuality {
            feature_id,
            tc_proposal_iri,
        } => {
            assert_eq!(feature_id, "FT-131");
            assert!(tc_proposal_iri.contains("pending-tc-proposal"));
        }
        _ => panic!("expected DispatchTcQuality, got {:?}", action),
    }

    // Simulate: quality approved, verdict recorded
    *mock.tcs_state.borrow_mut() = TcsLinkedState::AllReady;
    *mock.tc_verdicts_count.borrow_mut() = 1;
    *mock.pending_proposals.borrow_mut() = 0;

    // Step 3: TCs ready → move to next dimension
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    // Should no longer be stuck on TCs
    match action {
        Action::Stuck { reason } if reason.contains("TC") => {
            panic!("still stuck on TCs: {reason}");
        }
        Action::Done | Action::DispatchVerifyGraphAuthor { .. } => {
            // Expected: either done or dispatching VGA next
        }
        _ => {} // other actions are fine too
    }
}
