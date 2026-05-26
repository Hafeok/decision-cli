//! decision-cli — orchestration crate for Decision-Driven Design.
//!
//! Drives product-cli through the engineering process by dispatching
//! LLM-backed roles, recording sessions in the graph-native event
//! substrate from `oxi-events`. ADR-016 governs the internal layering:
//! `core` is the stable substrate, `features` are vertical slices, and
//! cross-feature imports are a compile error.

#![deny(clippy::unwrap_used)]

pub mod core;
pub mod features;

// Backwards-compatible top-level surface — external integration tests
// import from `decision_cli::{init, implement, …}`, so we re-export the
// feature modules at the crate root.
pub use crate::core::bundle;
pub use crate::core::bundled;
pub use crate::core::scope;
pub use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
pub use crate::core::subscriptions;
pub use crate::core::vocab;
pub use crate::core::worker;
pub use features::events;
pub use features::feedback;
pub use features::finalize;
pub use features::ft_074_migrate_provenance as migrate_provenance;
pub use features::ft_104_default_ack as default_ack;
pub use features::health;
pub use features::implement;
pub use features::init;
pub use features::mcp;
pub use features::preflight;
pub use features::session_inspect;
pub use features::submissions;
pub use features::telemetry;
pub use features::verify_env_list;
pub use features::verify_feature;
pub use features::verify_env_new;
pub use features::verify_env_show;
pub use features::verify_graph_generate;
pub use features::verify_graph_list;
pub use features::verify_graph_new;
pub use features::verify_graph_run;
pub use features::verify_graph_show;
pub use features::verify_step_add;
pub use features::workers_run;

// Stable type re-exports for slice 1 callers.
pub use crate::core::{
    ActiveScope, OntologyError, OntologyHandle, ScopeError, StreamWriter, ONTOLOGY_VERSION,
};
pub use features::finalize::{finalize_run, FinalizeError, FinalizeInput, FinalizeOutcome};
pub use features::health::{check as health_check, HealthReport};
pub use features::implement::{ImplementArgs, ImplementOutcome};
