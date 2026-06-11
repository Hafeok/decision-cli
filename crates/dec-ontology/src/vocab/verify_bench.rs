//! FT-112 / ADR-028 — `dec:VerificationBench` vocabulary (renamed from Environment).
//!
//! Split out of `core::vocab` (mod.rs) to keep the per-file size within
//! the ADR-013 400-line ceiling. Re-exported from the parent module so
//! external callers continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

/// Class IRI for `dec:VerificationBench` (ADR-028, renamed by FT-112).
pub const IRI_DEC_VERIFICATION_BENCH: &str = "https://decision-cli.dev/ns#VerificationBench";

/// `dec:benchType` predicate (renamed from envType by FT-112).
pub const IRI_DEC_BENCH_TYPE: &str = "https://decision-cli.dev/ns#benchType";
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
/// `dec:fixtureSource` predicate (FT-053 / ADR-032).
pub const IRI_DEC_FIXTURE_SOURCE: &str = "https://decision-cli.dev/ns#fixtureSource";

/// Named graph holding the verification-bench projections (ADR-028 §State, renamed by FT-112).
pub const IRI_DEC_GRAPH_VERIFY_BENCH: &str = "https://decision-cli.dev/ns/graph/verify-bench";

/// IRI prefix for minted bench IRIs (`https://decision-cli.dev/ns/bench/<id>`), renamed by FT-112.
pub const IRI_DEC_BENCH_PREFIX: &str = "https://decision-cli.dev/ns/bench/";

/// Safety class literal — sandboxed; failure does not affect other systems.
pub const SAFETY_ISOLATED: &str = "isolated";
/// Safety class literal — multi-tenant; reads and non-mutating writes allowed.
pub const SAFETY_SHARED_NON_DESTRUCTIVE: &str = "shared-non-destructive";
/// Safety class literal — production; only read operations permitted.
pub const SAFETY_PRODUCTION_READONLY: &str = "production-readonly";

#[must_use]
pub fn verification_bench_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_BENCH)
}

#[must_use]
pub fn bench_type() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BENCH_TYPE)
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
pub fn fixture_source_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_FIXTURE_SOURCE)
}

#[must_use]
pub fn verify_bench_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_VERIFY_BENCH)
}
