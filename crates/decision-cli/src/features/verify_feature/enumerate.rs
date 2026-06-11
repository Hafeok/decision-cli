//! Graph enumeration for `dec verify feature` (FT-099).
//!
//! Returns the deterministic ordered set of `(VerificationGraph,
//! VerificationBench)` tuples that cover the supplied feature:
//! every graph whose `dec:verifies` matches the feature or any of its
//! TCs, optionally filtered to a single env id. Tuples are sorted by
//! the graph's VG-NNN numeric tail to make the runner's sequential
//! ordering predictable for tests.

use std::path::Path;

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::handler::Error as HandlerError;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::vocab::{
    IRI_DEC_BENCH_PREFIX, IRI_DEC_GRAPH_VERIFY_GRAPH, IRI_DEC_VERIFY_GRAPH_PREFIX,
};

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";

/// One `(graph, env)` tuple ready for dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphTuple {
    /// Graph IRI (`…/ns/graph/VG-NNN`).
    pub graph_iri: String,
    /// Graph short id (`VG-NNN`).
    pub graph_short: String,
    /// Environment IRI.
    pub env_iri: String,
    /// Environment short id (`BNCH-NNN[-suffix]`).
    pub env_short: String,
}

/// Enumerate every runnable tuple covering `feature_iri` (whose TCs are
/// `tcs`), filtered by `env_filter` (short id) when set.
pub(super) fn enumerate_runnable_tuples(
    workdir: &Path,
    feature_iri: &str,
    tcs: &[String],
    env_filter: Option<&str>,
) -> Result<Vec<GraphTuple>, HandlerError> {
    let dump = orchestration_dump_path(workdir);
    if !dump.exists() {
        return Err(HandlerError::Internal {
            detail: format!(
                "no orchestration store at {} — run `dec init` first",
                dump.display()
            ),
        });
    }
    let store = load_store_from_dump(&dump).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    let mut tuples = collect_tuples(&store, feature_iri, tcs)?;
    if let Some(env) = env_filter {
        let env_iri = format!("{IRI_DEC_BENCH_PREFIX}{env}");
        tuples.retain(|t| t.env_iri == env_iri || t.env_short == env);
    }
    tuples.sort_by(|a, b| graph_sort_key(&a.graph_short).cmp(&graph_sort_key(&b.graph_short)));
    tuples.dedup();
    Ok(tuples)
}

fn collect_tuples(
    store: &Store,
    feature_iri: &str,
    tcs: &[String],
) -> Result<Vec<GraphTuple>, HandlerError> {
    let mut tuples: Vec<GraphTuple> = Vec::new();
    // Pattern 1 (graphs whose dec:verifies is the feature) was
    // intentionally dropped: stale or speculative
    // `dec:verifies <feature>` triples in the store would pull in
    // graphs that have no step-level evidence for the feature's TCs.
    // Such graphs can't contribute to the per-TC roll-up anyway —
    // including them just produces "uncovered TCs" with empty
    // rationales. Coverage is now strictly evidence-based: pattern 2
    // (the graph directly verifies one of the feature's TCs) or
    // pattern 3 (a step provides evidence for one of the feature's
    // TCs). A graph that wants to be counted MUST emit per-TC
    // evidence.
    // Pattern 2: graphs whose dec:verifies is one of the feature's TCs.
    for tc in tcs {
        extend_from_verifies_target(&mut tuples, store, tc)?;
    }
    // Pattern 3: graphs whose any step has `dec:providesEvidenceFor` ?tc.
    extend_from_evidence(&mut tuples, store, tcs)?;
    // Suppress the unused `feature_iri` lint while keeping the
    // signature stable (callers still pass it in case future
    // patterns need it).
    let _ = feature_iri;
    Ok(tuples)
}

fn extend_from_verifies_target(
    out: &mut Vec<GraphTuple>,
    store: &Store,
    target_iri: &str,
) -> Result<(), HandlerError> {
    let q = format!(
        "SELECT DISTINCT ?graph ?env WHERE {{\n  \
         GRAPH <{vg}> {{\n    \
         ?graph a <https://decision-cli.dev/ns#VerificationGraph> ;\n           \
         <https://decision-cli.dev/ns#verifies> <{target}> ;\n           \
         <https://decision-cli.dev/ns#environment> ?env .\n    \
         FILTER NOT EXISTS {{ ?graph <https://decision-cli.dev/ns#supersededBy> ?_succ }}\n  \
         }}\n\
         }}",
        vg = IRI_DEC_GRAPH_VERIFY_GRAPH,
        target = target_iri,
    );
    run_select_tuples(store, &q, out)
}

fn extend_from_evidence(
    out: &mut Vec<GraphTuple>,
    store: &Store,
    tcs: &[String],
) -> Result<(), HandlerError> {
    if tcs.is_empty() {
        return Ok(());
    }
    let values = tcs
        .iter()
        .map(|t| format!("<{t}>"))
        .collect::<Vec<_>>()
        .join(" ");
    let q = format!(
        "SELECT DISTINCT ?graph ?env WHERE {{\n  \
         GRAPH <{vg}> {{\n    \
         ?graph a <https://decision-cli.dev/ns#VerificationGraph> ;\n           \
         <https://decision-cli.dev/ns#steps>/<{rest}>*/<{first}> ?step ;\n           \
         <https://decision-cli.dev/ns#environment> ?env .\n    \
         ?step <https://decision-cli.dev/ns#providesEvidenceFor> ?tc .\n    \
         VALUES ?tc {{ {values} }}\n    \
         FILTER NOT EXISTS {{ ?graph <https://decision-cli.dev/ns#supersededBy> ?_succ }}\n  \
         }}\n\
         }}",
        vg = IRI_DEC_GRAPH_VERIFY_GRAPH,
        rest = RDF_REST,
        first = RDF_FIRST,
        values = values,
    );
    run_select_tuples(store, &q, out)
}

fn run_select_tuples(
    store: &Store,
    sparql: &str,
    out: &mut Vec<GraphTuple>,
) -> Result<(), HandlerError> {
    let results = store.query(sparql).map_err(|e| HandlerError::Internal {
        detail: format!("enumerate SPARQL: {e}"),
    })?;
    if let QueryResults::Solutions(sols) = results {
        for sol in sols {
            let sol = sol.map_err(|e| HandlerError::Internal {
                detail: format!("enumerate SPARQL row: {e}"),
            })?;
            let graph = match sol.get("graph") {
                Some(Term::NamedNode(n)) => n.as_str().to_string(),
                _ => continue,
            };
            let env = match sol.get("env") {
                Some(Term::NamedNode(n)) => n.as_str().to_string(),
                _ => continue,
            };
            let graph_short = graph
                .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
                .unwrap_or(&graph)
                .to_string();
            let env_short = env
                .strip_prefix(IRI_DEC_BENCH_PREFIX)
                .unwrap_or(&env)
                .to_string();
            let tuple = GraphTuple {
                graph_iri: graph,
                graph_short,
                env_iri: env,
                env_short,
            };
            if !out.iter().any(|t| t == &tuple) {
                out.push(tuple);
            }
        }
    }
    Ok(())
}

/// Sort `VG-NNN[-suffix]` by numeric tail then string. Mirrors
/// `verify_graph_list::query::graph_sort_key` so the binary's ordering
/// is consistent across the verify surface.
#[must_use]
fn graph_sort_key(id: &str) -> (u64, String) {
    let tail = id.strip_prefix("VG-").unwrap_or(id);
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n = digits.parse::<u64>().unwrap_or(u64::MAX);
    (n, id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_orders_numeric_tail() {
        let mut ids = vec!["VG-010", "VG-002", "VG-001-foo"];
        ids.sort_by(|a, b| graph_sort_key(a).cmp(&graph_sort_key(b)));
        assert_eq!(ids, vec!["VG-001-foo", "VG-002", "VG-010"]);
    }
}
