//! `dec:WorkerImageSubmission` artifact type per FT-087 / ADR-040 / ADR-055.
//!
//! WorkerImageSubmission is the boundary artifact CI posts when a worker
//! author releases a new image version (per the worker-distribution Brief).
//! It carries the claim payload the WorkerCurator role (FT-092) consumes
//! to decide admission. By construction it is *always* boundary-originated:
//! the producer-side world (worker repo, CI) lives outside the orchestration
//! graph, so the Submission is classified as a `dec:InitialRequest`
//! (a `dec:BoundaryArtifact` subclass per FT-071 / ADR-040) and is exempt
//! from per-type motivational provenance enforcement at the SHACL layer.
//!
//! Field-level invariants enforced by [`validate_quads`]:
//!
//! - `submission_id` required, non-empty.
//! - `candidate_registry_ref` required; must contain `@sha256:` digest.
//! - `claimed_capability_tag` cardinality ≥ 1; values non-empty.
//! - `claimed_sbom_ref` required (per FT-091).
//! - `claimed_signature_subject` + `claimed_signature_issuer` required (per FT-089).
//! - `claimed_source_repo_uri`, `claimed_source_commit_hash`, `claimed_build_run_url` required.
//! - `submission_lifecycle_state` ∈ {received, under-review, admitted, rejected}.
//! - `external_origin` required (the BoundaryArtifact escape hatch invariant).
//!
//! Lifecycle transitions are reachable only via Curator session output
//! (FT-092); manual state edits are rejected at the Curator workflow
//! layer. This module only ensures the persisted state literal is one of
//! the declared values.

mod shacl;
pub mod types;

#[cfg(test)]
mod tests;

pub use shacl::{
    validate_quads, WorkerImageSubmissionShaclError, WorkerImageSubmissionViolation,
};
pub use types::{SubmissionLifecycleState, WorkerImageSubmission};
