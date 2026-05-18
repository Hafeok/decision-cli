//! decision-cli — orchestration crate for Decision-Driven Design.
//!
//! Drives product-cli through the engineering process by dispatching
//! LLM-backed roles and recording sessions in the graph-native event
//! substrate from `oxi-events`.

#![deny(clippy::unwrap_used)]

pub mod bundled;
pub mod init;
pub mod ontology;
pub mod stream_writer;
pub mod vocab;

pub use ontology::{OntologyError, OntologyHandle, ONTOLOGY_VERSION};
pub use stream_writer::StreamWriter;
