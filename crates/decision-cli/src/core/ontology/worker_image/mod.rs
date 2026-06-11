//! Store-facing glue for `dec:WorkerImage` (FT-086 / ADR-055).
//!
//! The pure typed shape lives in
//! [`dec_ontology::ontology::worker_image`] (ADR-086); this module keeps
//! write-side SHACL validation (OCI-referrer checks against
//! `core::sbom_referrer`) and the store-backed read APIs, and re-exports
//! both halves.

pub use dec_ontology::ontology::worker_image::*;

pub mod read;
mod shacl;

#[cfg(test)]
mod tests;

pub use read::{
    query_by_capability_tag, query_by_eligibility_status, query_by_id, WorkerImageReadError,
};
pub use shacl::{validate_quads, WorkerImageShaclError, WorkerImageViolation};
