//! Stable substrate shared by every vertical-slice feature (ADR-016).
//!
//! Per ADR-016, `core/` sits below `features/` in the slice-level
//! Stable Dependency Principle: features depend on `core`; nothing in
//! `core` may depend on a sibling `features::*` module. The contents
//! here are the load-bearing primitives — ontology + vocab, the
//! orchestration store load/dump helpers, SPARQL term utilities, the
//! scope object, the stream-aware writer, the bundled definition
//! library, and the worker resolution chain.

pub mod bundled;
pub mod dispatch;
pub mod feedback;
pub mod handler;
pub mod mcp;
pub mod metrics;
pub mod ontology;
pub mod role_catalog;
pub mod scope;
pub mod sparql;
pub mod store;
pub mod stream_writer;
pub mod subscriptions;
pub mod vocab;
pub mod worker;

pub use ontology::{OntologyError, OntologyHandle, ONTOLOGY_VERSION};
pub use scope::{ActiveScope, ScopeError};
pub use stream_writer::StreamWriter;
