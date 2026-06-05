//! Graph read-side helpers for downstream consumers — metrics, CLI
//! surfaces, the dispatcher (FT-057 §"SPARQL helpers" et al.).
//!
//! FT-073 / ADR-041: the [`shacl`] and [`violation`] submodules expose
//! the dual-provenance validator and the `ProvenanceViolation` shape
//! emitted on rejection. They are consumed by `StreamWriter::commit`'s
//! validation stage and surfaced to callers via the `provenance violation`
//! error prefix on `WriteError`.

pub mod cluster_session;
pub mod session;
pub mod shacl;
pub mod violation;
pub mod writer;

pub use shacl::{ValidationReport, Validator, ValidatorError};
pub use violation::{
    violation_feedback_quads, ProvenanceViolation, ViolationKind, DEFAULT_TARGET_ROLE,
    PROVENANCE_VIOLATION_CLASS, VIOLATION_SEVERITY,
};
pub use writer::{validate_and_commit, validate_only, ProvenanceRejection, ValidateAndCommitOptions};
