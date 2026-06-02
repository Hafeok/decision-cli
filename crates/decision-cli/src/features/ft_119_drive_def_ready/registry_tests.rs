//! TC-253 — registry + parsing backstop for FT-119.
//!
//! Pins the wire-level invariants the FT-119 spec relies on:
//!   * `Goal::parse("def-ready")` round-trips through `Display` to
//!     the same string and rejects every near-miss form.
//!   * `planner_for(Feature, DefReady)` returns `Some(_)` so the
//!     driver finds a planner; `planner_for(<other kind>, DefReady)`
//!     returns `None` so the driver can surface
//!     `NoPlannerRegistered` rather than dispatching the wrong
//!     planner.
//!
//! Concrete-type assertion (`FeatureReadyPlanner<ProductionInspector>`)
//! per the TC-253 prose is implicit — the registry constructor is the
//! sole producer of `(Feature, DefReady)` planners, so presence here
//! pins the wiring without an `Any` downcast on `dyn Planner`.

#![cfg(test)]

use crate::core::drive::{ArtifactKind, Goal, PlanContext};
use crate::features::drive::registry::planner_for;

#[test]
fn tc_253_goal_parses_def_ready_form() {
    assert_eq!(Goal::parse("def-ready").unwrap(), Goal::DefReady);
}

#[test]
fn tc_253_goal_display_round_trips_to_wire_string() {
    assert_eq!(format!("{}", Goal::DefReady), "def-ready");
    assert_eq!(Goal::DefReady.as_str(), "def-ready");
}

#[test]
fn tc_253_goal_parse_rejects_no_hyphen_form() {
    assert!(Goal::parse("defready").is_err());
}

#[test]
fn tc_253_goal_parse_rejects_ready_alone() {
    assert!(Goal::parse("ready").is_err());
}

#[test]
fn tc_253_goal_parse_is_case_sensitive() {
    assert!(Goal::parse("Def-Ready").is_err());
    assert!(Goal::parse("DEF-READY").is_err());
}

#[test]
fn tc_253_planner_registered_for_feature_def_ready() {
    let ctx = PlanContext::for_test(std::path::Path::new("/tmp"));
    let planner = planner_for(ArtifactKind::Feature, Goal::DefReady, &ctx);
    assert!(
        planner.is_some(),
        "FeatureReadyPlanner must be registered for (Feature, DefReady) \
         so the driver finds it instead of bailing with NoPlannerRegistered"
    );
}

#[test]
fn tc_253_planner_unregistered_for_non_feature_kinds() {
    let ctx = PlanContext::for_test(std::path::Path::new("/tmp"));
    for kind in [
        ArtifactKind::TestCriterion,
        ArtifactKind::Adr,
        ArtifactKind::VerificationGraph,
    ] {
        assert!(
            planner_for(kind, Goal::DefReady, &ctx).is_none(),
            "no DefReady planner should be registered for {kind:?}"
        );
    }
}
