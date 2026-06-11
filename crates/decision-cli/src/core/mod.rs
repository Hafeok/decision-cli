//! Stable substrate shared by every vertical-slice feature (ADR-016).
//!
//! Per ADR-016, `core/` sits below `features/` in the slice-level
//! Stable Dependency Principle: features depend on `core`; nothing in
//! `core` may depend on a sibling `features::*` module. The contents
//! here are the load-bearing primitives — ontology + vocab, the
//! orchestration store load/dump helpers, SPARQL term utilities, the
//! scope object, the stream-aware writer, the bundled definition
//! library, and the worker resolution chain.

pub use dec_graph::bundle;
pub use dec_harness::bootstrap;
pub mod bundled;
pub mod cluster_session;
pub use dec_graph::graph;
pub use dec_harness::cosign_trust;
pub use dec_harness::dispatch;
pub use dec_harness::dispatch_session;
pub use dec_harness::drive;
pub use dec_harness::feedback;
pub use dec_harness::handler;
pub use dec_harness::identity_verifier;
pub mod mcp;
pub use dec_graph::ontology;
pub use dec_graph::queries;
pub use dec_graph::sbom_referrer;
pub use dec_graph::scope;
pub use dec_graph::sparql;
pub use dec_graph::store;
pub use dec_graph::stream_writer;
pub use dec_harness::metrics;
pub use dec_harness::oci_manifest;
pub use dec_harness::role_catalog;
pub use dec_harness::subscriptions;
pub use dec_harness::task_type;
pub use dec_harness::verify;
// ADR-086: the IRI vocabulary (dec-ontology) and the graph-access layer
// (dec-graph) are re-exported so existing crate::core::… paths keep working.
pub use dec_harness::worker;
pub use dec_harness::worker_curator;
pub use dec_harness::worker_manifest;
pub use dec_ontology::vocab;

pub use ontology::{OntologyError, OntologyHandle, ONTOLOGY_VERSION};
pub use scope::{ActiveScope, ScopeError};
pub use stream_writer::StreamWriter;
