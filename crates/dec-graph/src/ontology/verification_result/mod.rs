//! Store-facing glue for VerificationGraphResult + VerificationStepTrace
//! (FT-097 / ADR-028).
//!
//! The pure typed shapes live in
//! [`dec_ontology::ontology::verification_result`] (ADR-086); this
//! module keeps the store-aware SHACL validator and re-exports both
//! halves.

pub use dec_ontology::ontology::verification_result::*;

pub mod shacl;

pub use shacl::{validate_quads, validate_quads_with_store, ResultShaclError, ResultViolation};
