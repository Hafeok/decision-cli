//! Session escalation telemetry types (FT-057) — the in-memory
//! [`SessionRecord`] value and its RDF serialisation.
//!
//! The store-aware SHACL validator (`validate_quads`,
//! `validate_quads_with_store`) stays in decision-cli's
//! `core::ontology::session_record` glue module (ADR-086).

pub mod types;

pub use types::{SessionRecord, SessionRecordRef};
