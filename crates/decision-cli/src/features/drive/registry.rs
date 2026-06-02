//! Planner registry. Slice-1 keyed on the `(ArtifactKind, Goal)` tuple
//! via a `match`; future work may swap this for a graph-resident
//! `dec:DispatchRule` lookup, but the `planner_for` shape stays.

use crate::core::drive::{ArtifactKind, Goal, PlanContext, Planner};

use super::inspect::ProductionInspector;
use super::planners::FeatureShipPlanner;
use crate::features::ft_119_drive_def_ready::FeatureReadyPlanner;

/// Look up the planner for `(kind, goal)`. Returns `None` when no
/// planner is registered — the driver translates that into
/// `DriveError::NoPlannerRegistered`.
///
/// `ctx_ref` is borrowed only long enough for the inspector to clone
/// the planning state; the planner trait object doesn't hold it.
pub fn planner_for<'a>(
    kind: ArtifactKind,
    goal: Goal,
    ctx_ref: &'a PlanContext,
) -> Option<Box<dyn Planner + 'a>> {
    match (kind, goal) {
        (ArtifactKind::Feature, Goal::Ship) => {
            let inspector = ProductionInspector::new(ctx_ref);
            Some(Box::new(FeatureShipPlanner::new(inspector)))
        }
        (ArtifactKind::Feature, Goal::DefReady) => {
            // The FT-119 planner shares `ProductionInspector` with
            // the ship planner — the FT-119-specific dimensions
            // (preflight, deps, spec, tcs, vgs) currently fall
            // through to the trait's default impls (permissive
            // "ready" answers). Production overrides for those
            // dimensions land in a follow-up commit; until then the
            // registry wiring is structural so `dec drive def-ready`
            // dispatches and the test seams (TC-256/TC-257) can
            // inject their own inspectors.
            let inspector = ProductionInspector::new(ctx_ref);
            Some(Box::new(FeatureReadyPlanner::new(inspector)))
        }
        _ => None,
    }
}
