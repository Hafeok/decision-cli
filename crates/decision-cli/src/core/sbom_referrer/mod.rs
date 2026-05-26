//! Syntactic validation for SBOM OCI referrer descriptor URIs (FT-091 / ADR-059).
//!
//! Workers carry their CycloneDX SBOM as an OCI referrer (per OCI v1.1) of the
//! published image. The `sbom_ref` field on [`crate::core::ontology::worker_image::WorkerImage`]
//! and `claimed_sbom_ref` on
//! [`crate::core::ontology::worker_image_submission::WorkerImageSubmission`]
//! is the digest-pinned OCI reference the registry resolves through its
//! referrers API to produce the SBOM bytes — exactly what
//! `cosign download sbom <image-ref>` consumes.
//!
//! The descriptor URI shape is:
//!
//! ```text
//!   <registry>/<repository>@sha256:<64 lowercase hex digits>
//! ```
//!
//! Examples that ADMIT:
//! - `ghcr.io/example/worker@sha256:deadbeef…deadbeef` (64 hex)
//! - `registry.example.com:5000/team/worker@sha256:abc123…abc123`
//!
//! Examples that REJECT:
//! - `ghcr.io/example/worker:latest` — no `@sha256:` digest (mutable tag)
//! - `ghcr.io/example/worker@sha256:cafe` — digest too short
//! - `@sha256:deadbeef…deadbeef` — no registry/repository prefix
//! - `ghcr.io/EXAMPLE/worker@sha256:deadbeef…deadbeef` — uppercase repo component
//! - the empty string — missing entirely
//!
//! The validator is intentionally substrate-only: it consumes a `&str`,
//! does not call the registry, does not fetch bytes, and does not talk
//! to the graph. The caller (SHACL validator, curator-bundle assembler)
//! decides what to do with the verdict.

mod assemble;
mod uri;
mod validate;

#[cfg(test)]
mod tests;

pub use assemble::{
    assemble_curator_submission_bundle, CuratorSubmissionBundle, CuratorSubmissionBundleError,
};
pub use uri::{OciReferrerUri, SHA256_HEX_DIGITS};
pub use validate::{
    validate_oci_referrer_uri, OciReferrerUriValidationError, OciReferrerUriViolation,
};
