//! FT-131 — Definition-of-Ready planner becomes readiness orchestrator.
//!
//! Extends FT-119 with the full upstream chain (spec → ADR → TC → VG),
//! each authored and quality-judged per ADR-073.

pub mod planner;

pub use planner::FeatureReadyOrchestratorPlanner;
