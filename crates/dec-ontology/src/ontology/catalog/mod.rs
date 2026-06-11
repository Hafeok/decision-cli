//! Catalog artifact types per FT-101 / ADR-066 — pure typed shapes
//! with RDF quad serialisation.
//!
//! The store-aware SHACL validator wired into the `StreamWriter`
//! chokepoint stays in decision-cli's `core::ontology::catalog` glue
//! module (ADR-086).

pub mod types;

pub use types::{CapabilityReference, ExemplarGraph, OntologyDescription, SafetyClassTag};
