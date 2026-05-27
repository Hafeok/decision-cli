//! Concrete planner impls. Each `(ArtifactKind, Goal)` combination is
//! one module that exports a `Planner`-impl type. The registry in
//! `super::registry` selects between them.

pub mod feature_ship;

pub use feature_ship::FeatureShipPlanner;
