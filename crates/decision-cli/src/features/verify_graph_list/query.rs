//! SPARQL projection for `dec verify graph list` (FT-042).
//!
//! Reads the verify-graph named graph that `dec verify graph new` writes
//! into. The query applies filter predicates and computes `step_count`
//! server-side via SPARQL's rdf:List property path
//! (`dec:steps/rdf:rest*/rdf:first`) so the handler ships only matching
//! rows back to the caller.

use std::path::Path;

use oxigraph::sparql::{QueryResults, QuerySolution};
use oxigraph::store::Store;

use crate::core::handler::Error as HandlerError;
use crate::core::sparql::term_iri_string;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::vocab::{
    environment_pred, steps_pred, verification_graph_class, verifies_pred,
    IRI_DEC_ENV_PREFIX, IRI_DEC_GRAPH_VERIFY_GRAPH, IRI_DEC_VERIFY_GRAPH_PREFIX,
};

use super::GraphSummary;

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";

/// IRI prefix for feature artifacts the graph's `dec:verifies` references.
const IRI_FEATURE_PREFIX: &str = "https://decision-cli.dev/ns/feature/";
/// IRI prefix for test-criterion artifacts the graph's `dec:verifies` references.
const IRI_TC_PREFIX: &str = "https://decision-cli.dev/ns/tc/";

/// Load the orchestration store and project graph summaries via SPARQL.
pub(super) fn query_graphs(
    workdir: &Path,
    verifies: Option<&str>,
    environment: Option<&str>,
) -> Result<Vec<GraphSummary>, HandlerError> {
    let store = load_orchestration_store(workdir)?;
    let q = build_query(verifies, environment);
    let results = store.query(q.as_str()).map_err(|e| HandlerError::Internal {
        detail: format!("graph-list SPARQL: {e}"),
    })?;
    let QueryResults::Solutions(sols) = results else {
        return Err(HandlerError::Internal {
            detail: "graph-list: unexpected SPARQL result shape".to_string(),
        });
    };
    let mut out: Vec<GraphSummary> = Vec::new();
    for sol_res in sols {
        let sol = sol_res.map_err(|e| HandlerError::Internal {
            detail: format!("graph-list SPARQL row: {e}"),
        })?;
        if let Some(summary) = solution_to_summary(&sol) {
            out.push(summary);
        }
    }
    Ok(out)
}

/// Load the orchestration store at `workdir`, surfacing the canonical
/// "no orchestration store" diagnostic when the dump file is missing.
fn load_orchestration_store(workdir: &Path) -> Result<Store, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Err(HandlerError::Internal {
            detail: format!(
                "no orchestration store at {} — run `dec init` first",
                dump_path.display()
            ),
        });
    }
    load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })
}

/// Project one SPARQL solution row into a `GraphSummary`. Returns
/// `None` when the row's graph IRI does not carry the verify-graph
/// prefix (defensive — the SHACL shape forbids this in practice).
fn solution_to_summary(sol: &QuerySolution) -> Option<GraphSummary> {
    let graph_iri = sol.get("graph").map(term_iri_string).unwrap_or_default();
    let id = graph_iri
        .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
        .map(str::to_string)?;
    let verifies_iri = sol.get("verifies").map(term_iri_string).unwrap_or_default();
    let environment_iri = sol.get("env").map(term_iri_string).unwrap_or_default();
    let environment_short = environment_iri
        .strip_prefix(IRI_DEC_ENV_PREFIX)
        .unwrap_or(environment_iri.as_str())
        .to_string();
    let step_count = sol
        .get("count")
        .and_then(|t| match t {
            oxigraph::model::Term::Literal(lit) => lit.value().parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);
    Some(GraphSummary {
        id,
        verifies: canonicalize_verifies(&verifies_iri),
        environment: environment_short,
        step_count,
    })
}

/// Strip the feature/TC IRI prefix off of `dec:verifies` resolutions so
/// the response carries the canonical short id (`FT-001`, `TC-013`).
/// Unknown prefixes pass through unchanged.
fn canonicalize_verifies(iri: &str) -> String {
    if let Some(tail) = iri.strip_prefix(IRI_FEATURE_PREFIX) {
        return tail.to_string();
    }
    if let Some(tail) = iri.strip_prefix(IRI_TC_PREFIX) {
        return tail.to_string();
    }
    iri.to_string()
}

/// SPARQL SELECT for graph summaries with optional filters.
///
/// `step_count` is computed via the rdf:List property path
/// `dec:steps/rdf:rest*/rdf:first` — an aggregate over the materialised
/// list members. Empty lists (the `dec:steps rdf:nil` case) produce zero
/// matches and a `0` count via `OPTIONAL` + `COUNT`.
///
/// The FILTER predicates collapse to no-ops when the corresponding
/// option is `None`; conjunctive when both are `Some`.
fn build_query(verifies: Option<&str>, environment: Option<&str>) -> String {
    let verifies_filter = verifies
        .map(|v| {
            let iri = verifies_to_iri(v);
            format!("    FILTER(?verifies = <{iri}>)\n")
        })
        .unwrap_or_default();
    let env_filter = environment
        .map(|e| {
            let iri = format!("{IRI_DEC_ENV_PREFIX}{e}");
            format!("    FILTER(?env = <{iri}>)\n")
        })
        .unwrap_or_default();
    format!(
        "SELECT ?graph ?verifies ?env (COUNT(?step) AS ?count) WHERE {{\n  \
         GRAPH <{graph_iri}> {{\n    \
         ?graph a <{cls}> ;\n         \
         <{p_verifies}> ?verifies ;\n         \
         <{p_env}> ?env .\n\
         {verifies_filter}{env_filter}    \
         OPTIONAL {{ ?graph <{p_steps}>/<{rdf_rest}>*/<{rdf_first}> ?step }}\n  \
         }}\n\
         }} GROUP BY ?graph ?verifies ?env ORDER BY ?graph",
        graph_iri = IRI_DEC_GRAPH_VERIFY_GRAPH,
        cls = verification_graph_class().as_str(),
        p_verifies = verifies_pred().as_str(),
        p_env = environment_pred().as_str(),
        p_steps = steps_pred().as_str(),
        rdf_rest = RDF_REST,
        rdf_first = RDF_FIRST,
    )
}

/// Build the canonical IRI for a `verifies` short id (`FT-NNN`, `TC-NNN`).
/// Unknown shapes are returned as-is so callers can use the result in
/// FILTER clauses without further branching.
fn verifies_to_iri(reference: &str) -> String {
    if reference.starts_with("FT-") {
        return format!("{IRI_FEATURE_PREFIX}{reference}");
    }
    if reference.starts_with("TC-") {
        return format!("{IRI_TC_PREFIX}{reference}");
    }
    reference.to_string()
}

/// Stable sort key for `VG-NNN[-suffix]` ids: the numeric tail comes
/// first so `VG-2` orders before `VG-10`; the original string sorts
/// ids with identical numeric tails deterministically.
#[must_use]
pub(super) fn graph_sort_key(id: &str) -> (u64, String) {
    let tail = id.strip_prefix("VG-").unwrap_or(id);
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n = digits.parse::<u64>().unwrap_or(u64::MAX);
    (n, id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_key_orders_numeric_tail() {
        let mut ids = vec!["VG-002", "VG-010", "VG-001-foo", "VG-007"];
        ids.sort_by_key(|s| graph_sort_key(s));
        assert_eq!(ids, vec!["VG-001-foo", "VG-002", "VG-007", "VG-010"]);
    }

    #[test]
    fn build_query_inlines_verifies_filter() {
        let q = build_query(Some("FT-001"), None);
        assert!(q.contains("FILTER(?verifies = <https://decision-cli.dev/ns/feature/FT-001>)"));
        assert!(!q.contains("FILTER(?env"));
    }

    #[test]
    fn build_query_inlines_tc_verifies_filter() {
        let q = build_query(Some("TC-013"), None);
        assert!(q.contains("FILTER(?verifies = <https://decision-cli.dev/ns/tc/TC-013>)"));
    }

    #[test]
    fn build_query_inlines_env_filter() {
        let q = build_query(None, Some("ENV-001-ephemeral-cli"));
        assert!(q.contains(
            "FILTER(?env = <https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli>)"
        ));
        assert!(!q.contains("FILTER(?verifies"));
    }

    #[test]
    fn build_query_inlines_both_filters() {
        let q = build_query(Some("FT-001"), Some("ENV-001-ephemeral-cli"));
        assert!(q.contains("FILTER(?verifies"));
        assert!(q.contains("FILTER(?env"));
    }

    #[test]
    fn build_query_omits_filters_when_none() {
        let q = build_query(None, None);
        assert!(!q.contains("FILTER"));
    }

    #[test]
    fn build_query_includes_step_count_aggregate() {
        let q = build_query(None, None);
        assert!(q.contains("COUNT(?step)"));
        assert!(q.contains("GROUP BY"));
    }

    #[test]
    fn canonicalize_verifies_strips_feature_prefix() {
        assert_eq!(
            canonicalize_verifies("https://decision-cli.dev/ns/feature/FT-007"),
            "FT-007"
        );
    }

    #[test]
    fn canonicalize_verifies_strips_tc_prefix() {
        assert_eq!(
            canonicalize_verifies("https://decision-cli.dev/ns/tc/TC-013"),
            "TC-013"
        );
    }

    #[test]
    fn canonicalize_verifies_passes_through_unknown() {
        assert_eq!(
            canonicalize_verifies("https://example.com/other"),
            "https://example.com/other"
        );
    }
}
