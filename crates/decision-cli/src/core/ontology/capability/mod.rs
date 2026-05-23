//! `dec:Capability` artifact type per FT-054 / ADR-033.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-055 RoleBinding, FT-058 catalog bootstrap, FT-061 dispatcher
//! resolution) imports from this module — no consumer imports from
//! sibling features.
//!
//! Scope of FT-054 is intentionally the schema substrate: the in-memory
//! shape (`Capability`, `Endpoint`, `CapabilityStatus`, `CostCurrency`),
//! RDF serialisation (`to_quads`), the ADR-033 SHACL shape
//! (`validate_quads`), and the read API for the active capability with a
//! given id (`query_active_by_id`).
//!
//! Catalog content (the seed YAML driving bootstrap), role bindings,
//! dispatcher resolution, and worker-side consumption are out of scope
//! and land in [FT-055..FT-065].

pub mod lookup;
pub mod read;
pub mod shacl;
mod take;
pub mod types;

#[cfg(test)]
mod tests;

pub use lookup::query_by_iri;
pub use read::{query_active_by_id, CapabilityReadError};
pub use shacl::{validate_quads, CapabilityShaclError, CapabilityViolation};
pub use types::{Capability, CapabilityStatus, CostCurrency, Endpoint};
