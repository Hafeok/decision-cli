//! Capability endpoint resolution — from inline quads and the store.

use std::collections::HashMap;

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::vocab::{
    IRI_DEC_CAPABILITY, IRI_DEC_CAPABILITY_ENDPOINT, IRI_DEC_SESSION_CAPABILITY,
};

use super::super::types::RDF_TYPE;
use super::helpers::iri_objects_for;

/// Walk `quads` and return a map from capability IRI to endpoint literal
/// for every Capability subject declared inline.
pub(super) fn capability_endpoints(quads: &[Quad]) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let mut capabilities: Vec<String> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_CAPABILITY {
            continue;
        }
        if let Subject::NamedNode(s) = &q.subject {
            capabilities.push(s.as_str().to_string());
        }
    }
    for q in quads {
        if q.predicate.as_str() != IRI_DEC_CAPABILITY_ENDPOINT {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if !capabilities.iter().any(|c| c == s.as_str()) {
            continue;
        }
        if let Term::Literal(lit) = &q.object {
            out.insert(s.as_str().to_string(), lit.value().to_string());
        }
    }
    out
}

/// For every session subject whose capability isn't in `endpoints` yet,
/// look up the endpoint in the persistent store. Missing capabilities
/// are silently skipped (the constraint becomes advisory).
pub(super) fn merge_store_endpoints(
    quads: &[Quad],
    subjects: &[NamedNode],
    endpoints: &mut HashMap<String, String>,
    store: &Store,
) {
    for subject in subjects {
        let Some(cap_iri) = iri_objects_for(quads, subject, IRI_DEC_SESSION_CAPABILITY)
            .into_iter()
            .next()
        else {
            continue;
        };
        if endpoints.contains_key(&cap_iri) {
            continue;
        }
        let q = format!(
            "SELECT ?ep WHERE {{ \
              {{ <{cap}> <{pred}> ?ep }} \
              UNION \
              {{ GRAPH ?g {{ <{cap}> <{pred}> ?ep }} }} \
            }} LIMIT 1",
            cap = cap_iri,
            pred = IRI_DEC_CAPABILITY_ENDPOINT,
        );
        if let Ok(QueryResults::Solutions(mut sols)) = store.query(q.as_str()) {
            if let Some(Ok(sol)) = sols.next() {
                if let Some(Term::Literal(lit)) = sol.get("ep") {
                    endpoints.insert(cap_iri, lit.value().to_string());
                }
            }
        }
    }
}
