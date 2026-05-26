//! VerificationGraphResult + VerificationStepTrace artifact types per
//! FT-097 / ADR-028.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-098 runner, FT-099 CLI, FT-100 subscription) imports from this
//! module — no consumer imports from sibling features.
//!
//! Scope of FT-097 is intentionally the typed shapes + SHACL validation
//! + the per-graph and multi-graph verdict-derivation rules. Persistence
//! routing lives in FT-098 (the runner is the only writer); CLI
//! rendering lives in FT-099.

pub mod io;
pub mod quads;
pub mod shacl;
pub mod types;

pub use io::to_canonical_turtle;
pub use shacl::{
    validate_quads, validate_quads_with_store, ResultShaclError, ResultViolation,
};
pub use types::{
    EvidenceProjection, StepOutcome, VerificationGraphResult, VerificationStepTrace,
};
