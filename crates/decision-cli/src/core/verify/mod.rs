//! Verification chokepoints — pure safety enforcement for verification graphs.
//!
//! Per FT-037 / ADR-028 §Safety gating, every `dec:VerificationGraph`
//! commit and every per-step append routes through this module. The
//! check is structural and deterministic: `step.requiredOps ⊆ env.allowedOps`.
//!
//! The module is intentionally narrow — only the safety substrate lives
//! here. The graph and env schemas live under `core::ontology`; the
//! StreamWriter chokepoint integrates the check at write time.

pub mod coverage;
pub mod quads;
pub mod safety;

pub use coverage::{
    feature_covered_by, feature_coverage, CoverageError, CoverageHit, CoverageReport,
    FeatureId, GraphId, StepId, TcId,
};
pub use quads::{check_inserts_against_store, touches_verification_artifacts};
pub use safety::{
    check_graph_against_env, check_graph_against_env_all, check_step_against_env,
    required_ops_for, OpSource, SafetyError, SafetyViolation,
};
