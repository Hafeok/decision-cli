//! decision-cli — orchestration crate for Decision-Driven Design.
//!
//! Drives product-cli through the engineering process by dispatching
//! LLM-backed roles and recording sessions in the graph-native event
//! substrate from `oxi-events`.

#![deny(clippy::unwrap_used)]

pub mod bundled;
pub mod events;
pub mod health;
pub mod implement;
pub mod init;
pub mod ontology;
pub mod scope;
pub mod session_inspect;
pub mod stream_writer;
pub mod vocab;

pub use health::{check as health_check, HealthReport};
pub use implement::{ImplementArgs, ImplementOutcome};
pub use ontology::{OntologyError, OntologyHandle, ONTOLOGY_VERSION};
pub use scope::{ActiveScope, ScopeError};
pub use stream_writer::StreamWriter;
