//! `dec:WorkerImage` artifact type per FT-086 / ADR-055 — pure typed
//! shape and RDF serialisation.
//!
//! Write-side SHACL validation (OCI-referrer checks) and store-backed
//! read APIs stay in decision-cli's `core::ontology::worker_image` glue
//! module (ADR-086).

pub mod types;

pub use types::{EligibilityStatus, WorkerImage};
