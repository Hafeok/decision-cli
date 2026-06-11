//! VerificationVerdict artifact type per FT-020 / ADR-018 — pure typed
//! shapes and quad-level SHACL validation.
//!
//! Store-backed read APIs (`latest_verdict_for_dispatch` et al.) live in
//! decision-cli's `core::ontology::verdict` glue module (ADR-086).

pub mod shacl;
pub mod types;

#[cfg(test)]
mod tests;

pub use shacl::{validate_quads, VerdictShaclError, VerdictViolation, RATIONALE_MIN_LEN};
pub use types::{Verdict, VerdictArtifact};
