//! Store-facing glue for session escalation telemetry (FT-057).
//!
//! The pure [`SessionRecord`] value and its RDF serialisation live in
//! [`dec_ontology::ontology::session_record`] (ADR-086); this module
//! keeps the store-aware SHACL validator the `StreamWriter` invokes and
//! re-exports both halves.

pub use dec_ontology::ontology::session_record::*;

mod shacl;

#[cfg(test)]
mod tests;

pub use shacl::{
    validate_quads, validate_quads_with_store, SessionRecordShaclError, SessionRecordViolation,
};
