//! Environment IRI helpers for the matcher (FT-046).
//!
//! Two tiny helpers: turn an `BNCH-NNN[-suffix]` short id into its IRI
//! and ASK whether an env exists in the verify-bench named graph. Both
//! are read-only; both are kept here so the matcher's `mod.rs` stays
//! focused on the cover algorithm.

use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::MatchError;
use crate::core::vocab::{IRI_DEC_BENCH_PREFIX, IRI_DEC_GRAPH_VERIFY_BENCH};

/// Translate a short env id (`BNCH-001-ephemeral-cli`) into its IRI
/// form. Already-IRI inputs pass through unchanged.
#[must_use]
pub(super) fn bench_iri_for(short_id: &str) -> String {
    if short_id.starts_with("https://") {
        short_id.to_string()
    } else {
        format!("{IRI_DEC_BENCH_PREFIX}{short_id}")
    }
}

/// True iff `env_iri` is registered as a `dec:VerificationBench`
/// in the verify-bench named graph.
pub(super) fn bench_exists(store: &Store, env_iri: &str) -> Result<bool, MatchError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         ASK WHERE {{\n  GRAPH <{graph}> {{\n    <{iri}> a dec:VerificationBench .\n  }}\n}}\n",
        graph = IRI_DEC_GRAPH_VERIFY_BENCH,
        iri = env_iri,
    );
    match store
        .query(q.as_str())
        .map_err(|e| MatchError::StoreUnreachable {
            detail: e.to_string(),
        })? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}
