//! Shared helpers for capability SHACL validation.

use oxigraph::model::{NamedNode, Quad, Term};

use super::CapabilityViolation;

pub(super) fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !subject_matches(q, subject) {
                return None;
            }
            match &q.object {
                Term::Literal(lit) => Some(lit.value().to_string()),
                _ => None,
            }
        })
        .collect()
}

pub(super) fn iri_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !subject_matches(q, subject) {
                return None;
            }
            match &q.object {
                Term::NamedNode(n) => Some(n.as_str().to_string()),
                _ => None,
            }
        })
        .collect()
}

fn subject_matches(q: &Quad, subject: &NamedNode) -> bool {
    match &q.subject {
        oxigraph::model::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

pub(super) fn violation(subject: &NamedNode, path: &str, detail: &str) -> CapabilityViolation {
    CapabilityViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}
