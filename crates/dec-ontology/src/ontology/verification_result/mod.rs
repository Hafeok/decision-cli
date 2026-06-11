//! VerificationGraphResult + VerificationStepTrace artifact types per
//! FT-097 / ADR-028 — pure typed shapes, quad emission, canonical-Turtle
//! emission, and verdict-derivation rules.
//!
//! The store-aware SHACL validator stays in decision-cli's
//! `core::ontology::verification_result` glue module (ADR-086).

pub mod io;
pub mod quads;
pub mod types;

pub use io::to_canonical_turtle;
pub use types::{EvidenceProjection, StepOutcome, VerificationGraphResult, VerificationStepTrace};
