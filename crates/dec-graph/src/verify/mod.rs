//! Store-facing verification primitives (ADR-086 seam of `core::verify`).
//!
//! [`quads`] (mutation inspection against the live store) and [`safety`]
//! (static safety rules per FT-037) moved here because the stream-writer
//! chokepoint consumes them on every write. The rest of the verification
//! machinery (matcher, coverage, runner, chain integrity) is
//! orchestration-level and stays above this crate.

pub mod quads;
pub mod safety;

pub use quads::{check_inserts_against_store, touches_verification_artifacts};
pub use safety::{
    check_graph_against_env, check_graph_against_env_all, check_step_against_env, required_ops_for,
    OpSource, SafetyError, SafetyViolation,
};
