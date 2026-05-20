//! VerificationVerdict artifact type per FT-020 / ADR-018.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-021, FT-022, FT-023, FT-024) imports from this module — no
//! consumer imports from sibling features.

mod read;
mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use read::{list_verdicts_for_dispatch, latest_verdict_for_dispatch, ReadError};
pub use shacl::{validate_quads, VerdictShaclError, VerdictViolation, RATIONALE_MIN_LEN};
pub use types::{Verdict, VerdictArtifact};
