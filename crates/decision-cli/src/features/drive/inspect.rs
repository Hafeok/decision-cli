//! State-inspection trait used by planners.
//!
//! Planners depend on a small trait surface rather than on the
//! `PlanContext` directly, so unit tests can supply a stub that pins
//! the (verdict, open-feedback counts) state without seeding the live
//! orchestration store. Production code wires the trait against the
//! real readers (verify-feature aggregator, FT-108 defect loader).

use std::path::Path;

use crate::core::drive::PlanContext;

/// Aggregate verdict for a feature, as reported by `dec verify
/// feature FT-XXX` across every covering graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureVerdict {
    /// Every covering graph approved.
    Approved,
    /// At least one covering graph emitted `rejected` (evidence
    /// regression).
    Rejected,
    /// At least one covering graph emitted `amendment-required` (graph
    /// setup-failure).
    AmendmentRequired,
    /// No verify run on record for this feature.
    NeverRun,
}

/// Trait planners depend on. Production impl lives in
/// [`ProductionInspector`]; tests build their own.
pub trait GraphInspector {
    /// Aggregate verify verdict for the named feature.
    fn aggregate_verdict_for_feature(
        &self,
        feature_id: &str,
    ) -> Result<FeatureVerdict, InspectError>;

    /// Count of open (`produced | routed | received`) defect feedback
    /// entries targeting `role_id` whose source artifact is one of the
    /// feature's TCs.
    fn open_defect_feedback_count(
        &self,
        feature_id: &str,
        role_id: &str,
    ) -> Result<usize, InspectError>;
}

/// Inspector errors. Kept generic so planners can propagate
/// without knowing the underlying read API.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InspectError {
    /// Underlying store read failed.
    #[error("inspector: {detail}")]
    Store {
        /// Human-readable detail.
        detail: String,
    },
}

/// Map a real verify-feature response to a `FeatureVerdict`.
fn verdict_from(resp: &crate::features::verify_feature::FeatureVerifyResponse) -> FeatureVerdict {
    let Some(block) = resp.aggregate.as_ref() else {
        return FeatureVerdict::NeverRun;
    };
    match block.verdict.as_str() {
        "approved" => FeatureVerdict::Approved,
        "amendment-required" => FeatureVerdict::AmendmentRequired,
        "rejected" => FeatureVerdict::Rejected,
        _ => FeatureVerdict::NeverRun,
    }
}

/// Production inspector that reads against the real orchestration
/// store via the existing FT-099 / FT-108 surfaces. Constructed once
/// per driver invocation; cheap to hold across iterations.
pub struct ProductionInspector<'a> {
    ctx: &'a PlanContext,
}

impl<'a> ProductionInspector<'a> {
    /// Wire against a planning context.
    #[must_use]
    pub fn new(ctx: &'a PlanContext) -> Self {
        Self { ctx }
    }

    fn workdir(&self) -> &Path {
        &self.ctx.workdir
    }

    fn product_root(&self) -> &Path {
        &self.ctx.product_root
    }
}

impl<'a> GraphInspector for ProductionInspector<'a> {
    fn aggregate_verdict_for_feature(
        &self,
        feature_id: &str,
    ) -> Result<FeatureVerdict, InspectError> {
        use crate::features::verify_feature::{run as run_verify_feature, FeatureVerifyRequest};
        let req = FeatureVerifyRequest {
            feature_id: feature_id.to_string(),
            environment_id: self.ctx.env_override.clone(),
            no_feedback: true,
            include_stale: false,
            dry_run: true,
            workdir: Some(self.workdir().to_path_buf()),
        };
        let outcome = run_verify_feature(&req).map_err(|e| InspectError::Store {
            detail: format!("verify-feature dry-run read: {e}"),
        })?;
        // Dry-run mode doesn't actually execute; we use it as a cheap
        // shape probe. The aggregate verdict comes from a non-dry-run
        // pass, so we run a second time without dry-run when we need
        // the real verdict.
        if outcome.dry_run {
            // Fall through to a real verify pass — limited to one
            // environment when the operator pinned one so the read is
            // bounded.
            let real_req = FeatureVerifyRequest {
                dry_run: false,
                ..req
            };
            let real = run_verify_feature(&real_req).map_err(|e| InspectError::Store {
                detail: format!("verify-feature aggregate read: {e}"),
            })?;
            return Ok(verdict_from(&real));
        }
        Ok(verdict_from(&outcome))
    }

    fn open_defect_feedback_count(
        &self,
        feature_id: &str,
        role_id: &str,
    ) -> Result<usize, InspectError> {
        use crate::core::feedback::read::list_by_class;
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use crate::core::verify::coverage::feature_resolver::{
            resolve_feature_tcs_short, tc_iri_for,
        };

        let tc_shorts = resolve_feature_tcs_short(self.product_root(), feature_id).map_err(|e| {
            InspectError::Store {
                detail: format!("resolve TCs for {feature_id}: {e}"),
            }
        })?;
        if tc_shorts.is_empty() {
            return Ok(0);
        }
        let tc_iris: std::collections::HashSet<String> =
            tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let defects = list_by_class(&store, "defect").map_err(|e| InspectError::Store {
            detail: format!("listing defect feedback: {e}"),
        })?;
        let count = defects
            .into_iter()
            .filter(|fb| fb.target_role == role_id)
            .filter(|fb| {
                matches!(fb.lifecycle_state.as_str(), "produced" | "routed" | "received")
            })
            .filter(|fb| {
                fb.source_artifact
                    .as_ref()
                    .map(|src| tc_iris.contains(src.as_str()))
                    .unwrap_or(false)
            })
            .count();
        Ok(count)
    }
}
