//! `FeatureShipPlanner` — the FT+Ship planner.
//!
//! Classifier table (primary signal: `product verify FT-XXX`):
//!
//! | `product verify` | aggregate verdict | impl-open | vga-open | Action |
//! |---|---|---|---|---|
//! | passes | any | any | any | `Done` |
//! | fails | `Approved` | any | any | `DispatchImplementer` |
//! | fails | any | `> 0` | any | `DispatchImplementer` |
//! | fails | any | `0` | `> 0` | `DispatchVerifyGraphAuthor` |
//! | fails | `NeverRun` | `0` | `0` | `DispatchVerifier` |
//! | fails | `Rejected` / `Amendment` | `0` | `0` | `Stuck` |
//!
//! `product verify FT-XXX` is the authoritative completion
//! criterion per CLAUDE.md "Definition of done." When it passes,
//! the planner classifies Done regardless of VG verdict — VG
//! state is corroborating evidence at best and routinely lags
//! (witnessed: FT-113's drive marked TCs passing while a stale
//! VG-178/VG-179 run still emitted the rejected aggregate). When
//! it fails, VG-derived open defects route the workers via the
//! existing classifier table; the worst-VG-verdict signal stops
//! being the deciding criterion.

use std::cell::RefCell;
use std::collections::VecDeque;

use crate::core::drive::{Action, ArtifactKind, ArtifactRef, PlanContext, Planner};
use crate::core::drive::planner::PlanError;

use super::super::inspect::{FeatureVerdict, GraphInspector};

/// How many prior state-hashes to keep in the ring buffer for
/// graph-theoretic cycle detection. Catches cycles of period ≤ N
/// before they bleed off into the iteration cap. Eight is enough
/// for the realistic cases (verifier ↔ implementer ↔ vga ↔ defect
/// rotation) without dragging RAM or comparison cost.
const STATE_HASH_BUFFER_LEN: usize = 8;

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
    /// Ring buffer of recent state hashes (per feature). When a fresh
    /// classify observes a hash already in the buffer, the loop is in
    /// a cycle: the same (verdict, open-feedback-set, active-graph-set)
    /// can only produce the same dispatch decision by construction.
    /// Catches multi-step oscillations the pairwise prev/intended
    /// detector misses.
    recent_hashes: RefCell<RecentHashes>,
}

/// Per-feature ring buffer. We only track one feature at a time —
/// `dec drive ship FT-XXX` is single-feature — but we still keep the
/// feature id so a stale buffer from a prior driver invocation (in
/// process reuse, tests) doesn't false-positive on a different
/// feature.
#[derive(Debug, Default)]
struct RecentHashes {
    feature_id: String,
    hashes: VecDeque<u64>,
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
            recent_hashes: RefCell::new(RecentHashes::default()),
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

        // Ground-truth gate: `product verify FT-XXX` is the
        // authoritative completion criterion per CLAUDE.md
        // "Definition of done." It is also the primary classifier —
        // when it passes, the feature is done regardless of VG
        // verdict (which can lag behind: a stale rejected VGR from
        // before the fix isn't auto-superseded yet, but the strict
        // TC runners agree the code is correct). When it fails,
        // VG-derived open defects route the implementer / VGA via
        // the existing table; the worst-VG-verdict aggregate stops
        // being the deciding signal.
        let product_verify_passes = self
            .inspector
            .product_verify_passes_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        let intended = match (product_verify_passes, verdict, impl_open > 0, vga_open > 0) {
            (true, _, _, _) => Action::Done,
            (false, FeatureVerdict::Approved, _, _) => {
                // Aggregate VG approved but product verify still
                // fails. The VG's runners passed without strictly
                // exercising the TCs (witnessed: shell-command
                // `cargo test <name>` exits 0 on zero-match). Drop
                // back to dispatching the implementer; a fresh VGA
                // may be needed too, but that comes via normal
                // escalation.
                Action::DispatchImplementer {
                    feature_id: feature_id.to_string(),
                }
            }
            (false, _, true, _) => Action::DispatchImplementer {
                feature_id: feature_id.to_string(),
            },
            (false, _, _, true) => Action::DispatchVerifyGraphAuthor {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (false, FeatureVerdict::NeverRun, false, false) if !graphs_exist => {
                Action::DispatchVerifyGraphAuthor {
                    feature_id: feature_id.to_string(),
                    env_id: default_env_id.to_string(),
                }
            }
            (false, FeatureVerdict::NeverRun, false, false) => Action::DispatchVerifier {
                feature_id: feature_id.to_string(),
                env_id: default_env_id.to_string(),
            },
            (false, FeatureVerdict::Rejected | FeatureVerdict::AmendmentRequired, false, false)
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
            (false, FeatureVerdict::Rejected | FeatureVerdict::AmendmentRequired, false, false) => {
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

        let mut final_action = match self
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

        // Graph-theoretic cycle backstop. The pairwise no-progress
        // detector above catches two-in-a-row repeats; this catches
        // longer rotations (verifier → vga → implementer → verifier …
        // re-entering the same state-hash) the pairwise check can't
        // see. Every round records the hash so the buffer stays
        // continuous across escalations and settling rounds, but the
        // override only fires when the pairwise detector didn't
        // already decide — its diagnostic is more specific. Done is
        // already terminal.
        let cycle_period = self.detect_state_hash_cycle(feature_id)?;
        let pairwise_decided = matches!(
            final_action,
            Action::Stuck { .. }
                | Action::EscalateVgaToImplementer { .. }
                | Action::EscalateImplementerToVga { .. }
                | Action::Done
        );
        if !pairwise_decided {
            if let Some(cycle_len) = cycle_period {
                final_action = Action::Stuck {
                    reason: format!(
                        "feature {feature_id}: state-hash cycle of period \
                         {cycle_len} detected. The planner re-observed a \
                         (verdict, open-feedback-set, active-graph-set) \
                         state it saw earlier in this run, so the same \
                         dispatch will repeat by construction. The TC \
                         likely describes a real spec gap that needs \
                         spec-author attention. Inspect `dec loop show \
                         {feature_id}` for the chain."
                    ),
                };
            }
        }

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

    /// Compute the current state hash, check it against the ring
    /// buffer. Returns `Some(period)` when the hash already appears
    /// (period 1 = immediate repeat, 2 = AB cycle, 3 = ABC cycle, …).
    /// On miss, the hash is appended; the buffer is trimmed to
    /// `STATE_HASH_BUFFER_LEN`.
    ///
    /// Buffer is per-feature: a stale buffer from an earlier
    /// classify on a different feature is cleared on the first call
    /// for the new feature.
    fn detect_state_hash_cycle(
        &self,
        feature_id: &str,
    ) -> Result<Option<usize>, PlanError> {
        let hash = self
            .inspector
            .state_hash_for_feature(feature_id)
            .map_err(|e| PlanError::Store {
                detail: format!("{e}"),
            })?;
        let mut buf = self.recent_hashes.borrow_mut();
        if buf.feature_id != feature_id {
            buf.feature_id = feature_id.to_string();
            buf.hashes.clear();
        }
        // Newest-first: position 0 = previous classify, 1 = two ago,
        // so period = position + 1.
        if let Some(idx) = buf.hashes.iter().position(|&h| h == hash) {
            return Ok(Some(idx + 1));
        }
        buf.hashes.push_front(hash);
        while buf.hashes.len() > STATE_HASH_BUFFER_LEN {
            buf.hashes.pop_back();
        }
        Ok(None)
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
// FT-139 / ADR-080 — TaskType classifier branch.
// ---------------------------------------------------------------------

/// Classify a feature for typed cluster dispatch. Returns
/// `Some(Action::DispatchCluster { .. })` when the feature's front-matter
/// carries a `task_type:` value matching a registered TaskType in
/// `core::task_type::registry`. Returns `None` when the field is absent
/// or names an unknown TaskType — the caller falls through to the
/// broad-worker dispatch per ADR-080's escape-hatch principle.
///
/// Pure function: takes the parsed front-matter value rather than
/// reading disk, so unit tests can exercise it without I/O. The
/// disk-reading wrapper lives at `classify_for_task_type` (FT-139's
/// Phase 2 step 1).
#[must_use]
pub fn classify_for_task_type_value(
    feature_id: &str,
    task_type_value: Option<&str>,
) -> Option<crate::core::drive::Action> {
    let name = task_type_value?.trim();
    if name.is_empty() {
        return None;
    }
    if crate::core::task_type::lookup(name).is_some() {
        Some(crate::core::drive::Action::DispatchCluster {
            feature_id: feature_id.to_string(),
            task_type_name: name.to_string(),
        })
    } else {
        // Unknown TaskType → fall through to broad worker per ADR-080's
        // escape-hatch. Low-confidence ≡ no match.
        None
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
        fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
            Ok(stub_hash(self.verdict, self.impl_count, self.vga_count))
        }
    }

    /// Cheap state-hash for the stubs. Mirrors the production hash by
    /// using the same input dimensions (verdict, impl count, vga count).
    fn stub_hash(verdict: FeatureVerdict, impl_c: usize, vga_c: usize) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        format!("{verdict:?}").hash(&mut h);
        impl_c.hash(&mut h);
        vga_c.hash(&mut h);
        h.finish()
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

    /// The primary classifier: when `product verify FT-XXX` passes,
    /// the planner returns `Done` regardless of verdict / open
    /// defects / VG state. Per CLAUDE.md "Definition of done", that
    /// command is the authoritative signal; the VG-derived verdict
    /// is corroborating evidence at best and frequently lags
    /// (stale rejected VGRs not yet auto-superseded). The pre-gate
    /// rule "Approved → Done" was a strict subset of this new rule
    /// in the happy path: Approved + product-verify-passes still
    /// → Done. The change matters in the verdict=Rejected case,
    /// witnessed on FT-113's drive: product verify reported every
    /// TC PASS while the aggregate VG verdict was Rejected from a
    /// stale VG-178 / VG-179 run — pre-gate the planner stalled
    /// chasing the stale defects; post-gate it correctly classifies
    /// Done.
    #[test]
    fn product_verify_passes_returns_done_regardless_of_verdict() {
        struct PassesStub {
            verdict: FeatureVerdict,
            impl_count: usize,
            vga_count: usize,
        }
        impl GraphInspector for PassesStub {
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
                Ok(true)
            }
            fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
                Ok(stub_hash(self.verdict, self.impl_count, self.vga_count))
            }
            fn product_verify_passes_for_feature(
                &self,
                _: &str,
            ) -> Result<bool, InspectError> {
                Ok(true)
            }
        }
        for verdict in [
            FeatureVerdict::Approved,
            FeatureVerdict::Rejected,
            FeatureVerdict::AmendmentRequired,
            FeatureVerdict::NeverRun,
        ] {
            for impl_c in [0, 1, 5] {
                for vga_c in [0, 1, 5] {
                    let planner = FeatureShipPlanner::new(PassesStub {
                        verdict,
                        impl_count: impl_c,
                        vga_count: vga_c,
                    });
                    let action = planner.classify("FT-TEST", "ENV-002").unwrap();
                    assert!(
                        matches!(action, Action::Done),
                        "verdict={verdict:?} impl={impl_c} vga={vga_c} → {action:?}"
                    );
                }
            }
        }
    }

    /// Ground-truth gate: an Approved aggregate verdict alone is
    /// insufficient when the strict `product verify FT-XXX` runners
    /// report failure. Witnessed on FT-116's drive: a VGA authored a
    /// graph whose shell-command steps invoked `cargo test tc_239_…`
    /// which matched zero tests (the implementation didn't exist),
    /// exited 0, and the verifier called it Approved. The planner
    /// classified Done despite `product verify FT-116` failing all 7
    /// TCs. The gate dispatches the implementer instead so the loop
    /// can converge truthfully.
    #[test]
    fn approved_with_failing_product_verify_dispatches_implementer() {
        struct GatedStub;
        impl GraphInspector for GatedStub {
            fn aggregate_verdict_for_feature(
                &self,
                _: &str,
            ) -> Result<FeatureVerdict, InspectError> {
                Ok(FeatureVerdict::Approved)
            }
            fn open_defect_feedback_count(
                &self,
                _: &str,
                _: &str,
            ) -> Result<usize, InspectError> {
                Ok(0)
            }
            fn graphs_exist_for_feature(&self, _: &str) -> Result<bool, InspectError> {
                Ok(true)
            }
            fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
                Ok(stub_hash(FeatureVerdict::Approved, 0, 0))
            }
            fn product_verify_passes_for_feature(
                &self,
                _: &str,
            ) -> Result<bool, InspectError> {
                Ok(false)
            }
        }
        let planner = FeatureShipPlanner::new(GatedStub);
        let action = planner.classify("FT-TEST", "ENV-002").unwrap();
        match action {
            Action::DispatchImplementer { feature_id } => {
                assert_eq!(feature_id, "FT-TEST");
            }
            other => panic!(
                "expected DispatchImplementer when product verify fails, got {other:?}"
            ),
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
        fn state_hash_for_feature(&self, _: &str) -> Result<u64, InspectError> {
            Ok(stub_hash(
                self.verdict.get(),
                self.impl_count.get(),
                self.vga_count.get(),
            ))
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

    // -----------------------------------------------------------------
    // State-hash cycle detection: graph-theoretic backstop for the
    // multi-step rotations the pairwise no-progress detector misses.
    // -----------------------------------------------------------------

    #[test]
    fn period_two_cycle_between_impl_and_vga_returns_stuck() {
        // A: impl=2, vga=0 → DispatchImplementer
        // B: impl=0, vga=2 → DispatchVGA  (worker bounced defects)
        // back to A: impl=2, vga=0 → CYCLE detected (period 2)
        //
        // Pairwise can't see this because consecutive rounds dispatch
        // different roles, so its prev/intended pattern match fails.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 2, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let a1 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a1, Action::DispatchImplementer { .. }));
        planner.inspector.impl_count.set(0);
        planner.inspector.vga_count.set(2);
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        assert!(matches!(a2, Action::DispatchVerifyGraphAuthor { .. }));
        // Worker bounced back: same hash as round 1.
        planner.inspector.impl_count.set(2);
        planner.inspector.vga_count.set(0);
        let a3 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a3 {
            Action::Stuck { reason } => {
                assert!(
                    reason.contains("state-hash cycle"),
                    "reason: {reason}"
                );
                assert!(reason.contains("period 2"), "reason: {reason}");
            }
            other => panic!("expected Stuck from cycle detector, got {other:?}"),
        }
    }

    #[test]
    fn period_three_cycle_returns_stuck_with_period_three() {
        // Three distinct states rotated:
        //   A: impl=1, vga=0    → DispatchImplementer
        //   B: impl=0, vga=1    → DispatchVGA
        //   C: impl=2, vga=2    → DispatchImplementer (impl wins)
        // then back to A: impl=1, vga=0 → cycle of period 3.
        //
        // Each consecutive pair is impl→vga, vga→impl, impl→impl —
        // the impl→impl pair (C → A) drops impl_open (2 → 1), so the
        // pairwise detector treats that as progress and returns None.
        // Only the hash detector catches the rotation.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 1, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap(); // A
        planner.inspector.impl_count.set(0);
        planner.inspector.vga_count.set(1);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap(); // B
        planner.inspector.impl_count.set(2);
        planner.inspector.vga_count.set(2);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap(); // C
        planner.inspector.impl_count.set(1);
        planner.inspector.vga_count.set(0);
        let a4 = planner.classify("FT-TEST", "ENV-002").unwrap();
        match a4 {
            Action::Stuck { reason } => {
                assert!(
                    reason.contains("state-hash cycle"),
                    "reason: {reason}"
                );
                assert!(reason.contains("period 3"), "reason: {reason}");
            }
            other => panic!("expected Stuck from cycle detector, got {other:?}"),
        }
    }

    #[test]
    fn unique_state_every_round_does_not_false_positive() {
        // Strictly-decreasing impl count: each round observes a new
        // hash, so the cycle detector must stay quiet across the full
        // ring-buffer length and beyond.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 12, 0);
        let planner = FeatureShipPlanner::new(inspector);
        for round in (1..=12).rev() {
            planner.inspector.impl_count.set(round);
            let action = planner.classify("FT-TEST", "ENV-002").unwrap();
            assert!(
                matches!(action, Action::DispatchImplementer { .. }),
                "round impl={round} got {action:?}"
            );
        }
    }

    #[test]
    fn cycle_detector_defers_to_pairwise_diagnostic_when_pairwise_decides() {
        // Same state twice in a row dispatching the same role: the
        // pairwise detector decides first (escalation, or terminal
        // stuck on verifier). Cycle override should NOT replace its
        // more specific reason.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 3, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-TEST", "ENV-002").unwrap();
        let a2 = planner.classify("FT-TEST", "ENV-002").unwrap();
        // Pairwise wins: this is escalation, not "state-hash cycle".
        match a2 {
            Action::EscalateImplementerToVga { .. } => {}
            other => panic!(
                "expected pairwise EscalateImplementerToVga, got {other:?}"
            ),
        }
    }

    #[test]
    fn cycle_detector_resets_buffer_on_feature_id_change() {
        // A buffer left over from a different feature must not
        // false-positive on the new feature's first observation.
        let inspector = MutableStubInspector::new(FeatureVerdict::Rejected, 2, 0);
        let planner = FeatureShipPlanner::new(inspector);
        let _ = planner.classify("FT-A", "ENV-002").unwrap();
        let action = planner.classify("FT-B", "ENV-002").unwrap();
        assert!(matches!(action, Action::DispatchImplementer { .. }));
    }

    // ---------------------------------------------------------------------
    // TC-371 / FT-139 — classifier returns DispatchCluster for matching
    // task_type front-matter, falls through (None) on absent or unknown.
    // ---------------------------------------------------------------------

    #[test]
    fn classifier_returns_dispatch_cluster_for_task_type_frontmatter() {
        use crate::core::drive::Action;
        use super::classify_for_task_type_value;

        // Positive: known TaskType in registry.
        let action = classify_for_task_type_value("FT-T371", Some("add-judge-worker"))
            .expect("registered TaskType produces DispatchCluster");
        match action {
            Action::DispatchCluster {
                feature_id,
                task_type_name,
            } => {
                assert_eq!(feature_id, "FT-T371");
                assert_eq!(task_type_name, "add-judge-worker");
            }
            other => panic!("expected DispatchCluster, got {other:?}"),
        }

        // Fallthrough (absent): None → caller uses DispatchImplementer.
        assert!(
            classify_for_task_type_value("FT-T371", None).is_none(),
            "absent task_type falls through"
        );

        // Fallthrough (empty string): None.
        assert!(
            classify_for_task_type_value("FT-T371", Some("")).is_none(),
            "empty task_type falls through"
        );

        // Fallthrough (unknown): None per ADR-080's escape hatch
        // (low-confidence → broad worker, NOT a PlanError).
        assert!(
            classify_for_task_type_value("FT-T371", Some("not-a-real-task-type")).is_none(),
            "unknown task_type falls through to broad worker"
        );
    }

    // ---------------------------------------------------------------------
    // TC-353 / FT-140 — classifier dispatches add-author-worker cluster
    // when task_type front-matter matches; falls through to
    // DispatchImplementer (broad worker) when absent / unknown.
    // ---------------------------------------------------------------------

    #[test]
    fn author_worker_classifier_branch() {
        use crate::core::drive::Action;
        use super::classify_for_task_type_value;

        // Positive: registered add-author-worker → DispatchCluster.
        let action = classify_for_task_type_value("FT-T353", Some("add-author-worker"))
            .expect("registered add-author-worker dispatches cluster");
        match action {
            Action::DispatchCluster {
                feature_id,
                task_type_name,
            } => {
                assert_eq!(feature_id, "FT-T353");
                assert_eq!(task_type_name, "add-author-worker");
            }
            other => panic!("expected DispatchCluster, got {other:?}"),
        }

        // Fallthrough (absent task_type) → broad worker.
        assert!(classify_for_task_type_value("FT-T353", None).is_none());

        // Fallthrough (mistyped) → broad worker.
        assert!(
            classify_for_task_type_value("FT-T353", Some("add_author_worker")).is_none(),
            "underscore variant is not the kebab-case registered name"
        );
    }
}
