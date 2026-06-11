//! Store-facing glue for `dec:Archetype` (FT-147 / ADR-082).
//!
//! The pure schema substrate (types, quad emission, parsing, E102 SHACL)
//! lives in [`dec_ontology::ontology::archetype`]; this module keeps the
//! store-coupled pieces: the typed write path through the SHACL-enforced
//! chokepoint, the ADR-085 §6 status-promotion gate (E020), and the
//! W104 promotion-readiness walk.

pub use dec_ontology::ontology::archetype::*;

mod promotion;
mod write;

#[cfg(test)]
mod tests;

pub use promotion::{
    promotion_ready_candidates, validate_status_transition_with_store, PromotionReadiness,
    StatusGateError, StatusWriteAuthority, E020_CODE, W104_CODE,
};
pub use write::write_archetype;
