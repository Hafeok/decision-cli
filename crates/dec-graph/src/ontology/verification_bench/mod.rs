//! Store-facing glue for VerificationBench (FT-035 / ADR-028).
//!
//! The pure schema substrate lives in
//! [`dec_ontology::ontology::verification_bench`] (ADR-086); this module
//! keeps Turtle *parsing* (which needs an RDF parser) and re-exports
//! both halves.

pub use dec_ontology::ontology::verification_bench::*;

pub mod io;

#[cfg(test)]
mod tests;

pub use io::{from_turtle, from_turtle_bytes, EnvIoError};
