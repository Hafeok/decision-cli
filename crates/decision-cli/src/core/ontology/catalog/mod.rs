//! Store-facing glue for the FT-101 / ADR-066 catalog artifact types.
//!
//! The pure typed shapes live in [`dec_ontology::ontology::catalog`]
//! (ADR-086); this module keeps the store-aware SHACL validator wired
//! into the `StreamWriter` chokepoint and re-exports both halves.

pub use dec_ontology::ontology::catalog::*;

pub mod shacl;

pub use shacl::{validate_quads, validate_quads_with_store, CatalogShaclError, CatalogViolation};
