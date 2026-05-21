//! SPARQL execution helpers for the coverage primitive (FT-045).
//!
//! All routines here are read-only — they take `&Store`, run queries
//! defined in [`super::queries`], and return value types from
//! [`super::report`]. No quad is written; no global state is touched.
//! Determinism is achieved by sorting the result set in Rust before
//! returning it.

use std::collections::BTreeSet;

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::queries::{graph_exists_query, relevant_graphs_query, COVERAGE};
use super::report::{CoverageHit, GraphId, TcId};
use super::CoverageError;

/// True iff `graph_iri` is registered in the verify-graph named graph.
pub(super) fn graph_exists(store: &Store, graph_iri: &str) -> Result<bool, CoverageError> {
    let q = graph_exists_query(graph_iri);
    match store.query(q.as_str()).map_err(map_query_err)? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}

/// Discover every `VerificationGraph` whose `dec:verifies` matches the
/// feature itself or any of its TCs. Used when the caller supplies
/// `candidates = None` to roll up coverage across the full store.
pub(super) fn discover_relevant_graphs(
    store: &Store,
    feature_iri: &str,
    tcs: &[TcId],
) -> Result<Vec<GraphId>, CoverageError> {
    let mut targets: Vec<String> = Vec::with_capacity(tcs.len() + 1);
    targets.push(feature_iri.to_string());
    for tc in tcs {
        targets.push(tc.clone());
    }
    let q = relevant_graphs_query(&targets);
    let mut out: BTreeSet<String> = BTreeSet::new();
    if let QueryResults::Solutions(sols) = store.query(q.as_str()).map_err(map_query_err)? {
        for sol in sols {
            let sol = sol.map_err(map_query_err)?;
            if let Some(Term::NamedNode(n)) = sol.get("graph") {
                out.insert(n.as_str().to_string());
            }
        }
    }
    Ok(out.into_iter().collect())
}

/// Run the canonical [`COVERAGE`] query, filter results to
/// `(considered × tcs)`, and return a deterministic, deduplicated
/// list of hits.
pub(super) fn run_coverage_query(
    store: &Store,
    considered: &[GraphId],
    tcs: &[TcId],
) -> Result<Vec<CoverageHit>, CoverageError> {
    if considered.is_empty() || tcs.is_empty() {
        return Ok(Vec::new());
    }
    let considered_set: BTreeSet<&str> = considered.iter().map(String::as_str).collect();
    let tcs_set: BTreeSet<&str> = tcs.iter().map(String::as_str).collect();

    let mut hits: Vec<CoverageHit> = Vec::new();
    if let QueryResults::Solutions(sols) = store.query(COVERAGE).map_err(map_query_err)? {
        for sol in sols {
            let sol = sol.map_err(map_query_err)?;
            let Some(graph) = named_string(sol.get("graph")) else { continue };
            if !considered_set.contains(graph.as_str()) {
                continue;
            }
            let Some(tc) = named_string(sol.get("tc")) else { continue };
            if !tcs_set.contains(tc.as_str()) {
                continue;
            }
            let Some(step) = named_string(sol.get("step")) else { continue };
            hits.push(CoverageHit { tc, graph, step });
        }
    }
    hits.sort_by(|a, b| {
        (a.tc.as_str(), a.graph.as_str(), a.step.as_str())
            .cmp(&(b.tc.as_str(), b.graph.as_str(), b.step.as_str()))
    });
    hits.dedup();
    Ok(hits)
}

fn named_string(term: Option<&Term>) -> Option<String> {
    match term {
        Some(Term::NamedNode(n)) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn map_query_err<E: std::fmt::Display>(e: E) -> CoverageError {
    CoverageError::StoreUnreachable {
        detail: e.to_string(),
    }
}
