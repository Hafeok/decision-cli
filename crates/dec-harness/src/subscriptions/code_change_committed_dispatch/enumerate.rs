//! Enumerate covering `(graph, env)` tuples for a feature (FT-100).
//!
//! Mirrors the slice-2.5 `features::verify_feature::enumerate` SPARQL but
//! lives in `core/` so the code-change-committed subscription handler
//! (which sits under `core/subscriptions`) can use it without violating
//! the SDP rule.

use std::path::Path;

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use dec_graph::store::{load_store_from_dump, orchestration_dump_path};
use dec_ontology::vocab::{
    IRI_DEC_BENCH_PREFIX, IRI_DEC_GRAPH_VERIFY_GRAPH, IRI_DEC_VERIFY_GRAPH_PREFIX,
};

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";

/// One `(graph, env)` tuple covering a feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphTuple {
    /// Graph IRI (`…/ns/graph/VG-NNN`).
    pub graph_iri: String,
    /// Graph short id (`VG-NNN`).
    pub graph_short: String,
    /// Environment IRI.
    pub env_iri: String,
    /// Environment short id.
    pub env_short: String,
}

/// Errors from the enumeration helpers.
#[derive(Debug, Error)]
pub enum EnumerateError {
    /// Orchestration store is missing or unreadable.
    #[error("store unreachable: {0}")]
    Store(String),
}

/// Enumerate every covering tuple for the supplied feature + TC list.
pub fn enumerate_covering_tuples(
    workdir: &Path,
    feature_iri: &str,
    tcs: &[String],
    env_filter: Option<&str>,
) -> Result<Vec<GraphTuple>, EnumerateError> {
    let dump = orchestration_dump_path(workdir);
    if !dump.exists() {
        return Err(EnumerateError::Store(format!(
            "no orchestration store at {}",
            dump.display()
        )));
    }
    let store = load_store_from_dump(&dump)
        .map_err(|e| EnumerateError::Store(format!("loading store: {e:#}")))?;
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
) -> Result<Vec<GraphTuple>, EnumerateError> {
    let mut tuples: Vec<GraphTuple> = Vec::new();
    extend_from_verifies_target(&mut tuples, store, feature_iri)?;
    for tc in tcs {
        extend_from_verifies_target(&mut tuples, store, tc)?;
    }
    extend_from_evidence(&mut tuples, store, tcs)?;
    Ok(tuples)
}

fn extend_from_verifies_target(
    out: &mut Vec<GraphTuple>,
    store: &Store,
    target_iri: &str,
) -> Result<(), EnumerateError> {
    let q = format!(
        "SELECT DISTINCT ?graph ?env WHERE {{\n  \
         GRAPH <{vg}> {{\n    \
         ?graph a <https://decision-cli.dev/ns#VerificationGraph> ;\n           \
         <https://decision-cli.dev/ns#verifies> <{target}> ;\n           \
         <https://decision-cli.dev/ns#bench> ?env .\n  \
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
) -> Result<(), EnumerateError> {
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
         <https://decision-cli.dev/ns#bench> ?env .\n    \
         ?step <https://decision-cli.dev/ns#providesEvidenceFor> ?tc .\n    \
         VALUES ?tc {{ {values} }}\n  \
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
) -> Result<(), EnumerateError> {
    let results = store
        .query(sparql)
        .map_err(|e| EnumerateError::Store(format!("enumerate SPARQL: {e}")))?;
    if let QueryResults::Solutions(sols) = results {
        for sol in sols {
            let sol = sol.map_err(|e| EnumerateError::Store(format!("enumerate row: {e}")))?;
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

fn graph_sort_key(id: &str) -> (u64, String) {
    let tail = id.strip_prefix("VG-").unwrap_or(id);
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n = digits.parse::<u64>().unwrap_or(u64::MAX);
    (n, id.to_string())
}
