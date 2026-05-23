//! DispatchGroup lifecycle substrate for action-interpretation pairing (ADR-017, FT-021).
//!
//! This module is the slice-level core extension that hosts the
//! `dec:DispatchGroup` Rust shape and its state machine. Per ADR-016,
//! `features/*` modules import these types via [`crate::core::dispatch`];
//! sibling features never reach into each other for the lifecycle
//! primitives that gate `dec implement` completion.

pub mod capability_resolver;
pub mod group;
pub mod lifecycle;
pub mod pause;
pub mod quads;

pub use capability_resolver::{resolve_default_capability, ResolvedCapability, ResolverError};
pub use group::{DispatchGroup, GroupError};
pub use lifecycle::{DispatchEvent, DispatchStatus, LifecycleError};
pub use pause::{
    list_blocked_by, list_paused_groups_for_feedback, pause_on_feedback, resume_check, PauseError,
    ResumeError, ResumeOutcome,
};
pub use quads::{build_blocked_by_quad, build_group_creation_quads, build_status_transition_quads};
