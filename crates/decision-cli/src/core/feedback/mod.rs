//! Feedback artifact type per FT-026 / ADR-022.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-027 lifecycle, FT-028 vocabulary, FT-029 routing, FT-031 worker
//! SDK, FT-032 dispatch interaction, FT-033 CLI) imports from this
//! module — no consumer imports from sibling features.
//!
//! Scope of FT-026 is intentionally the schema substrate: the in-memory
//! shape, RDF (de)serialisation, the ADR-022 SHACL shape (required
//! fields and cardinality), and a small read API. The richer rules —
//! class controlled vocabulary (FT-028), lifecycle state machine
//! (FT-027), routing (FT-029) — are owned by later features and layer
//! on top of the contract defined here.

pub mod artifact;
pub mod read;
pub mod shacl;

#[cfg(test)]
mod tests;

pub use artifact::{Feedback, Severity};
pub use read::{get, list_by_class, list_by_target, list_open, FeedbackReadError};
pub use shacl::{validate_quads, FeedbackShaclError, FeedbackViolation};
