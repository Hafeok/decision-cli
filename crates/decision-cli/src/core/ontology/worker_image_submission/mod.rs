//! Store-facing glue for `dec:WorkerImageSubmission` (FT-087 / ADR-040 /
//! ADR-055).
//!
//! The pure typed shape lives in
//! [`dec_ontology::ontology::worker_image_submission`] (ADR-086); this
//! module keeps write-side SHACL validation (OCI-referrer checks) and
//! re-exports both halves.

pub use dec_ontology::ontology::worker_image_submission::*;

mod shacl;

#[cfg(test)]
mod tests;

pub use shacl::{validate_quads, WorkerImageSubmissionShaclError, WorkerImageSubmissionViolation};
