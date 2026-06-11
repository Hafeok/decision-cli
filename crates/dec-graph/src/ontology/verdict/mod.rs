//! Store-facing glue for VerificationVerdict (FT-020 / ADR-018).
//!
//! The pure typed shapes and quad-level SHACL validation live in
//! [`dec_ontology::ontology::verdict`] (ADR-086); this module keeps the
//! store-backed read APIs and re-exports both halves.

pub use dec_ontology::ontology::verdict::*;

mod read;

pub use read::{latest_verdict_for_dispatch, list_verdicts_for_dispatch, ReadError};
