//! Existing-graph matcher primitive (FT-046 / ADR-030).
//!
//! Pure read-only SPARQL over the orchestration store. Given a feature
//! and a target environment, the matcher returns the *best matching*
//! set of pre-existing `dec:VerificationGraph` artifacts (the smallest
//! set whose union covers all of the feature's TCs in that env). The
//! decision is deterministic: greedy minimum cover with a stable
//! tiebreak by ascending `VG-NNN` numeric suffix.
//!
//! No writes, no PROV-O, no logging beyond a single tracing span.
//! Per-env scoping is strict: a graph in a different env is never
//! considered, even if it would cover everything.

mod assemble;
mod bench_lookup;
mod candidates;
mod greedy;
mod query;
mod report;

pub use report::{EnvId, GraphSummary, MatchKind, MatchReport};

use std::path::Path;

use oxigraph::store::Store;
use thiserror::Error;

use super::coverage::{
    feature_resolver::{feature_iri_for, resolve_feature_tc_iris},
    CoverageError,
};

/// Failure surface of the matcher primitive. Mirrors FT-046 §Error handling.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MatchError {
    /// Unknown feature, environment, or candidate graph.
    #[error("not found: {kind} '{id}'")]
    ArtifactNotFound {
        /// Artifact kind label (e.g. `"Feature"`, `"VerificationBench"`).
        kind: String,
        /// The id the caller supplied.
        id: String,
    },
    /// The orchestration store could not be queried.
    #[error("store unreachable: {detail}")]
    StoreUnreachable {
        /// Human-readable diagnostic.
        detail: String,
    },
}

impl From<CoverageError> for MatchError {
    fn from(e: CoverageError) -> Self {
        match e {
            CoverageError::ArtifactNotFound { kind, id } => Self::ArtifactNotFound { kind, id },
            CoverageError::StoreUnreachable { detail } => Self::StoreUnreachable { detail },
        }
    }
}

/// Compute the best-matching set of existing `dec:VerificationGraph`
/// artifacts for `feature` in `env`.
///
/// Returns a [`MatchReport`] whose `graphs` is the minimum cover
/// (greedy, deterministic) chosen from candidates whose
/// `dec:environment` equals `env`. `MatchKind::None` is a valid
/// outcome (no graph in this env touches any of the feature's TCs);
/// the caller decides what to do with it.
///
/// # Errors
/// See [`MatchError`].
pub fn best_matching_graphs(
    feature: &str,
    env: &str,
    store: &Store,
    product_root: &Path,
) -> Result<MatchReport, MatchError> {
    let _span = tracing::trace_span!("matcher.best", feature, env).entered();

    let feature_iri = feature_iri_for(feature);
    let all_tcs = resolve_feature_tc_iris(product_root, feature)?;
    let env_iri = bench_lookup::bench_iri_for(env);
    if !bench_lookup::bench_exists(store, &env_iri)? {
        return Err(MatchError::ArtifactNotFound {
            kind: "VerificationBench".to_string(),
            id: env.to_string(),
        });
    }

    let non_empty = candidates::collect_non_empty(store, &env_iri, feature, product_root)?;
    if non_empty.is_empty() {
        return Ok(assemble::none_report(feature_iri, env_iri, all_tcs));
    }
    if let Some(single) = assemble::pick_complete_single(&non_empty, &all_tcs) {
        return Ok(assemble::single_report(
            feature_iri,
            env_iri,
            &all_tcs,
            single,
        ));
    }
    let cover = greedy::minimum_cover(&non_empty, &all_tcs);
    Ok(assemble::cover_report(
        feature_iri,
        env_iri,
        &all_tcs,
        &cover,
    ))
}
