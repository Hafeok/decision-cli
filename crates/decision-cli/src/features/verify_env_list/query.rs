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
    IRI_DEC_GRAPH_VERIFY_ENV, IRI_DEC_SAFETY_CLASS, IRI_DEC_SETUP, IRI_DEC_TEARDOWN,
    IRI_DEC_VERIFICATION_ENVIRONMENT,
};

use super::EnvSummary;

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Load the orchestration store and project env summaries via SPARQL.
pub(super) fn query_envs(
    workdir: &Path,
    safety_class: Option<&str>,
    env_type: Option<&str>,
) -> Result<Vec<EnvSummary>, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Err(HandlerError::Internal {
            detail: format!(
                "no orchestration store at {} — run `dec init` first",
                dump_path.display()
            ),
        });
    }
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    let q = build_query(safety_class, env_type);
    let results = store.query(q.as_str()).map_err(|e| HandlerError::Internal {
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
        let env_type_v = sol.get("type").map(term_literal_string).unwrap_or_default();
        let safety = sol.get("safety").map(term_literal_string).unwrap_or_default();
        let endpoint = sol
            .get("endpoint")
            .map(term_literal_string)
            .filter(|s| !s.is_empty());
        let setup = sol
            .get("setup")
            .map(term_literal_string)
            .filter(|s| !s.is_empty());
        let teardown = sol
            .get("teardown")
            .map(term_literal_string)
            .filter(|s| !s.is_empty());
        let allowed_ops = read_allowed_ops(&store, &env_iri)?;
        out.push(EnvSummary {
            id,
            env_type: env_type_v,
            safety_class: safety,
            endpoint,
            allowed_ops,
            setup,
            teardown,
        });
    }
    Ok(out)
}

/// SPARQL SELECT for env summaries with optional filters.
///
/// The FILTER predicates collapse to no-ops when the corresponding
/// option is `None`; conjunctive when both are `Some`.
fn build_query(safety_class: Option<&str>, env_type: Option<&str>) -> String {
    let safety_filter = safety_class
        .map(|s| {
            format!(
                "    FILTER(?safety = \"{escaped}\")\n",
                escaped = escape_sparql_literal(s)
            )
        })
        .unwrap_or_default();
    let type_filter = env_type
        .map(|t| {
            format!(
                "    FILTER(?type = \"{escaped}\")\n",
                escaped = escape_sparql_literal(t)
            )
        })
        .unwrap_or_default();
    format!(
        "SELECT ?env ?type ?safety ?endpoint ?setup ?teardown WHERE {{\n  \
         GRAPH <{graph}> {{\n    \
         ?env a <{cls}> ;\n         \
         <{p_type}> ?type ;\n         \
         <{p_safety}> ?safety .\n\
         {safety_filter}{type_filter}    \
         OPTIONAL {{ ?env <{p_endpoint}> ?endpoint }}\n    \
         OPTIONAL {{ ?env <{p_setup}> ?setup }}\n    \
         OPTIONAL {{ ?env <{p_teardown}> ?teardown }}\n  \
         }}\n\
         }} ORDER BY ?env",
        graph = IRI_DEC_GRAPH_VERIFY_ENV,
        cls = IRI_DEC_VERIFICATION_ENVIRONMENT,
        p_type = IRI_DEC_ENV_TYPE,
        p_safety = IRI_DEC_SAFETY_CLASS,
        p_endpoint = IRI_DEC_ENDPOINT,
        p_setup = IRI_DEC_SETUP,
        p_teardown = IRI_DEC_TEARDOWN,
    )
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
/// given env IRI. The list nodes are blank nodes; we walk them with
/// `quads_for_pattern` so blank-node identity is preserved (SPARQL
/// `WHERE` would treat them as fresh existential variables).
fn read_allowed_ops(store: &Store, env_iri: &str) -> Result<Vec<String>, HandlerError> {
    let env_node = NamedNode::new(env_iri).map_err(|e| HandlerError::Internal {
        detail: format!("malformed env iri {env_iri:?}: {e}"),
    })?;
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
        return Ok(Vec::new());
    }
    if heads.len() > 1 {
        return Err(HandlerError::Internal {
            detail: format!(
                "env {env_iri:?} declares {n} dec:allowedOps heads",
                n = heads.len()
            ),
        });
    }
    walk_list(store, heads.remove(0))
}

/// Walk an rdf:List starting at `head_term`. Each iteration pulls
/// `rdf:first` and `rdf:rest`. A cycle guard caps lookups so a
/// malformed store never loops forever.
fn walk_list(store: &Store, head_term: Term) -> Result<Vec<String>, HandlerError> {
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
            return Err(HandlerError::Internal {
                detail: "dec:allowedOps list is cyclic".to_string(),
            });
        }
        let (first, rest) = step_list(store, &current)?;
        out.push(first);
        current = rest;
    }
    Err(HandlerError::Internal {
        detail: "dec:allowedOps list exceeds 1024 elements".to_string(),
    })
}

fn key_for(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => format!("iri:{}", n.as_str()),
        Term::BlankNode(b) => format!("bn:{}", b.as_str()),
        _ => format!("other:{t}"),
    }
}

/// Read `rdf:first` (literal) and `rdf:rest` (term) for one list cell.
fn step_list(store: &Store, head: &Term) -> Result<(String, Term), HandlerError> {
    let head_subject = term_as_subject(head)?;
    let first_pred = NamedNode::new_unchecked(RDF_FIRST);
    let rest_pred = NamedNode::new_unchecked(RDF_REST);
    let mut first_value: Option<String> = None;
    for quad in store
        .quads_for_pattern(
            Some(head_subject.as_ref()),
            Some(first_pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = &quad.object {
            first_value = Some(lit.value().to_string());
            break;
        }
    }
    let Some(first_value) = first_value else {
        return Err(HandlerError::Internal {
            detail: "dec:allowedOps list node missing rdf:first literal".to_string(),
        });
    };
    let mut rest_term: Option<Term> = None;
    for quad in store
        .quads_for_pattern(
            Some(head_subject.as_ref()),
            Some(rest_pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        rest_term = Some(quad.object);
        break;
    }
    let Some(rest_term) = rest_term else {
        return Err(HandlerError::Internal {
            detail: "dec:allowedOps list node missing rdf:rest".to_string(),
        });
    };
    Ok((first_value, rest_term))
}

fn term_as_subject(t: &Term) -> Result<Subject, HandlerError> {
    match t {
        Term::NamedNode(n) => Ok(Subject::NamedNode(n.clone())),
        Term::BlankNode(b) => Ok(Subject::BlankNode(b.clone())),
        _ => Err(HandlerError::Internal {
            detail: "rdf:List node must be IRI or blank node".to_string(),
        }),
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
