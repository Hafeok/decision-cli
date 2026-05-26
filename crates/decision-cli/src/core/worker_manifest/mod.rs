//! Worker manifest TOML + release-build outputs → WorkerImageSubmission shape (FT-093).
//!
//! Every worker repo carries a declarative manifest TOML (`worker.toml`)
//! that pins the worker's identity, the capability tags it claims, the
//! roles it is compatible with, the SDK / wire-protocol versions baked
//! into the image, and its runtime entrypoint. The reusable release
//! workflow (see `.github/workflows/release-worker-full.yml`) reads this
//! manifest, builds the OCI image with labels per FT-088, generates the
//! CycloneDX SBOM per FT-091, pushes the image to ghcr.io, signs it
//! keyless via the FT-089 signing primitive workflow, attaches the SBOM
//! as an OCI referrer per ADR-059, then POSTs a `WorkerImageSubmission`
//! to pipeline-cli's submission endpoint (FT-094).
//!
//! This module ships the substrate the workflow uses:
//!
//! - [`WorkerManifest`] — the parsed shape of `worker.toml`. Mirrors the
//!   FT-093 feature_spec's declarative TOML exactly.
//! - [`parse_worker_manifest`] — a minimal, dependency-free TOML reader
//!   covering precisely the subset the manifest uses (top-level tables,
//!   string scalars, string arrays). Refuses anything outside the shape
//!   so a manifest typo surfaces as a structured violation, not silent
//!   acceptance.
//! - [`ReleaseBuildOutputs`] — the values the workflow produces during
//!   the build / push / sign cycle that must be threaded onto the
//!   Submission alongside the manifest claims.
//! - [`assemble_submission_payload`] — combines `(WorkerManifest +
//!   ReleaseBuildOutputs)` into a `SubmissionPayload` ready for POST to
//!   `/submissions`. The lifting rule is fixed here so the workflow has
//!   exactly one source of truth for which manifest field maps to which
//!   Submission field.
//!
//! Substrate-only: no I/O, no graph access, no HTTP. Callers (the
//! release workflow's `submit` step, integration tests, the dogfood
//! script) wire the pieces together.

mod assemble;
mod parse;
mod types;

#[cfg(test)]
mod tests;

pub use assemble::{
    assemble_submission_payload, AssembleSubmissionError, ReleaseBuildOutputs,
};
pub use parse::{parse_worker_manifest, ManifestParseError};
pub use types::{
    Capabilities, RuntimeKind, RuntimeSpec, WorkerManifest, WorkerSection,
    DEFAULT_WIRE_PROTOCOL_VERSION,
};
