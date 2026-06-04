//! TC-307 — FT-131 Stuck reasons cite offending artifact ids verbatim.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightGap, PreflightStatus,
    TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;

struct StubInspector {
    deps_done: bool,
    preflight: PreflightStatus,
    tcs: TcsLinkedState,
    vgs: CoveringGraphState,
}

impl GraphInspector for StubInspector {
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
        if self.deps_done {
            Ok(vec![])
        } else {
            Ok(vec![("FT-042".to_string(), "in-progress".to_string())])
        }
    }
    fn tcs_linked_state_for_feature(&self, _: &str) -> Result<TcsLinkedState, InspectError> {
        Ok(self.tcs.clone())
    }
    fn covering_graph_state_for_feature(
        &self,
        _: &str,
        _: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        Ok(self.vgs.clone())
    }
    fn preflight_status_for_feature(&self, _: &str) -> Result<PreflightStatus, InspectError> {
        Ok(self.preflight.clone())
    }
}

#[test]
fn stuck_blocked_cites_dep_id_and_status() {
    let stub = StubInspector {
        deps_done: false,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        vgs: CoveringGraphState::AcceptedAll,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("blocked: FT-042"), "got: {reason}");
            assert!(reason.contains("status=in-progress"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn stuck_preflight_cites_adr_id() {
    let stub = StubInspector {
        deps_done: true,
        preflight: PreflightStatus::Warnings {
            gaps: vec![PreflightGap::UnacknowledgedAdr("ADR-123".to_string())],
        },
        tcs: TcsLinkedState::AllReady,
        vgs: CoveringGraphState::AcceptedAll,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("preflight:"), "got: {reason}");
            assert!(reason.contains("ADR-123"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn stuck_tc_quality_cites_tc_id() {
    let stub = StubInspector {
        deps_done: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::SomeUnready {
            problem_tc: "TC-456".to_string(),
            reason: "runner missing".to_string(),
        },
        vgs: CoveringGraphState::AcceptedAll,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("TC-456"), "got: {reason}");
            assert!(reason.contains("runner missing"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn stuck_vg_pending_cites_vg_id() {
    let stub = StubInspector {
        deps_done: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        vgs: CoveringGraphState::PendingReview {
            graph_ids: vec!["VG-789".to_string()],
        },
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("VG-789"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}
