//! Mechanical-provenance fragment — materialiser + validator (FT-069 / ADR-038).
//!
//! GraphWriter materialises the universal mechanical block on every
//! artifact it commits, from the session record handed in by the
//! harness's session-completion handler. Workers never author these
//! triples themselves (`docs/ddd/Implementing_DDD.md` §7). This module
//! exposes:
//!
//! - [`SessionAttribution`] — the harness-supplied record carrying the
//!   producing Session IRI, its attributed Agent IRIs, and the wall-clock
//!   timestamp the GraphWriter commit assigned.
//! - [`materialise_quads`] — appends `prov:wasGeneratedBy`,
//!   `prov:wasAttributedTo`, `prov:generatedAtTime` triples to a quad
//!   bundle for a specific artifact subject.
//! - [`validate_quads`] — Rust-side SHACL validator mirroring the
//!   universal `:MechanicalProvenanceShape` (`shapes/mechanical-provenance.ttl`).
//!   Invoked alongside other shape validators at the StreamWriter
//!   chokepoint when slice-1 wiring lands per ADR-041; consumed
//!   directly by FT-069 acceptance tests today.
//!
//! The validator runs against any artifact subject whose `rdf:type` is
//! enumerated in [`covered_classes`]. The slice-1 coverage is empty —
//! per the FT-069 spec, the per-type composition (FT-072) wires the
//! universal fragment into every artifact-type shape; until then this
//! module only validates artifacts the caller explicitly opts in (used
//! by FT-069's tests).

mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use shacl::{
    validate_quads, validate_subject, MechanicalProvenanceShaclError,
    MechanicalProvenanceViolation,
};
pub use types::{materialise_quads, SessionAttribution};
