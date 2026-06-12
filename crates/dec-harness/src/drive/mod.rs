//! Pluggable artifact + goal orchestration substrate (FT-110).
//!
//! The driver is a tight loop:
//!   1. Parse the artifact reference.
//!   2. Look up the [`Planner`] for `(kind, goal)`.
//!   3. Loop: call `planner.plan(ctx, artifact)`, execute the returned
//!      [`Action`], repeat up to `max_iter`.
//!   4. Terminate when the planner returns [`Action::Done`] or
//!      [`Action::Stuck`].
//!
//! Substrate lives in `core::drive`; concrete planners live in
//! `features::drive::planners` so adding a new (kind, goal) combination
//! is a single trait impl plus a registry entry. The driver itself
//! does not change.
//!
//! Slice-1 hardcodes the registry in Rust. The trait shape is forward-
//! compatible with graph-resident `dec:DispatchRule` artifacts — when
//! we get there, `planner_for` will look up rules in the orchestration
//! store rather than matching on an enum.

pub mod action;
pub mod artifact;
pub mod context;
pub mod goal;
pub mod planner;

pub use action::Action;
pub use artifact::{ArtifactKind, ArtifactRef};
pub use context::PlanContext;
pub use goal::Goal;
pub use planner::{PlanError, Planner};
