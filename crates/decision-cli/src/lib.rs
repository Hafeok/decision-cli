//! decision-cli — orchestration crate for Decision-Driven Design.
//!
//! Drives product-cli through the engineering process by dispatching
//! LLM-backed roles, recording sessions in the graph-native event
//! substrate from `oxi-events`. ADR-016 governs the internal layering:
//! `core` is the stable substrate, `features` are vertical slices, and
//! cross-feature imports are a compile error.

#![deny(clippy::unwrap_used)]

pub mod core;
pub mod features;

// Backwards-compatible top-level surface — external integration tests
// import from `decision_cli::{init, implement, …}`, so we re-export the
// feature modules at the crate root.
pub use crate::core::bundled;
pub use crate::core::scope;
pub use crate::core::vocab;
pub use crate::core::worker;
pub use features::events;
pub use features::finalize;
pub use features::health;
pub use features::implement;
pub use features::init;
pub use features::session_inspect;

// Stable type re-exports for slice 1 callers.
pub use crate::core::{ActiveScope, OntologyError, OntologyHandle, ScopeError, StreamWriter,
                      ONTOLOGY_VERSION};
pub use features::finalize::{finalize_run, FinalizeError, FinalizeInput, FinalizeOutcome};
pub use features::health::{check as health_check, HealthReport};
pub use features::implement::{ImplementArgs, ImplementOutcome};
