//! SPARQL enumeration of `dec:VerificationGraph`s in a given env (FT-046).
//!
//! Read-only. Returns a deterministic, suffix-sorted list of
//! `(graph_iri, verifies_iri)` pairs — the matcher then asks
//! [`crate::core::verify::coverage::feature_covered_by`] one graph at
//! a time to derive each candidate's per-graph coverage subset.

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::greedy;
use super::MatchError;
use crate::core::vocab::IRI_DEC_GRAPH_VERIFY_GRAPH;

/// Enumerate every `dec:VerificationGraph` whose `dec:environment`
/// equals `env_iri`. Each row is `(graph_iri, verifies_iri)`.
///
/// Order: ascending numeric suffix of the graph id (`VG-3` before
/// `VG-10`). The matcher relies on this stable order for the
/// CompleteSingle tiebreak — equal-cover candidates lose to lower
/// suffixes.
pub(super) fn graphs_in_env(
    store: &Store,
    env_iri: &str,
) -> Result<Vec<(String, String)>, MatchError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         SELECT ?graph ?verifies WHERE {{\n  \
         GRAPH <{graph}> {{\n    \
         ?graph a dec:VerificationGraph ;\n           \
         dec:environment <{env}> ;\n           \
         dec:verifies ?verifies .\n  \
         }}\n}}\n",
        graph = IRI_DEC_GRAPH_VERIFY_GRAPH,
        env = env_iri,
    );
    let mut rows: Vec<(String, String)> = Vec::new();
    if let QueryResults::Solutions(sols) = store.query(q.as_str()).map_err(map_err)? {
        for sol in sols {
            let sol = sol.map_err(map_err)?;
            let Some(graph) = named_string(sol.get("graph")) else {
                continue;
            };
            let Some(verifies) = named_string(sol.get("verifies")) else {
                continue;
            };
            rows.push((graph, verifies));
        }
    }
    // Stable sort by numeric suffix so equal-cover singles + greedy
    // tiebreaks resolve deterministically.
    rows.sort_by(|a, b| {
        greedy::numeric_suffix_key(short_id_of(&a.0))
            .cmp(&greedy::numeric_suffix_key(short_id_of(&b.0)))
    });
    rows.dedup();
    Ok(rows)
}

fn short_id_of(graph_iri: &str) -> &str {
    graph_iri
        .strip_prefix(crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX)
        .unwrap_or(graph_iri)
}

fn named_string(term: Option<&Term>) -> Option<String> {
    match term {
        Some(Term::NamedNode(n)) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn map_err<E: std::fmt::Display>(e: E) -> MatchError {
    MatchError::StoreUnreachable {
        detail: e.to_string(),
    }
}
