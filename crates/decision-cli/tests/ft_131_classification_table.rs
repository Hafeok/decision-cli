//! TC-306 — FT-131 classification table test.
//!
//! Validates the expanded 13-row classification table via a
//! `StubInspector` per row.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightGap, PreflightStatus,
    SpecCompleteness, TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;

/// Stub inspector for table-driven tests.
struct StubInspector {
    deps_done: bool,
    spec_ready: bool,
    adr_acks_ready: bool,
    preflight: PreflightStatus,
    tcs: TcsLinkedState,
    tc_verdicts_count: usize,
    vgs: CoveringGraphState,
    vg_verdicts_count: usize,
    pending_proposals: usize,
}

impl GraphInspector for StubInspector {
    fn aggregate_verdict_for_feature(
        &self,
        _feature_id: &str,
    ) -> Result<decision_cli::features::drive::inspect::FeatureVerdict, InspectError> {
        Ok(decision_cli::features::drive::inspect::FeatureVerdict::NeverRun)
    }

    fn open_defect_feedback_count(
        &self,
        _feature_id: &str,
        _role_id: &str,
    ) -> Result<usize, InspectError> {
        Ok(0)
    }

    fn graphs_exist_for_feature(&self, _feature_id: &str) -> Result<bool, InspectError> {
        Ok(true)
    }

    fn state_hash_for_feature(&self, _feature_id: &str) -> Result<u64, InspectError> {
        Ok(0)
    }

    fn dependency_statuses_for_feature(
        &self,
        _feature_id: &str,
    ) -> Result<Vec<(String, String)>, InspectError> {
        if self.deps_done {
            Ok(vec![])
        } else {
            Ok(vec![("FT-999".to_string(), "in-progress".to_string())])
        }
    }

    fn tcs_linked_state_for_feature(
        &self,
        _feature_id: &str,
    ) -> Result<TcsLinkedState, InspectError> {
        Ok(self.tcs.clone())
    }

    fn covering_graph_state_for_feature(
        &self,
        _feature_id: &str,
        _env_id: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        Ok(self.vgs.clone())
    }

    fn preflight_status_for_feature(
        &self,
        _feature_id: &str,
    ) -> Result<PreflightStatus, InspectError> {
        Ok(self.preflight.clone())
    }

    fn spec_quality_approved(&self, _feature_id: &str) -> Result<bool, InspectError> {
        Ok(self.spec_ready)
    }

    fn adr_acks_quality_approved(&self, _feature_id: &str) -> Result<bool, InspectError> {
        Ok(self.adr_acks_ready)
    }

    fn tc_quality_verdicts_count(&self, _feature_id: &str) -> Result<usize, InspectError> {
        Ok(self.tc_verdicts_count)
    }

    fn vg_quality_verdicts_count(&self, _feature_id: &str) -> Result<usize, InspectError> {
        Ok(self.vg_verdicts_count)
    }

    fn pending_proposals_count(&self, _feature_id: &str) -> Result<usize, InspectError> {
        Ok(self.pending_proposals)
    }
}

#[test]
fn row_1_all_ready_yields_done() {
    let stub = StubInspector {
        deps_done: true,
        spec_ready: true,
        adr_acks_ready: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        tc_verdicts_count: 1,
        vgs: CoveringGraphState::AcceptedAll,
        vg_verdicts_count: 1,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    assert!(matches!(action, Action::Done));
}

#[test]
fn row_2_blocked_by_dep_yields_stuck() {
    let stub = StubInspector {
        deps_done: false,
        spec_ready: true,
        adr_acks_ready: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        tc_verdicts_count: 1,
        vgs: CoveringGraphState::AcceptedAll,
        vg_verdicts_count: 1,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("blocked:"));
            assert!(reason.contains("FT-999"));
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn row_3_spec_not_ready_yields_dispatch_spec_author() {
    let stub = StubInspector {
        deps_done: true,
        spec_ready: false,
        adr_acks_ready: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        tc_verdicts_count: 1,
        vgs: CoveringGraphState::AcceptedAll,
        vg_verdicts_count: 1,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchSpecAuthor { feature_id } => {
            assert_eq!(feature_id, "FT-131");
        }
        _ => panic!("expected DispatchSpecAuthor, got {:?}", action),
    }
}

#[test]
fn row_7_preflight_warnings_yields_stuck() {
    let stub = StubInspector {
        deps_done: true,
        spec_ready: true,
        adr_acks_ready: true,
        preflight: PreflightStatus::Warnings {
            gaps: vec![PreflightGap::UnacknowledgedAdr("ADR-042".to_string())],
        },
        tcs: TcsLinkedState::AllReady,
        tc_verdicts_count: 1,
        vgs: CoveringGraphState::AcceptedAll,
        vg_verdicts_count: 1,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("preflight:"));
            assert!(reason.contains("ADR-042"));
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn row_8_no_tcs_linked_yields_dispatch_tc_author() {
    let stub = StubInspector {
        deps_done: true,
        spec_ready: true,
        adr_acks_ready: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::NoneLinked,
        tc_verdicts_count: 0,
        vgs: CoveringGraphState::AcceptedAll,
        vg_verdicts_count: 1,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchTcAuthor {
            feature_id,
            target_count,
        } => {
            assert_eq!(feature_id, "FT-131");
            assert_eq!(target_count, 4); // ADR-072 default
        }
        _ => panic!("expected DispatchTcAuthor, got {:?}", action),
    }
}

#[test]
fn row_11_vg_missing_yields_dispatch_vga() {
    let stub = StubInspector {
        deps_done: true,
        spec_ready: true,
        adr_acks_ready: true,
        preflight: PreflightStatus::Clean,
        tcs: TcsLinkedState::AllReady,
        tc_verdicts_count: 1,
        vgs: CoveringGraphState::Missing,
        vg_verdicts_count: 0,
        pending_proposals: 0,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchVerifyGraphAuthor { feature_id, env_id } => {
            assert_eq!(feature_id, "FT-131");
            assert_eq!(env_id, "ENV-001");
        }
        _ => panic!("expected DispatchVerifyGraphAuthor, got {:?}", action),
    }
}
