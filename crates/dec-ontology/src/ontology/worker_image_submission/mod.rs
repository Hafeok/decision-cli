//! `dec:WorkerImageSubmission` artifact type per FT-087 / ADR-040 /
//! ADR-055 — pure typed shape and RDF serialisation.
//!
//! Write-side SHACL validation (OCI-referrer checks) stays in
//! decision-cli's `core::ontology::worker_image_submission` glue module
//! (ADR-086).

pub mod types;

pub use types::{SubmissionLifecycleState, WorkerImageSubmission};
