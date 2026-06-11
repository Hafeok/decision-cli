//! VerificationGraph + VerificationStep artifact types per FT-036 /
//! ADR-028 — pure schema substrate: typed shapes, quad emission,
//! canonical-Turtle serialisation, and per-kind SHACL validation.
//!
//! Turtle *parsing* (`from_turtle`, `from_turtle_bytes`) needs an RDF
//! parser and stays in decision-cli's `core::ontology::verification_graph`
//! glue module (ADR-086).

pub mod quads;
pub mod serialize;
pub mod shacl;
pub mod types;

pub use shacl::{validate_quads, GraphShaclError, GraphViolation, UnknownStepKindError};
pub use types::{
    step_iri_for, ArtifactRef, GraphIri, StepFields, StepIri, StepKind, VerificationGraph,
    VerificationStep,
};
