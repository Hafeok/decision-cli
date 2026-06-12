//! `dec:ApplicationContract` + `dec:Convention` artifact types per
//! FT-148 / ADR-082 §3 — pure schema substrate: typed shapes, inline
//! Convention sub-resources, quad emission/parsing, and the SHACL
//! mirror enforced at the dec-graph chokepoint.
//!
//! Cluster-authored under FT-141's `add-artifact-type` TaskType
//! (run 17, parallel dispatch); operator-promoted with corrections per
//! the cluster pattern's inspect-before-promote step.

mod emitter;
mod parser;
pub mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use parser::{quads_to_application_contract, ContractParseError};
pub use shacl::{validate_quads, ContractShaclError, ContractViolation};
pub use types::{application_contract_iri, ApplicationContract, Convention};
