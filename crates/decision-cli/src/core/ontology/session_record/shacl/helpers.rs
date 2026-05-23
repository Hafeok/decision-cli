//! Helpers shared by the FT-057 SHACL check modules.

use oxigraph::model::{NamedNode, Quad, Subject, Term};

/// One SHACL violation against a candidate session-record mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecordViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

pub(super) fn iri_objects_for(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            match &q.subject {
                Subject::NamedNode(s) if s == subject => match &q.object {
                    Term::NamedNode(n) => Some(n.as_str().to_string()),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

pub(super) fn literal_objects_for(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            match &q.subject {
                Subject::NamedNode(s) if s == subject => match &q.object {
                    Term::Literal(lit) => Some(lit.value().to_string()),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

pub(super) fn render_violations(violations: &[SessionRecordViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in violations {
        let _ = writeln!(
            out,
            "  • subject <{}> path <{}>: {}",
            v.subject, v.path, v.detail
        );
    }
    out
}

pub(super) fn violation(subject: &NamedNode, path: &str, detail: &str) -> SessionRecordViolation {
    SessionRecordViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}
