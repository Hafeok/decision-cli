//! Coverage check primitive (FT-045 / ADR-030 / ADR-031).
//!
//! Pure read-only SPARQL query over the orchestration store. The
//! primitive answers two structural questions:
//!
//! 1. **Per-graph coverage.** [`feature_covered_by`] — of `feature`'s
//!    TCs, which are covered by `graph`'s steps via
//!    `dec:providesEvidenceFor`?
//! 2. **Roll-up coverage.** [`feature_coverage`] — across a candidate
//!    set (or every relevant graph in the store), which of the
//!    feature's TCs are covered by at least one step?
//!
//! No writes, no PROV-O, no logging beyond a single tracing span.
//! Determinism is guaranteed for a fixed store snapshot.

pub mod feature_resolver;
pub mod queries;
pub mod report;

mod query_exec;

pub use queries::COVERAGE;
pub use report::{CoverageHit, CoverageReport, FeatureId, GraphId, StepId, TcId};

use std::path::Path;

use oxigraph::store::Store;
use thiserror::Error;

/// Failure surface of the coverage primitive. Mirrors the shapes
/// FT-045 §Error handling specifies.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CoverageError {
    /// The supplied feature id or candidate graph id does not resolve
    /// to a known artifact (`kind ∈ {"Feature", "VerificationGraph"}`).
    #[error("not found: {kind} '{id}'")]
    ArtifactNotFound {
        /// Artifact kind label (e.g. `"Feature"`, `"VerificationGraph"`).
        kind: String,
        /// The id the caller supplied (short id or IRI).
        id: String,
    },
    /// The orchestration store could not be queried (SPARQL parse
    /// failure, I/O error, etc.).
    #[error("store unreachable: {detail}")]
    StoreUnreachable {
        /// Human-readable diagnostic.
        detail: String,
    },
}

/// Per-graph coverage: does `graph` cover `feature`'s TCs?
///
/// Returns a [`CoverageReport`] whose `considered` is `[graph]` and
/// whose `covered`/`uncovered` partition `feature`'s TCs based on
/// `dec:providesEvidenceFor` linkage from `graph`'s steps.
///
/// # Errors
/// See [`CoverageError`].
pub fn feature_covered_by(
    feature: &str,
    graph: &str,
    store: &Store,
    product_root: &Path,
) -> Result<CoverageReport, CoverageError> {
    feature_coverage(feature, Some(vec![graph.to_string()]), store, product_root)
}

/// Feature coverage roll-up across a candidate set.
///
/// When `candidates` is `Some`, every supplied id is validated against
/// the store; an unknown id surfaces [`CoverageError::ArtifactNotFound`]
/// before any coverage SPARQL runs. When `candidates` is `None`, every
/// `VerificationGraph` whose `dec:verifies` matches the feature or one
/// of its TCs is considered (see [`query_exec::discover_relevant_graphs`]).
///
/// Empty candidate sets are accepted (FT-045 §Invariants): the report
/// reports `uncovered = all_tcs`, `covered = []`.
///
/// # Errors
/// See [`CoverageError`].
pub fn feature_coverage(
    feature: &str,
    candidates: Option<Vec<String>>,
    store: &Store,
    product_root: &Path,
) -> Result<CoverageReport, CoverageError> {
    let _span = tracing::trace_span!("coverage.feature", feature).entered();
    let feature_iri = feature_resolver::feature_iri_for(feature);
    let all_tcs = feature_resolver::resolve_feature_tc_iris(product_root, feature)?;

    let considered = match candidates {
        Some(ids) => resolve_candidate_set(store, ids)?,
        None => query_exec::discover_relevant_graphs(store, &feature_iri, &all_tcs)?,
    };

    let covered = query_exec::run_coverage_query(store, &considered, &all_tcs)?;

    let covered_set: std::collections::BTreeSet<&str> =
        covered.iter().map(|h| h.tc.as_str()).collect();
    let uncovered: Vec<TcId> = all_tcs
        .iter()
        .filter(|t| !covered_set.contains(t.as_str()))
        .cloned()
        .collect();

    Ok(CoverageReport {
        feature: feature_iri,
        all_tcs,
        covered,
        uncovered,
        considered,
    })
}

/// Translate a caller-supplied candidate id list into a vector of full
/// IRIs, validating each exists in the verify-graph named graph.
fn resolve_candidate_set(
    store: &Store,
    ids: Vec<String>,
) -> Result<Vec<GraphId>, CoverageError> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let iri = feature_resolver::graph_iri_for(&id);
        if !query_exec::graph_exists(store, &iri)? {
            return Err(CoverageError::ArtifactNotFound {
                kind: "VerificationGraph".to_string(),
                id,
            });
        }
        out.push(iri);
    }
    Ok(out)
}
