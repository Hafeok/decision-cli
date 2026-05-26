//! Catalog artifact types per FT-101 / ADR-066.
//!
//! Three closely-related artifact types that together form the substrate
//! the [`crate::core::bundle`] assembler consumes when assembling a
//! verify-graph-author bundle (FT-102):
//!
//! - [`CapabilityReference`] — structured reference for a single `dec`
//!   subcommand the worker may invoke from a `shell-command` step.
//! - [`OntologyDescription`] — declarative description of the dec
//!   namespace, its classes, and canonical predicates.
//! - [`ExemplarGraph`] — reference to a known-good `dec:VerificationGraph`
//!   curated for an env's `safety_class`.
//!
//! Per ADR-066 the bundle assembler reads these via SPARQL `CONSTRUCT`
//! rather than carrying hardcoded literals, so the catalog is dec's
//! self-description as queryable graph state.
//!
//! Scope of this module: in-memory types, RDF quad serialisation, and
//! the store-aware SHACL validator wired into the [`StreamWriter`]
//! chokepoint. Authoring CLI verbs (`dec catalog capability new`, etc.)
//! live under `features::catalog_*` and dispatch through this module.

pub mod shacl;
pub mod types;

pub use shacl::{validate_quads, validate_quads_with_store, CatalogShaclError, CatalogViolation};
pub use types::{CapabilityReference, ExemplarGraph, OntologyDescription, SafetyClassTag};
