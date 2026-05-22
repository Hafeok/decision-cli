//! VerificationGraph + VerificationStep artifact types per FT-036 / ADR-028.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-037 safety enforcement, FT-041..FT-044 graph CLI subcommands)
//! imports from this module — no consumer imports from sibling features.
//!
//! Scope of FT-036 is intentionally the schema substrate: the in-memory
//! shape (`StepKind`, `StepFields`, `VerificationStep`, `VerificationGraph`),
//! RDF serialisation (`to_quads`), on-disk Turtle round-trip
//! (`from_turtle`, `to_canonical_turtle`), and the per-kind SHACL shapes
//! (`validate_quads`). Safety enforcement (FT-037), CLI subcommands
//! (FT-041..FT-044), and step executors (slice 3) are out of scope here.

pub mod io;
mod parse;
pub mod quads;
pub mod serialize;
pub mod shacl;
pub mod types;

pub use io::{from_turtle, from_turtle_bytes, to_canonical_turtle, GraphIoError};
pub use shacl::{validate_quads, GraphShaclError, GraphViolation, UnknownStepKindError};
pub use types::{
    step_iri_for, ArtifactRef, GraphIri, StepFields, StepIri, StepKind, VerificationGraph,
    VerificationStep,
};
