//! Stable substrate shared by every vertical-slice feature (ADR-016).
//!
//! Per ADR-016, `core/` sits below `features/` in the slice-level
//! Stable Dependency Principle: features depend on `core`; nothing in
//! `core` may depend on a sibling `features::*` module. The contents
//! here are the load-bearing primitives — ontology + vocab, the
//! orchestration store load/dump helpers, SPARQL term utilities, the
//! scope object, the stream-aware writer, the bundled definition
//! library, and the worker resolution chain.

pub mod bootstrap;
pub use dec_graph::bundle;
pub mod bundled;
pub mod cluster_session;
pub mod cosign_trust;
pub mod dispatch;
pub mod dispatch_session;
pub mod drive;
pub mod feedback;
pub use dec_graph::graph;
pub mod handler;
pub mod identity_verifier;
pub mod mcp;
pub mod metrics;
pub mod oci_manifest;
pub use dec_graph::ontology;
pub use dec_graph::queries;
pub mod role_catalog;
pub use dec_graph::sbom_referrer;
pub use dec_graph::scope;
pub use dec_graph::sparql;
pub use dec_graph::store;
pub use dec_graph::stream_writer;
pub mod subscriptions;
pub mod task_type;
pub mod verify;
// ADR-086: the IRI vocabulary (dec-ontology) and the graph-access layer
// (dec-graph) are re-exported so existing crate::core::… paths keep working.
pub use dec_ontology::vocab;
pub mod worker;
pub mod worker_curator;
pub mod worker_manifest;

pub use ontology::{OntologyError, OntologyHandle, ONTOLOGY_VERSION};
pub use scope::{ActiveScope, ScopeError};
pub use stream_writer::StreamWriter;
