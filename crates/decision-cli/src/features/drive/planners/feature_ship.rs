//! `FeatureShipPlanner` — the FT+Ship planner.
//!
//! Classifier table:
//!
//! | aggregate verdict | implementer-open | verifier-open | Action |
//! |---|---|---|---|
//! | `Approved` | any | any | `Done` |
//! | any | `> 0` | any | `DispatchImplementer` |
//! | any | `0` | `> 0` | `DispatchVerifyGraphAuthor` |
//! | `NeverRun` | `0` | `0` | `DispatchVerifier` |
//! | `Rejected` / `Amendment` | `0` | `0` | `Stuck` |

use crate::core::drive::{Action, ArtifactKind, ArtifactRef, PlanContext, Planner};
use crate::core::drive::planner::PlanError;

use super::super::inspect::{FeatureVerdict, GraphInspector};

/// Planner for `dec drive ship FT-XXX`.
///
/// Composes [`GraphInspector`] reads via a trait so unit tests can
/// supply a stub inspector; the production driver wires
/// [`super::super::inspect::ProductionInspector`].
pub struct FeatureShipPlanner<I: GraphInspector> {
    inspector: I,
}

impl<I: GraphInspector> FeatureShipPlanner<I> {
    /// Construct with an explicit inspector. Production callers use
    /// `ProductionInspector::new(ctx)`; tests pass a stub.
    pub fn new(inspector: I) -> Self {
        Self { inspector }
    }

    /// Core classification — separated from the `Planner` trait impl
    /// so tests can call it without a `PlanContext`.
    pub fn classify(
        &self,
        feature_id: &str,
        default_env_id: &str,
    ) -> Result<Action, PlanError> {
        let verdict = self
            .inspector
            .aggregate_verdict_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        let impl_open = self
            .inspector
            .open_defect_feedback_count(feature_id, "implementer")
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        let vga_open = self
            .inspector
            .open_defect_feedback_count(feature_id, "verifier")
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;

        Ok(match (verdict, impl_open > 0, vga_open > 0) {
            (FeatureVerdict::Approved, _, _) => Action::Done,
            (_, true, _) => Action::DispatchImplementer {
                feature_id: feature_id.to_string(),
            },
            (_, _, true) => Action::DispatchVerifyGraphAuthor {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (FeatureVerdict::NeverRun, false, false) => Action::DispatchVerifier {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (FeatureVerdict::Rejected | FeatureVerdict::AmendmentRequired, false, false) => {
                Action::Stuck {
                    reason: format!(
                        "feature {feature_id}: verify still failing but all defect feedback \
                         has been addressed; the worker is not converging — inspect \
                         `dec loop show {feature_id}` for the chain",
                    ),
                }
            }
        })
    }
}

impl<I: GraphInspector> Planner for FeatureShipPlanner<I> {
    fn plan(&self, ctx: &PlanContext, artifact: &ArtifactRef) -> Result<Action, PlanError> {
        if artifact.kind != ArtifactKind::Feature {
            return Err(PlanError::Internal {
                detail: format!(
                    "FeatureShipPlanner asked to plan for {:?}; expected Feature",
                    artifact.kind
                ),
            });
        }
        let env_id = ctx.env_or_default("ENV-002");
        self.classify(&artifact.short_id, &env_id)
    }
}

// ---------------------------------------------------------------------
// Tests — exercise the classification table without I/O.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::inspect::InspectError;

    /// Stub inspector returning fixed (verdict, impl_count, vga_count).
    struct StubInspector {
        verdict: FeatureVerdict,
        impl_count: usize,
        vga_count: usize,
    }

    impl GraphInspector for StubInspector {
        fn aggregate_verdict_for_feature(
            &self,
            _: &str,
        ) -> Result<FeatureVerdict, InspectError> {
            Ok(self.verdict)
        }
        fn open_defect_feedback_count(
            &self,
            _: &str,
            role_id: &str,
        ) -> Result<usize, InspectError> {
            Ok(match role_id {
                "implementer" => self.impl_count,
                "verifier" => self.vga_count,
                _ => 0,
            })
        }
    }

    fn run_case(verdict: FeatureVerdict, impl_count: usize, vga_count: usize) -> Action {
        let planner = FeatureShipPlanner::new(StubInspector {
            verdict,
            impl_count,
            vga_count,
        });
        planner
            .classify("FT-TEST", "ENV-002")
            .expect("classification succeeds")
    }

    #[test]
    fn approved_returns_done() {
        for impl_c in [0, 1, 5] {
            for vga_c in [0, 1, 5] {
                let action = run_case(FeatureVerdict::Approved, impl_c, vga_c);
                assert!(matches!(action, Action::Done), "{impl_c} {vga_c}");
            }
        }
    }

    #[test]
    fn implementer_open_dispatches_implementer() {
        for verdict in [
            FeatureVerdict::Rejected,
            FeatureVerdict::AmendmentRequired,
            FeatureVerdict::NeverRun,
        ] {
            let action = run_case(verdict, 2, 0);
            assert!(matches!(action, Action::DispatchImplementer { .. }), "{verdict:?}");
        }
    }

    #[test]
    fn implementer_open_wins_over_verifier_open() {
        let action = run_case(FeatureVerdict::Rejected, 1, 1);
        assert!(matches!(action, Action::DispatchImplementer { .. }));
    }

    #[test]
    fn verifier_open_dispatches_vga() {
        for verdict in [
            FeatureVerdict::Rejected,
            FeatureVerdict::AmendmentRequired,
            FeatureVerdict::NeverRun,
        ] {
            let action = run_case(verdict, 0, 2);
            assert!(
                matches!(action, Action::DispatchVerifyGraphAuthor { .. }),
                "{verdict:?}"
            );
        }
    }

    #[test]
    fn never_run_with_clean_slate_dispatches_verifier() {
        let action = run_case(FeatureVerdict::NeverRun, 0, 0);
        match action {
            Action::DispatchVerifier { feature_id, env_id } => {
                assert_eq!(feature_id, "FT-TEST");
                assert_eq!(env_id, "ENV-002");
            }
            other => panic!("expected DispatchVerifier, got {other:?}"),
        }
    }

    #[test]
    fn rejected_with_no_open_feedback_is_stuck() {
        let action = run_case(FeatureVerdict::Rejected, 0, 0);
        match action {
            Action::Stuck { reason } => {
                assert!(reason.contains("worker is not converging"), "reason: {reason}");
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    #[test]
    fn amendment_required_with_no_open_feedback_is_stuck() {
        let action = run_case(FeatureVerdict::AmendmentRequired, 0, 0);
        assert!(matches!(action, Action::Stuck { .. }));
    }

    #[test]
    fn wrong_artifact_kind_errors() {
        let inspector = StubInspector {
            verdict: FeatureVerdict::Approved,
            impl_count: 0,
            vga_count: 0,
        };
        let planner = FeatureShipPlanner::new(inspector);
        let ctx = PlanContext::for_test(std::path::Path::new("/tmp"));
        let artifact = ArtifactRef {
            kind: ArtifactKind::TestCriterion,
            short_id: "TC-001".to_string(),
        };
        let err = planner.plan(&ctx, &artifact).unwrap_err();
        assert!(format!("{err}").contains("expected Feature"));
    }
}
