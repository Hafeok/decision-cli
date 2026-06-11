//! Store-facing glue for the Feedback artifact type (FT-026 / ADR-022).
//!
//! The pure schema substrate — typed shape, quad emission, SHACL,
//! class vocabulary (FT-028), lifecycle state machine (FT-027) — lives
//! in [`dec_ontology::ontology::feedback`] (ADR-086); this module keeps
//! the store-backed read API, routing (FT-029), transition application,
//! and address walking, and re-exports both halves.

pub use dec_ontology::ontology::feedback::*;

pub mod address_walk;
pub mod read;
pub mod routing;
pub mod supersede_misrouted;
pub mod transition;

#[cfg(test)]
mod tests;

pub use read::{get, list_by_class, list_by_target, list_open, FeedbackReadError};
pub use transition::{apply, read_prior_state, ApplyError, Evidence};
