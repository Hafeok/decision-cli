//! Phase-1 runtime safety re-check (FT-098 §Phase 1.2).
//!
//! Defensive replay of the FT-037 safety gate at run-time. The
//! authoring-time gate (`core::verify::safety::check_graph_against_env`)
//! cannot anticipate post-authoring mutations to the env or graph; this
//! module re-runs the same predicate immediately before the step loop
//! starts.
//!
//! **Single predicate, two call sites.** Both this module and FT-044's
//! `verify_step_add` invoke `core::verify::safety::check_step_against_env`.
//! Keeping the predicate centralised in `core::verify::safety` is what TC-155
//! §Acceptance requires.

use crate::core::ontology::verification_bench::VerificationBench;
use crate::core::ontology::verification_graph::VerificationGraph;
use crate::core::verify::safety::{check_step_against_env, SafetyError};

use super::request::RunnerError;

/// Run the static op-subset check on every step of the graph. The first
/// violation aborts the run with [`RunnerError::SafetyViolation`].
pub(crate) fn check(graph: &VerificationGraph, env: &VerificationBench) -> Result<(), RunnerError> {
    for step in &graph.steps {
        match check_step_against_env(step, env) {
            Ok(()) => continue,
            Err(SafetyError::Violation(v)) => {
                return Err(RunnerError::SafetyViolation {
                    step: v.step_id,
                    op: v.missing_ops.first().cloned().unwrap_or_default(),
                });
            }
            Err(SafetyError::UnknownOp { token, source }) => {
                return Err(RunnerError::SafetyViolation {
                    step: format!("(unknown-op declared by {})", source.as_str()),
                    op: token,
                });
            }
        }
    }
    Ok(())
}
