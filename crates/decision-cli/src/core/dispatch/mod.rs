//! DispatchGroup lifecycle substrate for action-interpretation pairing (ADR-017, FT-021).
//!
//! This module is the slice-level core extension that hosts the
//! `dec:DispatchGroup` Rust shape and its state machine. Per ADR-016,
//! `features/*` modules import these types via [`crate::core::dispatch`];
//! sibling features never reach into each other for the lifecycle
//! primitives that gate `dec implement` completion.

pub mod group;
pub mod lifecycle;
pub mod quads;

pub use group::{DispatchGroup, GroupError};
pub use lifecycle::{DispatchEvent, DispatchStatus, LifecycleError};
pub use quads::{build_group_creation_quads, build_status_transition_quads};
