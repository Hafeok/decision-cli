//! Store-facing glue for `dec:Capability` (FT-054 / ADR-033).
//!
//! The pure schema substrate (types, quad emission, SHACL) lives in
//! [`dec_ontology::ontology::capability`] (ADR-086); this module keeps
//! the store-backed read APIs and re-exports both halves.

pub use dec_ontology::ontology::capability::*;

pub mod lookup;
pub mod read;
mod take;

pub use lookup::query_by_iri;
pub use read::{query_active_by_id, CapabilityReadError};
