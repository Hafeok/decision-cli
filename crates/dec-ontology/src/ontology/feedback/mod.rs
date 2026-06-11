//! Feedback artifact type per FT-026 / ADR-022 — pure schema substrate.
//!
//! The in-memory shape, RDF (de)serialisation, the ADR-022 SHACL shape
//! (required fields and cardinality), the class controlled vocabulary
//! (FT-028), and the lifecycle state machine (FT-027). Store-backed
//! reads, routing (FT-029), transition application, and address walking
//! stay in decision-cli's `core::feedback` glue module (ADR-086).

pub mod artifact;
pub mod class;
pub mod defect_record;
pub mod lifecycle;
mod lifecycle_shacl;
pub mod shacl;

pub use artifact::{Feedback, Severity};
pub use class::{Disposition, FeedbackClass};
pub use defect_record::DefectFeedbackRecord;
pub use lifecycle::{next_states, validate_transition, LifecycleState, TransitionError};
pub use shacl::{validate_quads, FeedbackShaclError, FeedbackViolation};
