//! `dec:Archetype` artifact type per FT-147 / ADR-082 — pure schema
//! substrate: the in-memory shape, quad emission, quad parsing, and the
//! ADR-084 §1 SHACL constraints (`validate_quads`, E102).
//!
//! An archetype is the unit of cross-customer reuse: a recurring kind of
//! system owning two parallel contracts (application + infrastructure),
//! a TaskType set split by family, and audits at three scopes. Store
//! integration (the chokepoint registration, the E020 promotion gate,
//! the W104 readiness walk) lives above this crate in dec-graph.
//!
//! Cluster-authored under FT-141's `add-artifact-type` TaskType;
//! operator-corrected at promotion (canonical `decision-cli.dev/ns#`
//! vocabulary, compiling parser/emitter, reusable Provenance).

mod emitter;
mod parser;
pub mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use parser::{quads_to_archetype, ArchetypeParseError};
pub use shacl::{validate_quads, ArchetypeShaclError, ArchetypeViolation, E102_CODE};
pub use types::{archetype_iri, Archetype, ArchetypeEvidence, ArchetypeStatus, Variance};
