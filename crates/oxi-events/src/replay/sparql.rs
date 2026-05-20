//! SPARQL row builder/decoder helpers used by the replay API.

use std::fmt::Write as _;

use crate::error::ReplayError;
use crate::vocab::{
    IRI_OXI_EMITTED_AT, IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS, IRI_OXI_MATCHED_SUBSCRIPTION,
    IRI_OXI_PUBLISHED, IRI_OXI_SEQ, IRI_OXI_STATUS, IRI_PROV_WAS_GENERATED_BY,
};

use super::ReplayRequest;

pub(super) fn build_query(request: &ReplayRequest) -> String {
    let mut q = String::with_capacity(512);
    push_select_header(&mut q);
    push_where_pattern(&mut q);
    push_filters(&mut q, request);
    q.push_str("} ORDER BY ?seq");
    if let Some(limit) = request.limit {
        let _ = write!(q, " LIMIT {limit}");
    }
    q
}

/// SELECT projection plus FROM clause naming the events graph.
fn push_select_header(q: &mut String) {
    q.push_str("SELECT ?event ?seq ?mutation ?subscription ?emittedAt ?published ?status FROM <");
    q.push_str(IRI_OXI_GRAPH_EVENTS);
    q.push_str("> WHERE { ?event a <");
    q.push_str(IRI_OXI_EVENT);
    q.push_str("> ; <");
}

/// Predicate triples binding the projected variables onto each event.
fn push_where_pattern(q: &mut String) {
    q.push_str(IRI_OXI_SEQ);
    q.push_str("> ?seq ; <");
    q.push_str(IRI_PROV_WAS_GENERATED_BY);
    q.push_str("> ?mutation ; <");
    q.push_str(IRI_OXI_MATCHED_SUBSCRIPTION);
    q.push_str("> ?subscription ; <");
    q.push_str(IRI_OXI_EMITTED_AT);
    q.push_str("> ?emittedAt ; <");
    q.push_str(IRI_OXI_PUBLISHED);
    q.push_str("> ?published ; <");
    q.push_str(IRI_OXI_STATUS);
    q.push_str("> ?status .");
}

/// Optional seq-window plus caller-supplied SPARQL FILTER fragment.
fn push_filters(q: &mut String, request: &ReplayRequest) {
    if request.since_seq > 0 {
        let _ = write!(q, " FILTER(?seq >= {}) ", request.since_seq);
    }
    if let Some(until) = request.until_seq {
        let _ = write!(q, " FILTER(?seq <= {until}) ");
    }
    if let Some(filter) = request.filter.as_ref() {
        q.push_str(" FILTER(");
        q.push_str(filter.as_str());
        q.push_str(") ");
    }
}

pub(super) fn named_node_iri(
    term: &oxigraph::model::Term,
    name: &str,
) -> Result<String, ReplayError> {
    match term {
        oxigraph::model::Term::NamedNode(n) => Ok(n.as_str().to_string()),
        _ => Err(ReplayError::Internal(format!(
            "replay row ?{name} is not a named node"
        ))),
    }
}

pub(super) fn literal_value(
    term: &oxigraph::model::Term,
    name: &str,
) -> Result<String, ReplayError> {
    match term {
        oxigraph::model::Term::Literal(lit) => Ok(lit.value().to_string()),
        _ => Err(ReplayError::Internal(format!(
            "replay row ?{name} is not a literal"
        ))),
    }
}

pub(super) fn parse_u64_literal(
    term: &oxigraph::model::Term,
    name: &str,
) -> Result<u64, ReplayError> {
    let s = literal_value(term, name)?;
    s.parse::<u64>()
        .map_err(|e| ReplayError::Internal(format!("replay row ?{name} parse u64: {e}")))
}

pub(super) fn parse_bool_literal(
    term: &oxigraph::model::Term,
    name: &str,
) -> Result<bool, ReplayError> {
    let s = literal_value(term, name)?;
    match s.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(ReplayError::Internal(format!(
            "replay row ?{name} unrecognised boolean literal: {other}"
        ))),
    }
}
