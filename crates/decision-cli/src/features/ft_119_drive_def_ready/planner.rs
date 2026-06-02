//! `FeatureReadyPlanner` — the FT-119 Definition-of-Ready planner.
//!
//! Classifier table (first match wins, precedence enforced by the
//! order of guarded returns in `classify`):
//!
//! | check                                  | resulting `Action`                    |
//! |----------------------------------------|---------------------------------------|
//! | preflight = `Warnings { gaps }`        | `Stuck "preflight: ADR-N..."`         |
//! | any dep status ≠ `complete`            | `Stuck "blocked: FT-Y status=..."`    |
//! | `feature_spec_complete = false`        | `Stuck "spec incomplete: <feature>"`  |
//! | tcs = `NoneLinked`                     | `Stuck "no TCs linked"`               |
//! | tcs = `SomeUnready { problem_tc, ... }`| `Stuck "TC quality: TC-NNN ..."`      |
//! | vgs = `Missing`                        | `DispatchVerifyGraphAuthor`           |
//! | vgs = `PendingReview { graph_ids }`    | `Stuck "VG pending_review: VG-..."`   |
//! | otherwise                              | `Done`                                |
//!
//! The planner never writes — only reads via [`GraphInspector`] — so
//! it is a pure function of inspector observations (PAT-001). Cycle
//! detection (PAT-002) and the multi-feature sweep (PAT-003) land
//! in follow-up commits; this commit ships the classification core
//! and the TC-254 backstop that validates every row against a stub.

use crate::core::drive::planner::PlanError;
use crate::core::drive::{Action, ArtifactKind, ArtifactRef, PlanContext, Planner};

use crate::features::drive::inspect::{
    CoveringGraphState, GraphInspector, PreflightStatus, TcsLinkedState,
};

/// Planner for `dec drive def-ready FT-XXX`.
///
/// Composes [`GraphInspector`] reads via a trait so unit tests can
/// supply a stub; production wiring (the
/// `ProductionInspector` overrides for the new dimensions) lands in
/// a follow-up commit.
pub struct FeatureReadyPlanner<I: GraphInspector> {
    inspector: I,
}

impl<I: GraphInspector> FeatureReadyPlanner<I> {
    /// Wire the planner against an inspector. Cheap — holds the
    /// inspector by value.
    pub fn new(inspector: I) -> Self {
        Self { inspector }
    }

    /// Pure classification — separated from the `Planner` trait impl
    /// so tests can call it without constructing a `PlanContext`.
    /// First match wins per the table at module top.
    pub fn classify(
        &self,
        feature_id: &str,
        default_env_id: &str,
    ) -> Result<Action, PlanError> {
        // 1. Preflight gates everything — an unacknowledged
        //    cross-cutting ADR or domain gap means the feature's
        //    context bundle is incomplete, which is the highest-
        //    priority Stuck reason.
        let preflight = self
            .inspector
            .preflight_status_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        if let PreflightStatus::Warnings { gaps } = preflight {
            return Ok(Action::Stuck {
                reason: stuck_preflight(&gaps),
            });
        }

        // 2. Dependencies — a feature with an in-progress depends-on
        //    is structurally blocked.
        let deps = self
            .inspector
            .dependency_statuses_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        if let Some((dep_id, status)) = first_unfinished_dep(&deps) {
            return Ok(Action::Stuck {
                reason: stuck_blocked(dep_id, status),
            });
        }

        // 3. Spec body completeness — FT-055/ADR-047 H2/H3 check.
        let spec_complete = self
            .inspector
            .feature_spec_complete(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        if !spec_complete {
            return Ok(Action::Stuck {
                reason: stuck_spec_incomplete(feature_id),
            });
        }

        // 4. TCs — linked + body + runner state.
        let tcs = self
            .inspector
            .tcs_linked_state_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        match tcs {
            TcsLinkedState::NoneLinked => {
                return Ok(Action::Stuck {
                    reason: "no TCs linked".to_string(),
                });
            }
            TcsLinkedState::SomeUnready { problem_tc, reason } => {
                return Ok(Action::Stuck {
                    reason: stuck_tc_quality(&problem_tc, &reason),
                });
            }
            TcsLinkedState::AllReady => {}
        }

        // 5. Covering graph state — the only worker-resolvable arm.
        let vgs = self
            .inspector
            .covering_graph_state_for_feature(feature_id, default_env_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        match vgs {
            CoveringGraphState::Missing => Ok(Action::DispatchVerifyGraphAuthor {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            }),
            CoveringGraphState::PendingReview { graph_ids } => Ok(Action::Stuck {
                reason: stuck_vg_pending(&graph_ids),
            }),
            CoveringGraphState::AcceptedAll => Ok(Action::Done),
        }
    }
}

/// First `(feature_id, status)` pair whose status is not the literal
/// `"complete"`. Iteration is stable: the inspector returns the
/// dependency list in the order it appears in the feature's
/// `depends-on:` frontmatter, so the Stuck reason names the first
/// unfinished dep in document order.
fn first_unfinished_dep(deps: &[(String, String)]) -> Option<(&str, &str)> {
    deps.iter()
        .find(|(_, status)| status != "complete")
        .map(|(id, status)| (id.as_str(), status.as_str()))
}

/// Build the `preflight: ...` Stuck reason. Gaps are joined with a
/// comma so a single string can carry the full unacknowledged set.
fn stuck_preflight(gaps: &[String]) -> String {
    let joined = if gaps.is_empty() {
        "(unspecified)".to_string()
    } else {
        gaps.join(", ")
    };
    format!("preflight: {joined}")
}

/// Build the `blocked: FT-Y status=...` Stuck reason — cites the
/// dep id and its current status verbatim so the operator can open
/// the right artifact without re-running anything (TC-255 contract).
fn stuck_blocked(dep_id: &str, status: &str) -> String {
    format!("blocked: {dep_id} status={status}")
}

/// `spec incomplete` Stuck reason — names the feature so a sweep
/// row stays self-identifying when the driver echoes the reason
/// out of context.
fn stuck_spec_incomplete(feature_id: &str) -> String {
    format!("spec incomplete: {feature_id} body missing required H2/H3 sections")
}

/// `TC quality: TC-NNN ...` Stuck reason — TC-255 contract requires
/// the TC id appear verbatim so operators can jump straight to the
/// offending artifact.
fn stuck_tc_quality(tc_id: &str, reason: &str) -> String {
    format!("TC quality: {tc_id} {reason}")
}

/// `VG pending_review: VG-...` Stuck reason. The pending VG ids are
/// joined comma-separated so the planner can carry multiple in one
/// reason string when several graphs land for review on the same
/// feature.
fn stuck_vg_pending(graph_ids: &[String]) -> String {
    let joined = if graph_ids.is_empty() {
        "(unspecified)".to_string()
    } else {
        graph_ids.join(", ")
    };
    format!("VG pending_review: {joined}")
}

impl<I: GraphInspector> Planner for FeatureReadyPlanner<I> {
    fn plan(&self, ctx: &PlanContext, artifact: &ArtifactRef) -> Result<Action, PlanError> {
        if artifact.kind != ArtifactKind::Feature {
            return Err(PlanError::Internal {
                detail: format!(
                    "FeatureReadyPlanner asked to plan for {:?}; expected Feature",
                    artifact.kind
                ),
            });
        }
        let env_id = ctx.env_or_default("BNCH-002");
        self.classify(&artifact.short_id, &env_id)
    }
}

// ---------------------------------------------------------------------
// TC-254 — pure-classification backstop.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::drive::inspect::InspectError;

    /// Cell-driven stub: every dimension is set by hand for each
    /// row of the classification table. The TC body in
    /// `.product/tests/TC-254-*.md` enumerates the expected
    /// classification per cell; this stub lets the test exercise
    /// each row without seeding a real store.
    struct StubInspector {
        spec_complete: bool,
        preflight: PreflightStatus,
        deps: Vec<(String, String)>,
        tcs: TcsLinkedState,
        vgs: CoveringGraphState,
    }

    impl StubInspector {
        /// "All ready" baseline — every dimension set to its passing
        /// value. Per-test code mutates individual fields to encode
        /// the row under test.
        fn all_ready() -> Self {
            Self {
                spec_complete: true,
                preflight: PreflightStatus::Clean,
                deps: vec![("FT-DEP".to_string(), "complete".to_string())],
                tcs: TcsLinkedState::AllReady,
                vgs: CoveringGraphState::AcceptedAll,
            }
        }
    }

    impl GraphInspector for StubInspector {
        fn aggregate_verdict_for_feature(
            &self,
            _: &str,
        ) -> Result<crate::features::drive::inspect::FeatureVerdict, InspectError> {
            // FT-119 planner does not consult the verifier-pair
            // verdict — unused.
            Ok(crate::features::drive::inspect::FeatureVerdict::NeverRun)
        }
        fn open_defect_feedback_count(
            &self,
            _: &str,
            _: &str,
        ) -> Result<usize, InspectError> {
            Ok(0)
        }
        fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
            Ok(matches!(
                self.vgs,
                CoveringGraphState::AcceptedAll | CoveringGraphState::PendingReview { .. }
            ))
        }
        fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
            Ok(0)
        }
        fn feature_spec_complete(&self, _: &str) -> Result<bool, InspectError> {
            Ok(self.spec_complete)
        }
        fn preflight_status_for_feature(
            &self,
            _: &str,
        ) -> Result<PreflightStatus, InspectError> {
            Ok(self.preflight.clone())
        }
        fn dependency_statuses_for_feature(
            &self,
            _: &str,
        ) -> Result<Vec<(String, String)>, InspectError> {
            Ok(self.deps.clone())
        }
        fn tcs_linked_state_for_feature(
            &self,
            _: &str,
        ) -> Result<TcsLinkedState, InspectError> {
            Ok(self.tcs.clone())
        }
        fn covering_graph_state_for_feature(
            &self,
            _: &str,
            _: &str,
        ) -> Result<CoveringGraphState, InspectError> {
            Ok(self.vgs.clone())
        }
    }

    fn run(stub: StubInspector) -> Action {
        let planner = FeatureReadyPlanner::new(stub);
        planner
            .classify("FT-T254", "BNCH-002")
            .expect("classification succeeds")
    }

    /// Row 1 of the table — every dimension at its passing value
    /// resolves to `Done`. The DoR drive is finished; the operator
    /// may proceed to `dec drive ship`.
    #[test]
    fn tc_254_row_all_clean_yields_done() {
        let action = run(StubInspector::all_ready());
        assert_eq!(action, Action::Done);
    }

    /// Preflight warnings sit at the top of the precedence chain —
    /// even when every other dimension is ready, an unacknowledged
    /// cross-cutting gap is the Stuck reason and the gap list is
    /// cited verbatim so the operator can act.
    #[test]
    fn tc_254_row_preflight_warnings_yields_stuck_with_gap_list() {
        let mut s = StubInspector::all_ready();
        s.preflight = PreflightStatus::Warnings {
            gaps: vec!["ADR-070".to_string(), "ADR-071".to_string()],
        };
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("preflight:"), "reason was: {reason}");
                assert!(reason.contains("ADR-070"));
                assert!(reason.contains("ADR-071"));
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// An in-progress dep blocks the chain regardless of any
    /// downstream dimension. The Stuck reason cites the dep id
    /// verbatim per TC-255.
    #[test]
    fn tc_254_row_unfinished_dep_yields_stuck_blocked() {
        let mut s = StubInspector::all_ready();
        s.deps = vec![(
            "FT-DEP-1".to_string(),
            "in-progress".to_string(),
        )];
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("blocked: FT-DEP-1"));
                assert!(reason.contains("status=in-progress"));
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// Precedence: preflight > deps. With both failing the
    /// preflight reason wins.
    #[test]
    fn tc_254_precedence_preflight_beats_deps() {
        let mut s = StubInspector::all_ready();
        s.preflight = PreflightStatus::Warnings {
            gaps: vec!["ADR-070".to_string()],
        };
        s.deps = vec![("FT-DEP".to_string(), "in-progress".to_string())];
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("preflight:"), "reason was: {reason}");
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// A feature body missing required H2/H3 sections is Stuck
    /// before any TC / VG check fires.
    #[test]
    fn tc_254_row_spec_incomplete_yields_stuck() {
        let mut s = StubInspector::all_ready();
        s.spec_complete = false;
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("spec incomplete:"));
                assert!(reason.contains("FT-T254"));
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// Precedence: deps > spec. When the dep chain is broken AND
    /// the spec is incomplete, the deps reason wins.
    #[test]
    fn tc_254_precedence_deps_beats_spec() {
        let mut s = StubInspector::all_ready();
        s.deps = vec![("FT-DEP".to_string(), "in-progress".to_string())];
        s.spec_complete = false;
        match run(s) {
            Action::Stuck { reason } => assert!(reason.starts_with("blocked:")),
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// `tcs_linked = NoneLinked` is a distinct Stuck reason from
    /// `tcs_ready = false`; operators see "no TCs linked" without
    /// any TC id and know authoring (not amending) is the fix.
    #[test]
    fn tc_254_row_no_tcs_linked_yields_stuck() {
        let mut s = StubInspector::all_ready();
        s.tcs = TcsLinkedState::NoneLinked;
        match run(s) {
            Action::Stuck { reason } => assert_eq!(reason, "no TCs linked"),
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// TC quality failure cites the offending TC id verbatim
    /// (TC-255 contract). A single unready TC is enough to fail
    /// the row.
    #[test]
    fn tc_254_row_tc_unready_yields_stuck_with_tc_id() {
        let mut s = StubInspector::all_ready();
        s.tcs = TcsLinkedState::SomeUnready {
            problem_tc: "TC-T254a".to_string(),
            reason: "runner-args missing".to_string(),
        };
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("TC quality: TC-T254a"));
                assert!(reason.contains("runner-args missing"));
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// The single worker-resolvable row: every upstream dimension
    /// is ready, only the covering VG is missing. The planner
    /// dispatches verify-graph-author against the default env.
    #[test]
    fn tc_254_row_vg_missing_yields_dispatch_vga() {
        let mut s = StubInspector::all_ready();
        s.vgs = CoveringGraphState::Missing;
        match run(s) {
            Action::DispatchVerifyGraphAuthor {
                feature_id,
                env_id,
            } => {
                assert_eq!(feature_id, "FT-T254");
                assert_eq!(env_id, "BNCH-002");
            }
            other => panic!("expected DispatchVerifyGraphAuthor, got {other:?}"),
        }
    }

    /// Covering graph exists but sits in pending_review — Level-3
    /// human acceptance is the gate (ADR-030). The planner does
    /// not auto-accept; it cites the pending VG ids.
    #[test]
    fn tc_254_row_vg_pending_review_yields_stuck_with_ids() {
        let mut s = StubInspector::all_ready();
        s.vgs = CoveringGraphState::PendingReview {
            graph_ids: vec!["VG-T254b".to_string()],
        };
        match run(s) {
            Action::Stuck { reason } => {
                assert!(reason.starts_with("VG pending_review:"));
                assert!(reason.contains("VG-T254b"));
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    /// Precedence: vg_cover > vg_accepted. With both vgs_cover and
    /// vgs_accepted "failing" (i.e. covering graph missing), the
    /// missing-graph dispatch wins — pending_review only fires
    /// when a graph actually exists but isn't accepted.
    #[test]
    fn tc_254_precedence_vg_missing_beats_pending() {
        let mut s = StubInspector::all_ready();
        s.vgs = CoveringGraphState::Missing;
        // Forcing PendingReview here would contradict Missing; the
        // production semantics is that a single state encodes both
        // bits, so the precedence is captured by the enum's
        // exclusive variants. This test pins that Missing
        // dispatches, not Stuck.
        assert!(matches!(
            run(s),
            Action::DispatchVerifyGraphAuthor { .. }
        ));
    }
}
