//! rdf:List traversal helpers used by the verification-env SHACL checks.

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Quad, Term};

use crate::core::vocab::IRI_DEC_ALLOWED_OPS;

use super::types::{RDF_FIRST, RDF_NIL, RDF_REST};

/// Collect every `dec:allowedOps` object for `subject`, as the term that
/// names the rdf:List head (a blank node or the IRI `rdf:nil`).
pub(super) fn allowed_ops_heads(quads: &[Quad], subject: &NamedNode) -> Vec<Term> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != IRI_DEC_ALLOWED_OPS {
                return None;
            }
            if !subject_matches_iri(q, subject) {
                return None;
            }
            Some(q.object.clone())
        })
        .collect()
}

pub(super) fn list_is_nil(head: &Term) -> bool {
    matches!(head, Term::NamedNode(n) if n.as_str() == RDF_NIL)
}

/// Walk an rdf:List starting at `head` and collect literal values.
pub(super) fn walk_list(quads: &[Quad], head: &Term) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = head.clone();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    loop {
        let key = term_key(&current);
        if visited.contains(&key) {
            return out;
        }
        visited.insert(key);
        let first = find_list_value(quads, &current, RDF_FIRST);
        if let Some(Term::Literal(lit)) = first {
            out.push(lit.value().to_string());
        }
        let rest = find_list_value(quads, &current, RDF_REST);
        match rest {
            Some(Term::NamedNode(n)) if n.as_str() == RDF_NIL => return out,
            Some(t) => current = t,
            None => return out,
        }
    }
}

fn find_list_value(quads: &[Quad], head: &Term, predicate: &str) -> Option<Term> {
    quads
        .iter()
        .find(|q| {
            if q.predicate.as_str() != predicate {
                return false;
            }
            term_matches_subject(&q.subject, head)
        })
        .map(|q| q.object.clone())
}

fn term_matches_subject(s: &oxigraph::model::Subject, t: &Term) -> bool {
    match (s, t) {
        (oxigraph::model::Subject::NamedNode(a), Term::NamedNode(b)) => a == b,
        (oxigraph::model::Subject::BlankNode(a), Term::BlankNode(b)) => a == b,
        _ => false,
    }
}

fn term_key(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => format!("iri:{}", n.as_str()),
        Term::BlankNode(b) => format!("bn:{}", b.as_str()),
        Term::Literal(l) => format!("lit:{}", l.value()),
        Term::Triple(_) => "triple".to_string(),
    }
}

fn subject_matches_iri(q: &Quad, subject: &NamedNode) -> bool {
    match &q.subject {
        oxigraph::model::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}
