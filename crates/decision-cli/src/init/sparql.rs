//! SPARQL helpers used by the init pipeline.

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::vocab::{RDF_FIRST, RDF_NIL, RDF_REST};

pub(super) fn sole_subject_with_class(
    store: &Store,
    graph: &NamedNode,
    class: &str,
) -> Option<String> {
    let q = format!(
        "SELECT ?s WHERE {{ GRAPH <{g}> {{ ?s a <{cls}> }} }} LIMIT 2",
        g = graph.as_str(),
        cls = class,
    );
    let res = store.query(q.as_str()).ok()?;
    let QueryResults::Solutions(mut sols) = res else {
        return None;
    };
    let first = sols.next()?.ok()?;
    if sols.next().is_some() {
        return None;
    }
    if let Some(Term::NamedNode(n)) = first.get("s") {
        Some(n.as_str().to_string())
    } else {
        None
    }
}

pub(super) fn collect_property_values(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    prop: &str,
) -> Vec<Term> {
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH <{g}> {{ <{s}> <{p}> ?o }} }}",
        g = graph.as_str(),
        s = subject_iri,
        p = prop,
    );
    let mut out = Vec::new();
    let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) else {
        return out;
    };
    for sol in sols {
        let Ok(sol) = sol else { continue };
        if let Some(t) = sol.get("o") {
            out.push(t.clone());
        }
    }
    out
}

pub(super) fn single_iri_value(
    store: &Store,
    subject_iri: &str,
    prop: &str,
) -> Result<String, String> {
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH ?g {{ <{s}> <{p}> ?o }} }}",
        s = subject_iri,
        p = prop,
    );
    let res = store
        .query(q.as_str())
        .map_err(|e| format!("internal SPARQL error: {e}"))?;
    let QueryResults::Solutions(sols) = res else {
        return Err(format!("SPARQL for {prop} returned non-solutions"));
    };
    let mut iris = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| format!("internal SPARQL error: {e}"))?;
        if let Some(Term::NamedNode(n)) = sol.get("o") {
            iris.push(n.as_str().to_string());
        }
    }
    match iris.len() {
        0 => Err(format!("no IRI value for {prop} on {subject_iri}")),
        1 => Ok(iris.remove(0)),
        n => Err(format!("expected exactly one IRI for {prop}, found {n}")),
    }
}

/// Collect plain-string property values, accepting either repeated
/// triples or an RDF collection (Turtle list).
pub(super) fn collect_string_property(store: &Store, subject_iri: &str, prop: &str) -> Vec<String> {
    let mut out = Vec::new();
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH ?g {{ <{s}> <{p}> ?o }} }}",
        s = subject_iri,
        p = prop,
    );
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(t) = sol.get("o") {
                match t {
                    Term::Literal(lit) => out.push(lit.value().to_string()),
                    Term::BlankNode(b) => {
                        let head_iri = format!("_:{}", b.as_str());
                        walk_list(store, &head_iri, &mut out);
                    }
                    Term::NamedNode(n) => {
                        if n.as_str() != RDF_NIL {
                            walk_list(store, n.as_str(), &mut out);
                        }
                    }
                    Term::Triple(_) => {}
                }
            }
        }
    }
    out
}

fn walk_list(store: &Store, head: &str, out: &mut Vec<String>) {
    let mut current = head.to_string();
    loop {
        if current == RDF_NIL {
            return;
        }
        let first_q = list_query(&current, RDF_FIRST);
        let mut found_value = false;
        if let Ok(QueryResults::Solutions(sols)) = store.query(first_q.as_str()) {
            for sol in sols.flatten() {
                if let Some(Term::Literal(lit)) = sol.get("v") {
                    out.push(lit.value().to_string());
                    found_value = true;
                }
            }
        }
        if !found_value {
            return;
        }
        let rest_q = list_query(&current, RDF_REST);
        let mut next = String::new();
        if let Ok(QueryResults::Solutions(sols)) = store.query(rest_q.as_str()) {
            for sol in sols.flatten() {
                if let Some(term) = sol.get("v") {
                    match term {
                        Term::NamedNode(n) => next = n.as_str().to_string(),
                        Term::BlankNode(b) => next = format!("_:{}", b.as_str()),
                        _ => {}
                    }
                }
            }
        }
        if next.is_empty() || next == current {
            return;
        }
        current = next;
    }
}

fn list_query(current: &str, predicate: &str) -> String {
    if let Some(id) = current.strip_prefix("_:") {
        format!("SELECT ?v WHERE {{ GRAPH ?g {{ _:{id} <{predicate}> ?v }} }}")
    } else {
        format!("SELECT ?v WHERE {{ GRAPH ?g {{ <{current}> <{predicate}> ?v }} }}")
    }
}

pub(super) fn term_kind(t: &Term) -> &'static str {
    match t {
        Term::NamedNode(_) => "IRI",
        Term::BlankNode(_) => "blank node",
        Term::Literal(_) => "literal",
        Term::Triple(_) => "embedded triple",
    }
}
