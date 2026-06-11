//! StreamWriter chokepoint integration for FT-037 safety enforcement.
//!
//! Drives the pure check from [`super::safety`] over any
//! `VerificationGraph`/`Step` subject present in the inserted quad set.
//! Resolution of the per-step environment prefers the same mutation
//! first (graph + env re-committed together — the on-disk atomic
//! rewrite contract), then falls back to the store.
//!
//! This is the **only** call site that wires the safety check into a
//! commit path; TC-059 §Acceptance 4's structural test relies on that
//! exclusivity.

mod lookup;
mod store_view;

use oxigraph::model::Quad;
use oxigraph::store::Store;

use super::safety::{check_step_against_env, SafetyError};

use lookup::{step_env_iri, step_subjects_in};
use store_view::{load_env, step_from_quads};

/// True iff `inserts` declares at least one `VerificationGraph` or
/// `VerificationStep` subject. Used by the StreamWriter chokepoint to
/// short-circuit when no verification artifact is being mutated.
#[must_use]
pub fn touches_verification_artifacts(inserts: &[Quad]) -> bool {
    lookup::touches_verification_artifacts(inserts)
}

/// Drive the safety check across every `VerificationStep` subject in
/// `inserts`. Returns `Ok(())` if there are no such subjects or every
/// present subject is safe against its env.
///
/// Empty graphs (no step subjects in the inserts and `dec:steps ()`)
/// trivially pass per TC-059 §Acceptance 3.
///
/// # Errors
/// Returns the first [`SafetyError`] encountered.
pub fn check_inserts_against_store(inserts: &[Quad], store: &Store) -> Result<(), SafetyError> {
    for subject in step_subjects_in(inserts) {
        // If the step's env can't be resolved (dangling reference), the
        // safety check is vacuously Ok — FT-036's `DanglingRef` SHACL
        // pass catches the missing env. Per FT-037 §Error handling:
        // "Env not found → delegated to FT-036's DanglingRef error;
        // not surfaced as a safety violation."
        let Ok(env_iri) = step_env_iri(inserts, store, &subject) else {
            continue;
        };
        let Some(env) = load_env(inserts, store, &env_iri) else {
            continue;
        };
        let step = step_from_quads(inserts, store, &subject)?;
        check_step_against_env(&step, &env)?;
    }
    Ok(())
}
