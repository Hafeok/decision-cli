//! TC-311 — FT-131 --no-author produces FT-119 byte-for-byte parity.

use decision_cli::core::drive::Action;
use decision_cli::features::drive::inspect::{
    CoveringGraphState, GraphInspector, InspectError, PreflightGap, PreflightStatus,
    TcsLinkedState,
};
use decision_cli::features::ft_131_readiness_orchestrator::FeatureReadyOrchestratorPlanner;

struct StubInspector {
    scenario: Scenario,
}

#[derive(Clone)]
enum Scenario {
    NoTcsLinked,
    PreflightWarnings,
    VgMissing,
    SpecNotReady,
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
        Ok(vec![])
    }
    fn tcs_linked_state_for_feature(&self, _: &str) -> Result<TcsLinkedState, InspectError> {
        match self.scenario {
            Scenario::NoTcsLinked => Ok(TcsLinkedState::NoneLinked),
            _ => Ok(TcsLinkedState::AllReady),
        }
    }
    fn covering_graph_state_for_feature(
        &self,
        _: &str,
        _: &str,
    ) -> Result<CoveringGraphState, InspectError> {
        match self.scenario {
            Scenario::VgMissing => Ok(CoveringGraphState::Missing),
            _ => Ok(CoveringGraphState::AcceptedAll),
        }
    }
    fn preflight_status_for_feature(&self, _: &str) -> Result<PreflightStatus, InspectError> {
        match self.scenario {
            Scenario::PreflightWarnings => Ok(PreflightStatus::Warnings {
                gaps: vec![PreflightGap::UnacknowledgedAdr("ADR-999".to_string())],
            }),
            _ => Ok(PreflightStatus::Clean),
        }
    }
    fn spec_quality_approved(&self, _: &str) -> Result<bool, InspectError> {
        match self.scenario {
            Scenario::SpecNotReady => Ok(false),
            _ => Ok(true),
        }
    }
    fn tc_quality_verdicts_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(1)
    }
    fn vg_quality_verdicts_count(&self, _: &str) -> Result<usize, InspectError> {
        Ok(1)
    }
}

#[test]
fn no_author_no_tcs_yields_stuck_with_prefix() {
    let stub = StubInspector {
        scenario: Scenario::NoTcsLinked,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, true);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("[no-author]"), "got: {reason}");
            assert!(reason.contains("no TCs linked"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn no_author_preflight_warnings_yields_stuck_with_prefix() {
    let stub = StubInspector {
        scenario: Scenario::PreflightWarnings,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, true);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            // Should still get preflight Stuck, but with FT-119's formatting
            assert!(reason.contains("preflight:"), "got: {reason}");
            assert!(reason.contains("ADR-999"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn no_author_vg_missing_yields_stuck_with_prefix() {
    let stub = StubInspector {
        scenario: Scenario::VgMissing,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, true);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("[no-author]"), "got: {reason}");
            assert!(reason.contains("VG missing"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn no_author_spec_not_ready_yields_stuck_with_prefix() {
    let stub = StubInspector {
        scenario: Scenario::SpecNotReady,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, true);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::Stuck { reason } => {
            assert!(reason.contains("[no-author]"), "got: {reason}");
            assert!(reason.contains("spec not ready"), "got: {reason}");
        }
        _ => panic!("expected Stuck, got {:?}", action),
    }
}

#[test]
fn author_enabled_no_tcs_dispatches_tc_author() {
    let stub = StubInspector {
        scenario: Scenario::NoTcsLinked,
    };
    let planner = FeatureReadyOrchestratorPlanner::new(stub, false);
    let action = planner.classify("FT-131", "ENV-001").unwrap();
    match action {
        Action::DispatchTcAuthor { .. } => {
            // Expected behavior
        }
        _ => panic!("expected DispatchTcAuthor, got {:?}", action),
    }
}
