//! FT-035 / ADR-028 — `dec:VerificationEnvironment` vocabulary.
//!
//! Split out of `core::vocab` (mod.rs) to keep the per-file size within
//! the ADR-013 400-line ceiling. Re-exported from the parent module so
//! external callers continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

/// Class IRI for `dec:VerificationEnvironment` (ADR-028).
pub const IRI_DEC_VERIFICATION_ENVIRONMENT: &str =
    "https://decision-cli.dev/ns#VerificationEnvironment";

/// `dec:envType` predicate.
pub const IRI_DEC_ENV_TYPE: &str = "https://decision-cli.dev/ns#envType";
/// `dec:safetyClass` predicate.
pub const IRI_DEC_SAFETY_CLASS: &str = "https://decision-cli.dev/ns#safetyClass";
/// `dec:allowedOps` predicate (rdf:List head).
pub const IRI_DEC_ALLOWED_OPS: &str = "https://decision-cli.dev/ns#allowedOps";
/// `dec:setup` predicate.
pub const IRI_DEC_SETUP: &str = "https://decision-cli.dev/ns#setup";
/// `dec:teardown` predicate.
pub const IRI_DEC_TEARDOWN: &str = "https://decision-cli.dev/ns#teardown";
/// `dec:endpoint` predicate.
pub const IRI_DEC_ENDPOINT: &str = "https://decision-cli.dev/ns#endpoint";

/// Named graph holding the verification-environment projections (ADR-028 §State).
pub const IRI_DEC_GRAPH_VERIFY_ENV: &str = "https://decision-cli.dev/ns/graph/verify-env";

/// IRI prefix for minted environment IRIs (`https://decision-cli.dev/ns/env/<id>`).
pub const IRI_DEC_ENV_PREFIX: &str = "https://decision-cli.dev/ns/env/";

/// Safety class literal — sandboxed; failure does not affect other systems.
pub const SAFETY_ISOLATED: &str = "isolated";
/// Safety class literal — multi-tenant; reads and non-mutating writes allowed.
pub const SAFETY_SHARED_NON_DESTRUCTIVE: &str = "shared-non-destructive";
/// Safety class literal — production; only read operations permitted.
pub const SAFETY_PRODUCTION_READONLY: &str = "production-readonly";

#[must_use]
pub fn verification_environment_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_ENVIRONMENT)
}

#[must_use]
pub fn env_type() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ENV_TYPE)
}

#[must_use]
pub fn safety_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SAFETY_CLASS)
}

#[must_use]
pub fn allowed_ops() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ALLOWED_OPS)
}

#[must_use]
pub fn setup_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SETUP)
}

#[must_use]
pub fn teardown_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TEARDOWN)
}

#[must_use]
pub fn endpoint_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ENDPOINT)
}

#[must_use]
pub fn verify_env_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_VERIFY_ENV)
}
