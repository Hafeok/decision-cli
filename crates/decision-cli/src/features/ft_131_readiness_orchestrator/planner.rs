//! `FeatureReadyOrchestratorPlanner` — FT-131 readiness orchestrator.
//!
//! Extends FT-119's DoR planner with the full upstream authoring chain.
//! Classification table (first match wins, precedence enforced by guard
//! order in `classify`):
//!
//! | spec_ready | adr_acks | preflight | deps_done | tcs_linked | tcs_ready | vgs_cover | vgs_ready | Action                         |
//! |------------|----------|-----------|-----------|------------|-----------|-----------|-----------|--------------------------------|
//! | ready      | ready    | clean     | done      | true       | true      | true      | true      | Done                           |
//! | *          | *        | *         | false     | *          | *         | *         | *         | Stuck "blocked: FT-Y..."       |
//! | false      | *        | *         | true      | *          | *         | *         | *         | DispatchSpecAuthor (Slice B)   |
//! | pending    | *        | *         | true      | *          | *         | *         | *         | Stuck "spec pending..."        |
//! | ready      | false    | warnings  | true      | *          | *         | *         | *         | DispatchAdrAuthor (Slice B)    |
//! | ready      | pending  | warnings  | true      | *          | *         | *         | *         | Stuck "adr pending..."         |
//! | ready      | ready    | warnings  | true      | *          | *         | *         | *         | Stuck "preflight: <gaps>"      |
//! | ready      | ready    | clean     | true      | false      | *         | *         | *         | DispatchTcAuthor               |
//! | ready      | ready    | clean     | true      | pending    | *         | *         | *         | DispatchTcQuality              |
//! | ready      | ready    | clean     | true      | true       | false     | *         | *         | Stuck "tc rejected..."         |
//! | ready      | ready    | clean     | true      | true       | true      | false     | *         | DispatchVerifyGraphAuthor      |
//! | ready      | ready    | clean     | true      | true       | true      | pending   | *         | DispatchVgQuality              |
//! | ready      | ready    | clean     | true      | true       | true      | true      | false     | Stuck "vg rejected..."         |
//!
//! With `--no-author`, every DispatchAuthor / DispatchQuality row
//! collapses to Stuck with FT-119's reason format.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use crate::core::drive::planner::PlanError;
use crate::core::drive::{Action, PlanContext, Planner};

use crate::features::drive::inspect::{
    CoveringGraphState, GraphInspector, PreflightGap, PreflightStatus,
    TcsLinkedState,
};

/// How many prior state hashes to retain (matches FT-119).
const STATE_HASH_BUFFER_LEN: usize = 8;

/// Planner for `dec drive def-ready FT-XXX` with authoring orchestration.
pub struct FeatureReadyOrchestratorPlanner<I: GraphInspector> {
    inspector: I,
    /// --no-author flag: collapses all authoring rows to Stuck.
    no_author: bool,
    /// Pairwise no-progress snapshot.
    last_seen: RefCell<Option<LastSeen>>,
    /// Per-feature ring buffer of recent state hashes.
    recent_hashes: RefCell<RecentHashes>,
}

#[derive(Debug, Default)]
struct RecentHashes {
    feature_id: String,
    hashes: VecDeque<u64>,
}

#[derive(Debug, Clone)]
struct LastSeen {
    feature_id: String,
    state_hash: u64,
    final_action: Action,
}

impl<I: GraphInspector> FeatureReadyOrchestratorPlanner<I> {
    /// Wire the planner against an inspector.
    pub fn new(inspector: I, no_author: bool) -> Self {
        Self {
            inspector,
            no_author,
            last_seen: RefCell::new(None),
            recent_hashes: RefCell::new(RecentHashes::default()),
        }
    }

    /// Pure classification (testable without PlanContext).
    pub fn classify(
        &self,
        feature_id: &str,
        default_env_id: &str,
    ) -> Result<Action, PlanError> {
        let (proposed, state_hash) =
            self.classify_and_hash(feature_id, default_env_id)?;
        let final_action =
            self.apply_cycle_detection_with_hash(feature_id, proposed, state_hash);
        self.record_observation_with_hash(feature_id, &final_action, state_hash);
        Ok(final_action)
    }

    /// Compatibility shim for tests that want pre-cycle-check action.
    #[cfg(test)]
    #[allow(dead_code)]
    fn classify_without_cycle_check(
        &self,
        feature_id: &str,
        default_env_id: &str,
    ) -> Result<Action, PlanError> {
        Ok(self.classify_and_hash(feature_id, default_env_id)?.0)
    }

    /// Classify and hash each dimension as it is read.
    fn classify_and_hash(
        &self,
        feature_id: &str,
        default_env_id: &str,
    ) -> Result<(Action, u64), PlanError> {
        let mut h = DefaultHasher::new();
        feature_id.hash(&mut h);

        // 1. Dependencies gate everything.
        let deps = self
            .inspector
            .dependency_statuses_for_feature(feature_id)
            .map_err(store_err)?;
        for (id, status) in &deps {
            id.hash(&mut h);
            status.hash(&mut h);
        }
        if let Some((dep_id, status)) = first_unfinished_dep(&deps) {
            return Ok((
                Action::Stuck {
                    reason: stuck_blocked(dep_id, status),
                },
                h.finish(),
            ));
        }

        // 2. Spec completeness (Slice B — stub for now).
        let spec_ready = self
            .inspector
            .spec_quality_approved(feature_id)
            .map_err(store_err)?;
        spec_ready.hash(&mut h);
        if !spec_ready && !self.no_author {
            return Ok((
                Action::DispatchSpecAuthor {
                    feature_id: feature_id.to_string(),
                },
                h.finish(),
            ));
        }
        if !spec_ready && self.no_author {
            return Ok((
                Action::Stuck {
                    reason: format!("[no-author] spec not ready for {feature_id}"),
                },
                h.finish(),
            ));
        }

        // 3. ADR acknowledgements (Slice B — stub for now).
        let adr_acks_ready = self
            .inspector
            .adr_acks_quality_approved(feature_id)
            .map_err(store_err)?;
        adr_acks_ready.hash(&mut h);

        // 4. Preflight.
        let preflight = self
            .inspector
            .preflight_status_for_feature(feature_id)
            .map_err(store_err)?;
        hash_preflight(&preflight, &mut h);
        if !adr_acks_ready && matches!(preflight, PreflightStatus::Warnings { .. }) {
            if !self.no_author {
                if let PreflightStatus::Warnings { gaps } = &preflight {
                    let first_gap = gaps.first().map(gap_to_string).unwrap_or_default();
                    return Ok((
                        Action::DispatchAdrAuthor {
                            feature_id: feature_id.to_string(),
                            preflight_gap: first_gap,
                        },
                        h.finish(),
                    ));
                }
            } else {
                if let PreflightStatus::Warnings { gaps } = &preflight {
                    return Ok((
                        Action::Stuck {
                            reason: format!("[no-author] {}", stuck_preflight(gaps)),
                        },
                        h.finish(),
                    ));
                }
            }
        }
        if let PreflightStatus::Warnings { gaps } = preflight {
            return Ok((
                Action::Stuck {
                    reason: stuck_preflight(&gaps),
                },
                h.finish(),
            ));
        }

        // 5. TCs linked.
        let tcs = self
            .inspector
            .tcs_linked_state_for_feature(feature_id)
            .map_err(store_err)?;
        hash_tcs(&tcs, &mut h);
        match &tcs {
            TcsLinkedState::NoneLinked => {
                if !self.no_author {
                    return Ok((
                        Action::DispatchTcAuthor {
                            feature_id: feature_id.to_string(),
                            target_count: 4, // ADR-072 default floor
                        },
                        h.finish(),
                    ));
                } else {
                    return Ok((
                        Action::Stuck {
                            reason: "[no-author] no TCs linked".to_string(),
                        },
                        h.finish(),
                    ));
                }
            }
            TcsLinkedState::SomeUnready { problem_tc, reason } => {
                // Check if we have pending proposals that need quality judging
                let pending_count = self
                    .inspector
                    .pending_proposals_count(feature_id)
                    .map_err(store_err)?;
                pending_count.hash(&mut h);

                if pending_count > 0 && !self.no_author {
                    return Ok((
                        Action::DispatchTcQuality {
                            feature_id: feature_id.to_string(),
                            tc_proposal_iri: format!("pending-tc-proposal-{problem_tc}"),
                        },
                        h.finish(),
                    ));
                }

                return Ok((
                    Action::Stuck {
                        reason: stuck_tc_quality(problem_tc, reason),
                    },
                    h.finish(),
                ));
            }
            TcsLinkedState::AllReady => {}
        }

        // 6. TCs quality verdicts (new FT-131 dimension).
        let tc_verdicts_count = self
            .inspector
            .tc_quality_verdicts_count(feature_id)
            .map_err(store_err)?;
        tc_verdicts_count.hash(&mut h);

        // If no quality verdicts but TCs are linked and ready (old-style),
        // treat as ready for backward compatibility.
        let tcs_quality_ready = tc_verdicts_count > 0 || matches!(tcs, TcsLinkedState::AllReady);

        if !tcs_quality_ready {
            return Ok((
                Action::Stuck {
                    reason: "TC quality verdicts missing".to_string(),
                },
                h.finish(),
            ));
        }

        // 7. FT-138: open implementer feedback → Done.
        let open_feedback = self
            .inspector
            .has_open_implementer_feedback_for_feature(feature_id)
            .map_err(store_err)?;
        open_feedback.hash(&mut h);
        if open_feedback {
            return Ok((Action::Done, h.finish()));
        }

        // 8. Covering graph state.
        let vgs = self
            .inspector
            .covering_graph_state_for_feature(feature_id, default_env_id)
            .map_err(store_err)?;
        hash_vgs(&vgs, &mut h);
        match &vgs {
            CoveringGraphState::Missing => {
                if !self.no_author {
                    return Ok((
                        Action::DispatchVerifyGraphAuthor {
                            feature_id: feature_id.to_string(),
                            env_id: default_env_id.to_string(),
                        },
                        h.finish(),
                    ));
                } else {
                    return Ok((
                        Action::Stuck {
                            reason: "[no-author] VG missing".to_string(),
                        },
                        h.finish(),
                    ));
                }
            }
            CoveringGraphState::PendingReview { graph_ids } => {
                // Check if we should dispatch VG quality judge
                let pending_count = self
                    .inspector
                    .pending_proposals_count(feature_id)
                    .map_err(store_err)?;

                if pending_count > 0 && !self.no_author {
                    let first_vg = graph_ids.first().map(|s| s.as_str()).unwrap_or("");
                    return Ok((
                        Action::DispatchVgQuality {
                            feature_id: feature_id.to_string(),
                            graph_proposal_iri: format!("pending-vg-proposal-{first_vg}"),
                        },
                        h.finish(),
                    ));
                }

                return Ok((
                    Action::Stuck {
                        reason: stuck_vg_pending(graph_ids),
                    },
                    h.finish(),
                ));
            }
            CoveringGraphState::AcceptedAll => {}
        }

        // 9. VG quality verdicts (new FT-131 dimension).
        let vg_verdicts_count = self
            .inspector
            .vg_quality_verdicts_count(feature_id)
            .map_err(store_err)?;
        vg_verdicts_count.hash(&mut h);

        // If no quality verdicts but VGs are accepted (old-style),
        // treat as ready for backward compatibility.
        let vgs_quality_ready = vg_verdicts_count > 0 || matches!(vgs, CoveringGraphState::AcceptedAll);

        if !vgs_quality_ready {
            return Ok((
                Action::Stuck {
                    reason: "VG quality verdicts missing".to_string(),
                },
                h.finish(),
            ));
        }

        Ok((Action::Done, h.finish()))
    }

    /// PAT-002 cycle detection.
    fn apply_cycle_detection_with_hash(
        &self,
        feature_id: &str,
        proposed: Action,
        state_hash: u64,
    ) -> Action {
        if proposed.is_terminal() {
            return proposed;
        }

        // Pairwise check.
        if let Some(prev) = self.last_seen.borrow().as_ref() {
            if prev.feature_id == feature_id
                && prev.state_hash == state_hash
                && action_kind(&prev.final_action) == action_kind(&proposed)
            {
                return Action::Stuck {
                    reason: format!(
                        "{tag} dispatch did not change state for {feature_id}",
                        tag = proposed.tag(),
                    ),
                };
            }
        }

        // Ring-buffer check.
        let mut buf = self.recent_hashes.borrow_mut();
        if buf.feature_id != feature_id {
            buf.feature_id = feature_id.to_string();
            buf.hashes.clear();
        }
        if let Some(period) = position_from_head(&buf.hashes, state_hash) {
            return Action::Stuck {
                reason: format!(
                    "cycle: {period} rounds on {tag}",
                    tag = proposed.tag()
                ),
            };
        }

        proposed
    }

    fn record_observation_with_hash(
        &self,
        feature_id: &str,
        final_action: &Action,
        state_hash: u64,
    ) {
        *self.last_seen.borrow_mut() = Some(LastSeen {
            feature_id: feature_id.to_string(),
            state_hash,
            final_action: final_action.clone(),
        });

        let mut buf = self.recent_hashes.borrow_mut();
        if buf.feature_id != feature_id {
            buf.feature_id = feature_id.to_string();
            buf.hashes.clear();
        }
        buf.hashes.push_front(state_hash);
        if buf.hashes.len() > STATE_HASH_BUFFER_LEN {
            buf.hashes.pop_back();
        }
    }
}

impl<I: GraphInspector> Planner for FeatureReadyOrchestratorPlanner<I> {
    fn plan(&self, ctx: &PlanContext, artifact: &crate::core::drive::ArtifactRef) -> Result<Action, PlanError> {
        let feature_id = artifact
            .short_id
            .strip_prefix("FT-")
            .unwrap_or(&artifact.short_id);
        let default_env_id = ctx.env_or_default("ENV-001");
        self.classify(feature_id, &default_env_id)
    }
}

// Helper functions

fn store_err<E: std::fmt::Display>(e: E) -> PlanError {
    PlanError::Store {
        detail: e.to_string(),
    }
}

fn first_unfinished_dep(deps: &[(String, String)]) -> Option<(&str, &str)> {
    deps.iter()
        .find(|(_, status)| status != "complete")
        .map(|(id, status)| (id.as_str(), status.as_str()))
}

fn stuck_blocked(dep_id: &str, status: &str) -> String {
    format!("blocked: {dep_id} status={status}")
}

fn stuck_preflight(gaps: &[PreflightGap]) -> String {
    let tokens: Vec<String> = gaps.iter().map(gap_to_string).collect();
    format!("preflight: {}", tokens.join(", "))
}

fn gap_to_string(gap: &PreflightGap) -> String {
    match gap {
        PreflightGap::UnacknowledgedAdr(id) => format!("{id} unacknowledged"),
        PreflightGap::UncoveredDomain(name) => format!("{name} domain uncovered"),
    }
}

fn stuck_tc_quality(problem_tc: &str, reason: &str) -> String {
    format!("TC quality: {problem_tc} {reason}")
}

fn stuck_vg_pending(graph_ids: &[String]) -> String {
    format!("VG pending_review: {}", graph_ids.join(", "))
}

fn action_kind(a: &Action) -> &'static str {
    match a {
        Action::Done => "done",
        Action::DispatchVerifier { .. } => "verifier",
        Action::DispatchImplementer { .. } => "implementer",
        Action::DispatchVerifyGraphAuthor { .. } => "vga",
        Action::DispatchTcAuthor { .. } => "tc-author",
        Action::DispatchTcQuality { .. } => "tc-quality",
        Action::DispatchVgQuality { .. } => "vg-quality",
        Action::DispatchSpecAuthor { .. } => "spec-author",
        Action::DispatchAdrAuthor { .. } => "adr-author",
        Action::EscalateVgaToImplementer { .. } => "esc-vga-impl",
        Action::EscalateImplementerToVga { .. } => "esc-impl-vga",
        Action::DispatchCluster { .. } => "cluster",
        Action::Stuck { .. } => "stuck",
    }
}

fn position_from_head(ring: &VecDeque<u64>, needle: u64) -> Option<usize> {
    ring.iter().position(|&h| h == needle).map(|i| i + 1)
}

fn hash_preflight(p: &PreflightStatus, h: &mut DefaultHasher) {
    match p {
        PreflightStatus::Clean => "clean".hash(h),
        PreflightStatus::Warnings { gaps } => {
            "warnings".hash(h);
            for g in gaps {
                match g {
                    PreflightGap::UnacknowledgedAdr(id) => {
                        "adr".hash(h);
                        id.hash(h);
                    }
                    PreflightGap::UncoveredDomain(name) => {
                        "domain".hash(h);
                        name.hash(h);
                    }
                }
            }
        }
    }
}

fn hash_tcs(tcs: &TcsLinkedState, h: &mut DefaultHasher) {
    match tcs {
        TcsLinkedState::NoneLinked => "none".hash(h),
        TcsLinkedState::AllReady => "ready".hash(h),
        TcsLinkedState::SomeUnready { problem_tc, reason } => {
            "unready".hash(h);
            problem_tc.hash(h);
            reason.hash(h);
        }
    }
}

fn hash_vgs(vgs: &CoveringGraphState, h: &mut DefaultHasher) {
    match vgs {
        CoveringGraphState::Missing => "missing".hash(h),
        CoveringGraphState::AcceptedAll => "accepted".hash(h),
        CoveringGraphState::PendingReview { graph_ids } => {
            "pending".hash(h);
            for id in graph_ids {
                id.hash(h);
            }
        }
    }
}
