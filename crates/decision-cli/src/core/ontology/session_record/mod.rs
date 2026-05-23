//! Session escalation edges and cache-aware token breakdown (FT-057).
//!
//! Extends `dec:Session` (a.k.a. `dec:SessionRecord`) with bidirectional
//! escalation chain membership plus the four token-count fields needed
//! for cache-aware cost rollups on Anthropic prompt-cached dispatches
//! (ADR-034 / ADR-037 / [FT-065](FT-065)).
//!
//! This module surfaces:
//!
//! - [`SessionRecord`] — in-memory value carrying the new fields.
//! - [`SessionRecord::to_quads`] / [`SessionRecord::escalated_to_quad`] —
//!   RDF serialisation; the latter is appended on the *prior* session in
//!   the same transaction that writes the new escalated session, so the
//!   bidirectional invariant holds at every write boundary.
//! - [`validate_quads`] — Rust-side SHACL validator the `StreamWriter`
//!   invokes before persisting. Implements the three FT-057 `sh:sparql`
//!   constraints: bidirectional escalation, escalation_reason iff
//!   escalated_from, and Scaleway-no-cache.

mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use shacl::{
    validate_quads, validate_quads_with_store, SessionRecordShaclError, SessionRecordViolation,
};
pub use types::{SessionRecord, SessionRecordRef};
