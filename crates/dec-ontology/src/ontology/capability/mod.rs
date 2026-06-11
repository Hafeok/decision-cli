//! `dec:Capability` artifact type per FT-054 / ADR-033 — pure schema
//! substrate: the in-memory shape, RDF serialisation (`to_quads`), and
//! the ADR-033 SHACL shape (`validate_quads`).
//!
//! Store-backed read APIs (`query_active_by_id`, `query_by_iri`) live in
//! decision-cli's `core::ontology::capability` glue module (ADR-086).

pub mod shacl;
pub mod types;

#[cfg(test)]
mod tests;

pub use shacl::{validate_quads, CapabilityShaclError, CapabilityViolation};
pub use types::{Capability, CapabilityStatus, CostCurrency, Endpoint};
