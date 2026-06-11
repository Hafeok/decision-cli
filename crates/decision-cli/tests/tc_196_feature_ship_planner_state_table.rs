//! TC-196 — `FeatureShipPlanner::classify` returns the right `Action`
//! for every cell in the (verdict × impl-open × vga-open) matrix.
//!
//! Validates: FT-110.

use decision_cli::core::drive::Action;
use decision_cli::drive::inspect::{FeatureVerdict, GraphInspector, InspectError};
use decision_cli::drive::planners::FeatureShipPlanner;

struct StubInspector {
    verdict: FeatureVerdict,
    impl_count: usize,
    vga_count: usize,
}

impl GraphInspector for StubInspector {
    fn aggregate_verdict_for_feature(&self, _: &str) -> Result<FeatureVerdict, InspectError> {
        Ok(self.verdict)
    }
    fn open_defect_feedback_count(&self, _: &str, role_id: &str) -> Result<usize, InspectError> {
        Ok(match role_id {
            "implementer" => self.impl_count,
            "verifier" => self.vga_count,
            _ => 0,
        })
    }
    fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
        Ok(true)
    }
    fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        format!("{:?}", self.verdict).hash(&mut h);
        self.impl_count.hash(&mut h);
        self.vga_count.hash(&mut h);
        Ok(h.finish())
    }
}

fn classify(verdict: FeatureVerdict, impl_c: usize, vga_c: usize) -> Action {
    let planner = FeatureShipPlanner::new(StubInspector {
        verdict,
        impl_count: impl_c,
        vga_count: vga_c,
    });
    planner
        .classify("FT-T196", "BNCH-002")
        .expect("classify ok")
}

#[test]
fn tc_196_feature_ship_planner_state_table() {
    // Row 1: Approved + any counts → Done. Counts don't matter once
    // the goal is reached; the planner doesn't preemptively re-fire.
    for impl_c in [0, 1, 5] {
        for vga_c in [0, 1, 5] {
            assert!(
                matches!(
                    classify(FeatureVerdict::Approved, impl_c, vga_c),
                    Action::Done
                ),
                "Approved/{impl_c}/{vga_c} should be Done"
            );
        }
    }

    // Row 2: implementer-open > 0 dominates (regardless of verdict
    // and vga count).
    for verdict in [
        FeatureVerdict::Rejected,
        FeatureVerdict::AmendmentRequired,
        FeatureVerdict::NeverRun,
    ] {
        for vga_c in [0, 1, 5] {
            let action = classify(verdict, 2, vga_c);
            match action {
                Action::DispatchImplementer { feature_id } => {
                    assert_eq!(feature_id, "FT-T196");
                }
                other => {
                    panic!("{verdict:?} 2 {vga_c} should be DispatchImplementer; got {other:?}")
                }
            }
        }
    }

    // Row 3: implementer == 0 + vga > 0 → verify-graph-author.
    for verdict in [
        FeatureVerdict::Rejected,
        FeatureVerdict::AmendmentRequired,
        FeatureVerdict::NeverRun,
    ] {
        let action = classify(verdict, 0, 3);
        match action {
            Action::DispatchVerifyGraphAuthor { feature_id, env_id } => {
                assert_eq!(feature_id, "FT-T196");
                assert_eq!(env_id, "BNCH-002");
            }
            other => panic!("{verdict:?} 0 3 should be DispatchVerifyGraphAuthor; got {other:?}"),
        }
    }

    // Row 4: NeverRun + clean slate → DispatchVerifier.
    let action = classify(FeatureVerdict::NeverRun, 0, 0);
    match action {
        Action::DispatchVerifier { feature_id, env_id } => {
            assert_eq!(feature_id, "FT-T196");
            assert_eq!(env_id, "BNCH-002");
        }
        other => panic!("expected DispatchVerifier; got {other:?}"),
    }

    // Row 5: bad verdict + clean slate → re-dispatch verifier to refresh
    // evidence. The no-state-change detector will catch terminal Stuck on
    // the next iteration if the verifier run produces no new feedback.
    // This gives the loop a chance to continue automatically instead of
    // bailing on a potentially stale snapshot.
    for verdict in [FeatureVerdict::Rejected, FeatureVerdict::AmendmentRequired] {
        let action = classify(verdict, 0, 0);
        match action {
            Action::DispatchVerifier { feature_id, env_id } => {
                assert_eq!(feature_id, "FT-T196");
                assert_eq!(env_id, "BNCH-002");
            }
            other => panic!("{verdict:?} 0 0 should be DispatchVerifier; got {other:?}"),
        }
    }
}
