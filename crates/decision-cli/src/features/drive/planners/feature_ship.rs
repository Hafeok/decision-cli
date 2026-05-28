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

use std::cell::RefCell;

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
    /// Snapshot from the most recent `classify` call. Used to detect
    /// non-convergence: if a dispatch round didn't reduce the
    /// open-feedback count for the role we just dispatched, we are
    /// stuck and should escalate rather than spin.
    last_seen: RefCell<Option<LastSeen>>,
}

/// Snapshot of the planner's observed state from a prior classify
/// call. `final_action` records what classify() actually returned to
/// the driver (i.e., what the executor ran in the prior iteration);
/// `escalated_in_chain` tracks whether the driver has already used
/// one of the two escalations on this feature, so we can return a
/// terminal Stuck rather than ping-ponging back and forth.
#[derive(Debug, Clone)]
struct LastSeen {
    feature_id: String,
    verdict: FeatureVerdict,
    impl_open: usize,
    vga_open: usize,
    final_action: Action,
    escalated_in_chain: bool,
}

impl<I: GraphInspector> FeatureShipPlanner<I> {
    /// Construct with an explicit inspector. Production callers use
    /// `ProductionInspector::new(ctx)`; tests pass a stub.
    pub fn new(inspector: I) -> Self {
        Self {
            inspector,
            last_seen: RefCell::new(None),
        }
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

        // Check graphs_exist for both NeverRun and the failing-with-
        // no-feedback branches: in both, the planner needs to know
        // whether a real evidence-emitting graph is available before
        // deciding between "run the verifier" and "author a new
        // graph first." The check reads on-disk .ttl files
        // (authoritative for step definitions) rather than relying
        // on the store, which can carry stale providesEvidenceFor
        // cruft from past sessions.
        let needs_graphs_check = matches!(
            verdict,
            FeatureVerdict::NeverRun
                | FeatureVerdict::Rejected
                | FeatureVerdict::AmendmentRequired
        );
        let graphs_exist = if needs_graphs_check {
            self.inspector
                .graphs_exist_for_feature(feature_id)
                .map_err(|e| PlanError::Store {
                    detail: format!("{e}"),
                })?
        } else {
            true
        };

        let intended = match (verdict, impl_open > 0, vga_open > 0) {
            (FeatureVerdict::Approved, _, _) => Action::Done,
            (_, true, _) => Action::DispatchImplementer {
                feature_id: feature_id.to_string(),
            },
            (_, _, true) => Action::DispatchVerifyGraphAuthor {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (FeatureVerdict::NeverRun, false, false) if !graphs_exist => {
                Action::DispatchVerifyGraphAuthor {
                    feature_id: feature_id.to_string(),
                    env_id: default_env_id.to_string(),
                }
            }
            (FeatureVerdict::NeverRun, false, false) => Action::DispatchVerifier {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (FeatureVerdict::Rejected | FeatureVerdict::AmendmentRequired, false, false)
                if !graphs_exist =>
            {
                // Verdict says failing but no evidence-emitting
                // graph exists on disk for the feature's TCs (any
                // VGRs we see were emitted by graphs that don't
                // actually cover this feature — stale
                // providesEvidenceFor cruft in the store). Bootstrap
                // a real graph rather than re-running ghosts.
                Action::DispatchVerifyGraphAuthor {
                    feature_id: feature_id.to_string(),
                    env_id: default_env_id.to_string(),
                }
            }
            (FeatureVerdict::Rejected | FeatureVerdict::AmendmentRequired, false, false) => {
                // Verdict is failing but no defect feedback is open
                // — either everything was previously addressed
                // (lifecycle transitions ate the evidence) or the
                // open defects were emitted by graphs that have
                // since been superseded (filtered out by the
                // inspector). Re-dispatch the verifier to refresh
                // evidence; if its run also produces no new
                // feedback the (Verifier, Verifier) no-state-change
                // detector will return terminal Stuck on the next
                // iteration. This gives the loop a chance to
                // continue automatically instead of bailing on a
                // stale snapshot.
                Action::DispatchVerifier {
                    feature_id: feature_id.to_string(),
                    env_id: default_env_id.to_string(),
                }
            }
        };

        let prior_escalated = self
            .last_seen
            .borrow()
            .as_ref()
            .filter(|p| p.feature_id == feature_id)
            .map_or(false, |p| p.escalated_in_chain);

        let final_action = match self
            .detect_no_progress(feature_id, verdict, impl_open, vga_open, &intended)
        {
            Some(NoProgress::Stuck { reason }) => Action::Stuck { reason },
            Some(NoProgress::EscalateVgaToImplementer) => Action::EscalateVgaToImplementer {
                feature_id: feature_id.to_string(),
            },
            Some(NoProgress::EscalateImplementerToVga) => Action::EscalateImplementerToVga {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            None => intended.clone(),
        };

        let now_escalated = matches!(
            final_action,
            Action::EscalateVgaToImplementer { .. } | Action::EscalateImplementerToVga { .. }
        );

        *self.last_seen.borrow_mut() = Some(LastSeen {
            feature_id: feature_id.to_string(),
            verdict,
            impl_open,
            vga_open,
            final_action: final_action.clone(),
            escalated_in_chain: prior_escalated || now_escalated,
        });

        Ok(final_action)
    }

    /// Compare the current observed state against the prior snapshot.
    /// Returns `Some(reason)` when the most recent dispatch round
    /// failed to reduce the open-defect count for the role that was
    /// dispatched — meaning another dispatch of the same kind would
    /// be unproductive.
    ///
    /// Implementer dispatches expect `impl_open` to drop; verifier
    /// dispatches expect `vga_open` to drop. If a round didn't
    /// reduce its corresponding count by at least one, the worker
    /// either failed to emit `addressed_feedback_iris` (the
    /// transition never fired) or genuinely couldn't address the
    /// feedback — both are stuck-states for the planner.
    fn detect_no_progress(
        &self,
        feature_id: &str,
        verdict: FeatureVerdict,
        impl_open: usize,
        vga_open: usize,
        intended: &Action,
    ) -> Option<NoProgress> {
        let prev = self.last_seen.borrow();
        let prev = prev.as_ref()?;
        if prev.feature_id != feature_id {
            return None;
        }
        // Settling round: the round immediately after an Escalate
        // executor runs sees rerouted defect counts that the
        // pre-escalation baseline can't fairly compare against.
        // Skip detection entirely — the next round will pair up two
        // post-escalation observations and detect normally.
        if matches!(
            prev.final_action,
            Action::EscalateVgaToImplementer { .. }
                | Action::EscalateImplementerToVga { .. }
        ) {
            return None;
        }
        match (&prev.final_action, intended) {
            (
                Action::DispatchImplementer { .. },
                Action::DispatchImplementer { .. },
            ) => no_progress_for_impl(prev, feature_id, impl_open),
            (
                Action::DispatchVerifyGraphAuthor { .. },
                Action::DispatchVerifyGraphAuthor { .. },
            ) => no_progress_for_vga(prev, feature_id, vga_open),
            (
                Action::DispatchVerifier { .. },
                Action::DispatchVerifier { .. },
            ) => {
                if verdict == prev.verdict
                    && impl_open == prev.impl_open
                    && vga_open == prev.vga_open
                {
                    Some(NoProgress::Stuck {
                        reason: format!(
                            "verifier dispatch did not change state \
                             (verdict={verdict:?}, impl_open={impl_open}, \
                             vga_open={vga_open}) for feature {feature_id}. The \
                             verifier likely had no covering graphs to run, or \
                             the run did not emit feedback. Inspect `dec verify \
                             feature {feature_id}` and `dec loop show \
                             {feature_id}` to diagnose.",
                        ),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Implementer no-progress: only fires when prev had defects to fix
/// (so the count going up from 0 doesn't false-positive) and the new
/// count didn't strictly decrease. When `escalated_in_chain` is
/// already true (we used the other escalation earlier), this is the
/// terminal stuck — both directions tried, both failed.
fn no_progress_for_impl(prev: &LastSeen, feature_id: &str, impl_open: usize) -> Option<NoProgress> {
    if prev.impl_open == 0 || impl_open < prev.impl_open {
        return None;
    }
    if prev.escalated_in_chain {
        return Some(NoProgress::Stuck {
            reason: format!(
                "feature {feature_id}: both escalation directions exhausted. \
                 We already routed the defects across the verifier↔implementer \
                 boundary once, and the implementer still can't make progress \
                 (impl_open {prev_n} → {new_n}). The TC most likely describes a \
                 real spec gap that needs spec-author attention. Inspect `dec \
                 loop show {feature_id}` for the chain.",
                prev_n = prev.impl_open,
                new_n = impl_open,
            ),
        });
    }
    Some(NoProgress::EscalateImplementerToVga)
}

/// Verify-graph-author no-progress: symmetric to the implementer
/// branch. Bootstrap (prev.vga_open == 0) is treated as
/// evidence-production, not regression.
fn no_progress_for_vga(prev: &LastSeen, feature_id: &str, vga_open: usize) -> Option<NoProgress> {
    if prev.vga_open == 0 || vga_open < prev.vga_open {
        return None;
    }
    if prev.escalated_in_chain {
        return Some(NoProgress::Stuck {
            reason: format!(
                "feature {feature_id}: both escalation directions exhausted. \
                 We already routed the defects across the verifier↔implementer \
                 boundary once, and the verify-graph-author still can't make \
                 progress (vga_open {prev_n} → {new_n}). The TC most likely \
                 describes a real spec gap that needs spec-author attention. \
                 Inspect `dec loop show {feature_id}` for the chain.",
                prev_n = prev.vga_open,
                new_n = vga_open,
            ),
        });
    }
    Some(NoProgress::EscalateVgaToImplementer)
}

/// What `detect_no_progress` decided the planner should do next.
/// Distinguishes terminal `Stuck` (driver returns DriveError::Stuck)
/// from the two escalation directions, which re-route open defects
/// across the targetRole boundary and try one more round.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NoProgress {
    Stuck { reason: String },
    EscalateVgaToImplementer,
    EscalateImplementerToVga,
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
        fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
            // Existing tests assume covering graphs exist — only the new
            // graphs-do-not-exist test overrides this via the mutable stub.
            Ok(true)
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
    fn rejected_with_no_open_feedback_redispatches_verifier() {
        // Verdict-failing-but-no-feedback means the prior verifier
        // ran, all the feedback got addressed (or filtered as
        // superseded), and we now need a fresh verifier run to
        // re-emit evidence. Loop should NOT immediately stuck; the
        // (Verifier, Verifier) no-state-change detector handles the
        // case where the re-run also produces nothing.
        let action = run_case(FeatureVerdict::Rejected, 0, 0);
        assert!(matches!(action, Action::DispatchVerifier { .. }), "got {action:?}");
    }

    #[test]
    fn amendment_required_with_no_open_feedback_redispatches_verifier() {
        let action = run_case(FeatureVerdict::AmendmentRequired, 0, 0);
        assert!(matches!(action, Action::DispatchVerifier { .. }), "got {action:?}");
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

    // -----------------------------------------------------------------
    // Convergence-detection tests. Stateful planner: state evolves
    // across consecutive classify() calls via interior mutability.
    // The mutable stub lets us model the count-change a real
    // dispatch would produce.
    // -----------------------------------------------------------------

    use std::cell::Cell;

    struct MutableStubInspector {
        verdict: Cell<FeatureVerdict>,
        impl_count: Cell<usize>,
        vga_count: Cell<usize>,
        graphs_exist: Cell<bool>,
    }

    impl MutableStubInspector {
        fn new(v: FeatureVerdict, i: usize, g: usize) -> Self {
            Self {
                verdict: Cell::new(v),
                impl_count: Cell::new(i),
                vga_count: Cell::new(g),
                graphs_exist: Cell::new(true),
            }
        }
    }

    impl GraphInspector for MutableStubInspector {
        fn aggregate_verdict_for_feature(
            &self,
            _: &str,
        ) -> Result<FeatureVerdict, InspectError> {
            Ok(self.verdict.get())
        }
        fn open_defect_feedback_count(
            &self,
            _: &str,
            role_id: &str,
        ) -> Result<usize, InspectError> {
            Ok(match role_id {
                "implementer" => self.impl_count.get(),
                "verifier" => self.vga_count.get(),
                _ => 0,
            })
        }
        fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
            Ok(self.graphs_exist.get())
        }
    }

    #[test]
    fn implementer_repeated_with_no_progress_escalates_to_vga() {
        // Round 1: 5 implementer-defects, dispatch implementer.
        // Round 2: same 5 defects (worker no-op), expect
        // EscalateImplementerToVga (mirror of the vga→impl
        // escalation — when impl can't fix, ask verifier to
        // re-author the test).
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 5, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchImplementer { .. }));
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a2 {
            Action::EscalateImplementerToVga { feature_id, env_id } => {
                assert_eq!(feature_id, "FT-TEST");
                assert_eq!(env_id, "ENV-002");
            }
            other => panic!("expected EscalateImplementerToVga, got {other:?}"),
        }
    }

    #[test]
    fn vga_to_impl_escalation_then_impl_no_progress_is_terminal_stuck() {
        // Sequence: VGA twice with no progress → escalate to impl
        // → settling round (impl dispatches normally, no detection)
        // → impl still no progress → terminal Stuck (both directions
        // exhausted).
        let inspector = MutableStubInspector::new(FeatureVerdict::AmendmentRequired, 0, 3);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap();
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::EscalateVgaToImplementer { .. }));
        // Simulate the escalation executor: vga drops, impl jumps.
        planner.inspector.vga_count.set(0);
        planner.inspector.impl_count.set(3);
        let a3 = planner.classify("FT-TEST", "ENV-002").unwrap();
        // Settling round — detection skipped; planner dispatches
        // implementer against the rerouted defects.
        assert!(matches!(a3, Action::DispatchImplementer { .. }), "got {a3:?}");
        // Implementer ran but couldn't reduce impl_open. Now the
        // detector fires terminal Stuck because escalation was used.
        let a4 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a4 {
            Action::Stuck { reason } => {
                assert!(
                    reason.contains("both escalation directions exhausted"),
                    "reason: {reason}"
                );
                assert!(reason.contains("spec-author"), "reason: {reason}");
            }
            other => panic!("expected terminal Stuck, got {other:?}"),
        }
    }

    #[test]
    fn impl_to_vga_escalation_then_vga_no_progress_is_terminal_stuck() {
        // Symmetric sequence: impl twice no progress → escalate to
        // VGA → settling round → VGA still no progress → terminal
        // Stuck.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 3, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap();
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::EscalateImplementerToVga { .. }));
        planner.inspector.impl_count.set(0);
        planner.inspector.vga_count.set(3);
        let a3 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a3, Action::DispatchVerifyGraphAuthor { .. }), "got {a3:?}");
        let a4 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a4 {
            Action::Stuck { reason } => {
                assert!(
                    reason.contains("both escalation directions exhausted"),
                    "reason: {reason}"
                );
            }
            other => panic!("expected terminal Stuck, got {other:?}"),
        }
    }

    #[test]
    fn escalation_then_progress_continues_without_stuck() {
        // If the post-escalation worker actually makes progress, the
        // detector must NOT terminal-stuck on the round after that.
        // Sequence: impl no-progress → escalate → settling → VGA
        // makes progress → no stuck.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 3, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap();
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap();
        planner.inspector.impl_count.set(0);
        planner.inspector.vga_count.set(3);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap(); // settling
        planner.inspector.vga_count.set(1); // VGA fixed two of three
        let a4 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(
            matches!(a4, Action::DispatchVerifyGraphAuthor { .. }),
            "expected continued dispatch, got {a4:?}"
        );
    }

    #[test]
    fn implementer_repeated_with_progress_continues_dispatching() {
        // Round 1: 5 defects, dispatch.
        // Round 2: 3 defects (worker fixed 2), dispatch again.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 5, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchImplementer { .. }));
        // Simulate dispatch result: 2 defects addressed.
        planner.inspector.impl_count.set(3);
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::DispatchImplementer { .. }));
    }

    #[test]
    fn vga_repeated_with_no_progress_escalates_to_implementer() {
        // Round 1: 0 implementer, 3 verifier defects, dispatch vga.
        // Round 2: same 3 defects, expect EscalateVgaToImplementer
        // (verifier can't fix → ask implementer to fix the underlying
        // gap).
        let inspector = MutableStubInspector::new(FeatureVerdict::AmendmentRequired, 0, 3);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchVerifyGraphAuthor { .. }));
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a2 {
            Action::EscalateVgaToImplementer { feature_id } => {
                assert_eq!(feature_id, "FT-TEST");
            }
            other => panic!("expected EscalateVgaToImplementer, got {other:?}"),
        }
    }

    #[test]
    fn vga_repeated_with_progress_continues_dispatching() {
        let inspector = MutableStubInspector::new(FeatureVerdict::AmendmentRequired, 0, 4);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchVerifyGraphAuthor { .. }));
        planner.inspector.vga_count.set(1);
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::DispatchVerifyGraphAuthor { .. }));
    }

    #[test]
    fn vga_bootstrap_count_increase_is_not_stuck() {
        // FT-104 regression: first VGA dispatch on a no-graph feature
        // bootstraps the verify suite by authoring + running the
        // graph. The new run emits defects, so vga_open goes 0 → N.
        // That's evidence-production, not regression — must NOT
        // trigger the no-progress stuck branch.
        let inspector = MutableStubInspector::new(FeatureVerdict::NeverRun, 0, 0);
        inspector.graphs_exist.set(false);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-104", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchVerifyGraphAuthor { .. }));
        // Simulate the bootstrap result: graphs now exist, the auto-run
        // emitted 3 verifier-defects.
        planner.inspector.graphs_exist.set(true);
        planner.inspector.verdict.set(FeatureVerdict::AmendmentRequired);
        planner.inspector.vga_count.set(3);
        let a2 = planner.classify("FT-104", "ENV-002").unwrap();
        assert!(
            matches!(a2, Action::DispatchVerifyGraphAuthor { .. }),
            "got {a2:?}"
        );
    }

    #[test]
    fn never_run_without_graphs_dispatches_verify_graph_author() {
        // FT-104 regression: no covering graphs ⇒ DispatchVerifier
        // would be a no-op (nothing to run) and the loop would spin
        // until max_iter. Planner must author a graph first.
        let inspector = MutableStubInspector::new(FeatureVerdict::NeverRun, 0, 0);
        inspector.graphs_exist.set(false);
        let planner = FeatureShipPlanner::new(inspector);
        let action = planner.classify("FT-104", "ENV-002").unwrap();
        match action {
            Action::DispatchVerifyGraphAuthor { feature_id, env_id } => {
                assert_eq!(feature_id, "FT-104");
                assert_eq!(env_id, "ENV-002");
            }
            other => panic!("expected DispatchVerifyGraphAuthor, got {other:?}"),
        }
    }

    #[test]
    fn verifier_repeated_with_no_state_change_returns_stuck() {
        // FT-104 regression backstop: if for some reason the planner
        // dispatches verifier twice in a row and nothing changes,
        // we must not spin to max_iter — the detector now catches
        // it with a verifier-specific reason.
        let inspector = MutableStubInspector::new(FeatureVerdict::NeverRun, 0, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchVerifier { .. }));
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a2 {
            Action::Stuck { reason } => {
                assert!(reason.contains("verifier"), "reason: {reason}");
                assert!(reason.contains("did not change state"), "reason: {reason}");
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    #[test]
    fn verifier_then_implementer_does_not_falsely_flag_stuck() {
        // Round 1: never-run, dispatch verifier.
        // Round 2: post-verify, 5 impl defects appear. Different
        // dispatch shape — must NOT flag stuck even though counts
        // diverged from the prior (which were 0).
        let inspector = MutableStubInspector::new(FeatureVerdict::NeverRun, 0, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchVerifier { .. }));
        planner.inspector.verdict.set(FeatureVerdict::Rejected);
        planner.inspector.impl_count.set(5);
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::DispatchImplementer { .. }));
    }
}
