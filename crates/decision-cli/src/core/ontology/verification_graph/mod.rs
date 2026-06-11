//! Store-facing glue for VerificationGraph + VerificationStep
//! (FT-036 / ADR-028).
//!
//! The pure schema substrate lives in
//! [`dec_ontology::ontology::verification_graph`] (ADR-086); this module
//! keeps Turtle *parsing* (which needs an RDF parser) and re-exports
//! both halves.

pub use dec_ontology::ontology::verification_graph::*;

pub mod io;
mod parse;

pub use io::{from_turtle, from_turtle_bytes, to_canonical_turtle, GraphIoError};
