//! # dec-graph — the decision-cli graph-access layer (ADR-086)
//!
//! Everything that touches the orchestration store goes through this
//! crate: store open/load/dump, named-graph management, SPARQL execution
//! helpers, bundle CONSTRUCT assembly, the stream-aware writer
//! (ADR-005), and the SHACL-enforced GraphWriter chokepoint (ADR-041).
//!
//! The crate depends on the pure domain ([`dec_ontology`]) — never the
//! other way around — and is itself free of orchestration machinery
//! (dispatch loops, drive planners, worker contracts live above, in
//! `dec-harness` / `decision-cli`). The store-facing halves of the
//! typed-artifact modules (store-backed read APIs, store-aware SHACL
//! validators, Turtle parsing, the [`ontology::OntologyHandle`] loader)
//! live here under [`ontology`], which re-exports the pure halves so
//! consumers see one surface.

#![deny(clippy::unwrap_used)]

pub mod bundle;
pub mod graph;
pub mod ontology;
pub mod queries;
pub mod sbom_referrer;
pub mod scope;
pub mod sparql;
pub mod store;
pub mod stream_writer;
mod stream_writer_validations;
pub mod verify;

pub use scope::{ActiveScope, ScopeError};
pub use stream_writer::StreamWriter;
