//! TC-309 — FT-131 dispatches verify-graph-author then vg-quality.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightStatus, TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;
use std::cell::RefCell;

struct MockInspector {
    vgs_state: RefCell<CoveringGraphState>,
    vg_verdicts_count: RefCell<usize>,
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
        Ok(TcsLinkedState::AllReady)
    }
    fn covering_graph_state_for_feature(
        &self,
        _: &str,
        _: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        Ok(self.vgs_state.borrow().clone())
    }
    fn preflight_status_for_feature(&self, _: &str) -> Result<PreflightStatus, InspectError> {
        Ok(PreflightStatus::Clean)
    }
    fn tc_quality_verdicts_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(1)
    }
    fn vg_quality_verdicts_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(*self.vg_verdicts_count.borrow())
    }
    fn pending_proposals_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(*self.pending_proposals.borrow())
    }
}

#[test]
fn vg_chain_dispatches_author_then_quality() {
    let mock = MockInspector {
        vgs_state: RefCell::new(CoveringGraphState::Missing),
        vg_verdicts_count: RefCell::new(0),
        pending_proposals: RefCell::new(0),
    };
    let planner = FeatureReadyOrchestratorPlanner::new(&mock, false);

    // Step 1: no VG → dispatch verify-graph-author
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchVerifyGraphAuthor { feature_id, env_id } => {
            assert_eq!(feature_id, "FT-131");
            assert_eq!(env_id, "ENV-001");
        }
        _ => panic!("expected DispatchVerifyGraphAuthor, got {:?}", action),
    }

    // Simulate: vg-author completed, proposal pending review
    *mock.vgs_state.borrow_mut() = CoveringGraphState::PendingReview {
        graph_ids: vec!["VG-NEW".to_string()],
    };
    *mock.pending_proposals.borrow_mut() = 1;

    // Step 2: pending proposal → dispatch vg-quality judge
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchVgQuality {
            feature_id,
            graph_proposal_iri,
        } => {
            assert_eq!(feature_id, "FT-131");
            assert!(graph_proposal_iri.contains("pending-vg-proposal"));
        }
        _ => panic!("expected DispatchVgQuality, got {:?}", action),
    }

    // Simulate: quality approved, verdict recorded
    *mock.vgs_state.borrow_mut() = CoveringGraphState::AcceptedAll;
    *mock.vg_verdicts_count.borrow_mut() = 1;
    *mock.pending_proposals.borrow_mut() = 0;

    // Step 3: VGs ready → Done
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    assert!(matches!(action, Action::Done));
}
