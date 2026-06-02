//! `dec drive def-ready` — Definition-of-Ready planner (FT-119).
//!
//! Slice-1 surface: a pure-classification planner that walks the
//! seven readiness dimensions described in the FT-119 spec and
//! returns either `Done`, `DispatchVerifyGraphAuthor`, or a
//! `Stuck { reason }` whose text cites the offending artifact id
//! verbatim (per TC-255).
//!
//! The planner is observe-only — it never writes to the product
//! graph or the orchestration store. The only worker it dispatches
//! is the existing verify-graph-author (FT-048/FT-067), invoked
//! when `vgs_cover` is false and every other dimension is satisfied.
//! Every other gap is classified as `Stuck` because it requires
//! human-authored content (a spec body, a TC, an ADR
//! acknowledgement); the FT-131 successor (ADR-076) is what closes
//! those arms.
//!
//! This commit lands the classification core: planner.rs carries
//! the pure-function classifier + the stub-driven TC-254 backstop.
//! CLI surface, sweep lift, and production inspector wiring land in
//! follow-up commits per the FT-119 implementation arc.

pub mod planner;

pub use planner::FeatureReadyPlanner;
