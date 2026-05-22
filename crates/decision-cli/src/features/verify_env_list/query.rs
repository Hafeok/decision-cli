//! SPARQL projection for `dec verify env list` (FT-039).
//!
//! Reads the verify-env named graph that `dec init` and `dec verify env
//! new` write into. The query applies filter predicates server-side so
//! the handler ships only matching rows back to the caller.
//!
//! `dec:allowedOps` is encoded as an rdf:List of blank-node cells.
//! SPARQL `WHERE` treats `_:foo` as an existential variable rather
//! than a specific identifier, so walking the list goes through
//! `quads_for_pattern` (which preserves blank-node identity) rather
//! than SPARQL — mirroring `core::ontology::verification_env::io`.

use std::path::Path;

use oxigraph::model::{NamedNode, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::handler::Error as HandlerError;
use crate::core::sparql::{term_iri_string, term_literal_string};
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::vocab::{
    IRI_DEC_ALLOWED_OPS, IRI_DEC_ENDPOINT, IRI_DEC_ENV_PREFIX, IRI_DEC_ENV_TYPE,
    IRI_DEC_FIXTURE_SOURCE, IRI_DEC_GRAPH_VERIFY_ENV, IRI_DEC_SAFETY_CLASS, IRI_DEC_SETUP,
    IRI_DEC_TEARDOWN, IRI_DEC_VERIFICATION_ENVIRONMENT,
};

use super::{EnvRowError, EnvSummary};

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Load the orchestration store and project env summaries via SPARQL.
///
/// Per-env failures (corrupt rdf:List, multiple `dec:allowedOps` heads,
/// schema drift) DO NOT abort the listing — they surface as
/// [`EnvSummary::error`] markers so an operator can triage from the
/// list itself (TC-096). Only failures that prevent us from enumerating
/// any envs at all (store unreadable, SPARQL planner error) propagate as
/// [`HandlerError`].
pub(super) fn query_envs(
    workdir: &Path,
    safety_class: Option<&str>,
    env_type: Option<&str>,
) -> Result<Vec<EnvSummary>, HandlerError> {
    let store = open_store(workdir)?;
    let q = build_query(safety_class, env_type);
    let results = store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("env-list SPARQL: {e}"),
        })?;
    let QueryResults::Solutions(sols) = results else {
        return Err(HandlerError::Internal {
            detail: "env-list: unexpected SPARQL result shape".to_string(),
        });
    };
    let mut out: Vec<EnvSummary> = Vec::new();
    for sol_res in sols {
        let sol = sol_res.map_err(|e| HandlerError::Internal {
            detail: format!("env-list SPARQL row: {e}"),
        })?;
        let env_iri = sol.get("env").map(term_iri_string).unwrap_or_default();
        let Some(id) = env_iri.strip_prefix(IRI_DEC_ENV_PREFIX).map(str::to_string) else {
            continue;
        };
        let (allowed_ops, row_error) = read_allowed_ops_resilient(&store, &env_iri);
        out.push(summary_from_solution(id, &sol, allowed_ops, row_error));
    }
    Ok(out)
}

/// Load the orchestration store dump, returning a structured error when
/// the dump is missing (typical pre-`dec init` state).
fn open_store(workdir: &Path) -> Result<Store, HandlerError> {
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

/// Project a single SPARQL solution row into an [`EnvSummary`]. The
/// `allowed_ops` list is read separately because it lives as an rdf:List
/// of blank nodes that SPARQL cannot bind through `WHERE`. A non-`None`
/// `row_error` flags the row as corrupt; the renderer surfaces the
/// marker in place of a fully-populated `allowed_ops` field.
fn summary_from_solution(
    id: String,
    sol: &oxigraph::sparql::QuerySolution,
    allowed_ops: Vec<String>,
    row_error: Option<EnvRowError>,
) -> EnvSummary {
    let env_type = sol.get("type").map(term_literal_string).unwrap_or_default();
    let safety_class = sol
        .get("safety")
        .map(term_literal_string)
        .unwrap_or_default();
    let endpoint = optional_literal(sol, "endpoint");
    let setup = optional_literal(sol, "setup");
    let teardown = optional_literal(sol, "teardown");
    let fixture_source = optional_literal(sol, "fixture_source");
    EnvSummary {
        id,
        env_type,
        safety_class,
        endpoint,
        allowed_ops,
        setup,
        teardown,
        fixture_source,
        error: row_error,
    }
}

/// Read an optional literal binding, mapping the empty string to `None`
/// so absent OPTIONAL columns and explicit empty strings collapse the same way.
fn optional_literal(sol: &oxigraph::sparql::QuerySolution, name: &str) -> Option<String> {
    sol.get(name)
        .map(term_literal_string)
        .filter(|s| !s.is_empty())
}

/// SPARQL SELECT for env summaries with optional filters.
///
/// The FILTER predicates collapse to no-ops when the corresponding
/// option is `None`; conjunctive when both are `Some`.
fn build_query(safety_class: Option<&str>, env_type: Option<&str>) -> String {
    let safety_filter = filter_clause("safety", safety_class);
    let type_filter = filter_clause("type", env_type);
    format!(
        "SELECT ?env ?type ?safety ?endpoint ?setup ?teardown ?fixture_source WHERE {{\n  \
         GRAPH <{graph}> {{\n    \
         ?env a <{cls}> ;\n         \
         <{p_type}> ?type ;\n         \
         <{p_safety}> ?safety .\n\
         {safety_filter}{type_filter}    \
         OPTIONAL {{ ?env <{p_endpoint}> ?endpoint }}\n    \
         OPTIONAL {{ ?env <{p_setup}> ?setup }}\n    \
         OPTIONAL {{ ?env <{p_teardown}> ?teardown }}\n    \
         OPTIONAL {{ ?env <{p_fixture_source}> ?fixture_source }}\n  \
         }}\n\
         }} ORDER BY ?env",
        graph = IRI_DEC_GRAPH_VERIFY_ENV,
        cls = IRI_DEC_VERIFICATION_ENVIRONMENT,
        p_type = IRI_DEC_ENV_TYPE,
        p_safety = IRI_DEC_SAFETY_CLASS,
        p_endpoint = IRI_DEC_ENDPOINT,
        p_setup = IRI_DEC_SETUP,
        p_teardown = IRI_DEC_TEARDOWN,
        p_fixture_source = IRI_DEC_FIXTURE_SOURCE,
    )
}

/// Build a single `FILTER(?<var> = "...")` clause when the value is
/// `Some`, or the empty string otherwise (so the FILTER is omitted).
fn filter_clause(var: &str, value: Option<&str>) -> String {
    value
        .map(|v| {
            format!(
                "    FILTER(?{var} = \"{escaped}\")\n",
                escaped = escape_sparql_literal(v)
            )
        })
        .unwrap_or_default()
}

fn escape_sparql_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Pull the rdf:List value bound to `?env <dec:allowedOps> ...` for the
/// given env IRI, tolerating per-env corruption.
///
/// Returns `(ops, error)` where `error` is `None` for healthy envs.
/// Corruption modes flagged here (TC-096):
///
/// - **Multiple `dec:allowedOps` heads** — usually a write-path bug or a
///   manual store edit. We pick the first head deterministically (sorted
///   by term key) and return its walk alongside a
///   `MultipleAllowedOpsHeads` marker. The walk's result is best-effort
///   data; consumers should treat the `error` field as authoritative.
/// - **Cyclic list / missing rdf:first or rdf:rest / unsupported term
///   shape** — surfaced as a generic `Corrupt` marker with the detail
///   string from the underlying walker.
/// - **Malformed env IRI** — surfaced as `Corrupt`.
fn read_allowed_ops_resilient(store: &Store, env_iri: &str) -> (Vec<String>, Option<EnvRowError>) {
    let env_node = match NamedNode::new(env_iri) {
        Ok(n) => n,
        Err(e) => {
            return (
                Vec::new(),
                Some(EnvRowError::corrupt(format!(
                    "malformed env iri {env_iri:?}: {e}"
                ))),
            );
        }
    };
    let env_subject = Subject::NamedNode(env_node);
    let ops_pred = NamedNode::new_unchecked(IRI_DEC_ALLOWED_OPS);
    let mut heads: Vec<Term> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(env_subject.as_ref()),
            Some(ops_pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        heads.push(quad.object);
    }
    if heads.is_empty() {
        return (Vec::new(), None);
    }
    if heads.len() > 1 {
        // Best-effort: walk the deterministically-first head so the
        // operator still sees *something* alongside the error marker.
        let count = heads.len();
        heads.sort_by_key(key_for);
        let first = heads.remove(0);
        let best_effort = walk_list(store, first).unwrap_or_default();
        return (
            best_effort,
            Some(EnvRowError::multiple_allowed_ops_heads(count)),
        );
    }
    match walk_list(store, heads.remove(0)) {
        Ok(ops) => (ops, None),
        Err(detail) => (Vec::new(), Some(EnvRowError::corrupt(detail))),
    }
}

/// Walk an rdf:List starting at `head_term`. Each iteration pulls
/// `rdf:first` and `rdf:rest`. A cycle guard caps lookups so a
/// malformed store never loops forever.
///
/// Returns the parsed ops on success; on per-env corruption, returns
/// the failure mode as a human-readable detail string — the caller
/// wraps it in an [`EnvRowError`].
fn walk_list(store: &Store, head_term: Term) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = head_term;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for _ in 0..1024 {
        if let Term::NamedNode(n) = &current {
            if n.as_str() == RDF_NIL {
                return Ok(out);
            }
        }
        let key = key_for(&current);
        if !seen.insert(key) {
            return Err("dec:allowedOps list is cyclic".to_string());
        }
        let (first, rest) = step_list(store, &current)?;
        out.push(first);
        current = rest;
    }
    Err("dec:allowedOps list exceeds 1024 elements".to_string())
}

fn key_for(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => format!("iri:{}", n.as_str()),
        Term::BlankNode(b) => format!("bn:{}", b.as_str()),
        _ => format!("other:{t}"),
    }
}

/// Read `rdf:first` (literal) and `rdf:rest` (term) for one list cell.
fn step_list(store: &Store, head: &Term) -> Result<(String, Term), String> {
    let head_subject = term_as_subject(head)?;
    let first_value = read_first_literal(store, &head_subject)?;
    let rest_term = read_rest_term(store, &head_subject)?;
    Ok((first_value, rest_term))
}

/// Pull the `rdf:first` literal for an rdf:List cell.
fn read_first_literal(store: &Store, head: &Subject) -> Result<String, String> {
    let first_pred = NamedNode::new_unchecked(RDF_FIRST);
    for quad in store
        .quads_for_pattern(Some(head.as_ref()), Some(first_pred.as_ref()), None, None)
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = &quad.object {
            return Ok(lit.value().to_string());
        }
    }
    Err("dec:allowedOps list node missing rdf:first literal".to_string())
}

/// Pull the `rdf:rest` term (named/blank node) for an rdf:List cell.
fn read_rest_term(store: &Store, head: &Subject) -> Result<Term, String> {
    let rest_pred = NamedNode::new_unchecked(RDF_REST);
    for quad in store
        .quads_for_pattern(Some(head.as_ref()), Some(rest_pred.as_ref()), None, None)
        .filter_map(Result::ok)
    {
        return Ok(quad.object);
    }
    Err("dec:allowedOps list node missing rdf:rest".to_string())
}

fn term_as_subject(t: &Term) -> Result<Subject, String> {
    match t {
        Term::NamedNode(n) => Ok(Subject::NamedNode(n.clone())),
        Term::BlankNode(b) => Ok(Subject::BlankNode(b.clone())),
        _ => Err("rdf:List node must be IRI or blank node".to_string()),
    }
}

/// Stable sort key for `ENV-NNN[-suffix]` ids: the numeric tail comes
/// first so `ENV-2` orders before `ENV-10`; the original string sorts
/// ids with identical numeric tails deterministically.
#[must_use]
pub(super) fn env_sort_key(id: &str) -> (u64, String) {
    let tail = id.strip_prefix("ENV-").unwrap_or(id);
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n = digits.parse::<u64>().unwrap_or(u64::MAX);
    (n, id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_key_orders_numeric_tail() {
        let mut ids = vec!["ENV-002", "ENV-010", "ENV-001-foo", "ENV-007"];
        ids.sort_by_key(|s| env_sort_key(s));
        assert_eq!(ids, vec!["ENV-001-foo", "ENV-002", "ENV-007", "ENV-010"]);
    }

    #[test]
    fn build_query_inlines_safety_filter() {
        let q = build_query(Some("isolated"), None);
        assert!(q.contains("FILTER(?safety = \"isolated\")"));
        assert!(!q.contains("FILTER(?type"));
    }

    #[test]
    fn build_query_inlines_both_filters() {
        let q = build_query(Some("shared-non-destructive"), Some("remote-http"));
        assert!(q.contains("FILTER(?safety = \"shared-non-destructive\")"));
        assert!(q.contains("FILTER(?type = \"remote-http\")"));
    }

    #[test]
    fn build_query_omits_filters_when_none() {
        let q = build_query(None, None);
        assert!(!q.contains("FILTER"));
    }

    #[test]
    fn escape_sparql_literal_handles_quotes() {
        assert_eq!(escape_sparql_literal("a\"b"), "a\\\"b");
        assert_eq!(escape_sparql_literal("a\\b"), "a\\\\b");
    }
}
