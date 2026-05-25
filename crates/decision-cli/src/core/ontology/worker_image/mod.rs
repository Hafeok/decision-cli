//! `dec:WorkerImage` artifact type per FT-086 / ADR-055.
//!
//! Mirrors the structural shape of [`super::capability`] for the worker
//! registry catalog: an identity-versioned artifact, a fixed schema (the
//! per-type SHACL composition shape already lives at
//! `assets/shapes/worker-image.ttl`), an in-memory Rust shape, RDF
//! serialisation, write-side SHACL validation of the field-level
//! invariants ADR-055 requires, and read APIs for catalog discovery.
//!
//! Field-level invariants enforced by [`validate_quads`]:
//!
//! - `id`, `name`, `version`, `registry_ref` — required strings.
//! - `version` must be semver-shaped (`major.minor.patch`).
//! - `registry_ref` must contain a `@sha256:` digest (OCI digest pin).
//! - `eligibility_status` ∈ {`qualified`, `candidate`, `deprecated`, `pulled`}.
//! - `capability_tag` cardinality ≥ 1.
//! - `signed_by_subject` + `signed_by_issuer` required (sigstore identity).
//! - `sbom_ref` required.
//! - provenance: `source_repo_uri`, `source_commit_hash`, `build_run_url` required.

pub mod read;
mod shacl;
pub mod types;

#[cfg(test)]
mod tests;

pub use read::{
    query_by_capability_tag, query_by_eligibility_status, query_by_id, WorkerImageReadError,
};
pub use shacl::{validate_quads, WorkerImageShaclError, WorkerImageViolation};
pub use types::{EligibilityStatus, WorkerImage};
