//! Role catalog backed by the orchestration graph (FT-019 / FT-030).
//!
//! Slice 1 shipped a single hardcoded `implementer` role; FT-019 added
//! the verifier as the first graph-resident entry; FT-030 generalises
//! the catalog so every role (implementer + verifier in Phase A) is a
//! first-class artifact carrying a `dec:Authority` declaration per
//! ADR-027. Per the slice-level SDP convention in `CLAUDE.md`, this
//! module lives under `core/`: every feature reads the catalog through
//! here, never through a sibling feature.
//!
//! There is no caching; lookups run a small SPARQL query each time.

pub mod authority;
pub mod role;
pub mod seeds;

pub use authority::{
    Authority, EscalationHint, AUTHORITY_CLASS_IRI, AUTHORITY_PRED_IRI, ESCALATE_CATEGORY_IRI,
    ESCALATE_CLASS_IRI, ESCALATE_TARGET_ROLE_IRI, ESCALATE_VIA_IRI, IMPLEMENTER_AUTHORITY_IRI,
    MAY_DECIDE_IRI, MUST_ESCALATE_IRI, RATIONALE_IRI, VERIFIER_AUTHORITY_IRI,
};
pub use role::{
    list_all, lookup, Role, ROLE_CLASS_IRI, ROLE_ID_IRI, ROLE_INPUT_TYPE_IRI,
    ROLE_MODEL_BINDING_IRI, ROLE_OUTPUT_TYPE_IRI, VERIFICATION_VERDICT_IRI,
};
pub use seeds::{
    implementer_seed_quads, verifier_seed_quads, IMPLEMENTER_ROLE_ID, IMPLEMENTER_ROLE_IRI,
    VERIFIER_ROLE_ID, VERIFIER_ROLE_IRI,
};

/// Raw Turtle bytes for the verifier role seed — embedded via
/// `include_str!` per ADR-007.
pub const VERIFIER_SEED_TTL: &str = include_str!("seeds/verifier.ttl");

/// Raw Turtle bytes for the implementer role seed (FT-030).
pub const IMPLEMENTER_SEED_TTL: &str = include_str!("seeds/implementer.ttl");

/// Raw Turtle bytes for the verifier authority seed (FT-030).
pub const VERIFIER_AUTHORITY_TTL: &str = include_str!("seeds/verifier-authority.ttl");

/// Raw Turtle bytes for the implementer authority seed (FT-030).
pub const IMPLEMENTER_AUTHORITY_TTL: &str = include_str!("seeds/implementer-authority.ttl");

/// FT-030 read API: list every role in the catalog (implementer +
/// verifier in Phase A). Order is `dec:roleId` ascending.
///
/// Thin wrapper over [`role::list_all`] so external callers can rely on
/// the canonical `list_roles` name from the FT-030 spec.
pub fn list_roles(store: &oxigraph::store::Store) -> anyhow::Result<Vec<Role>> {
    role::list_all(store)
}

#[cfg(test)]
mod tests;
