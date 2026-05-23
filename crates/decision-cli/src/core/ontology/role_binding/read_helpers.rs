//! Shared helpers for `dec:RoleBinding` SPARQL reads.

use oxigraph::model::{NamedNode, Quad, Subject, Term};

use super::read::RoleBindingReadError;

pub(super) fn take_one_str(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<String, RoleBindingReadError> {
    let values = literal_strings(quads, subject, predicate);
    match values.len() {
        0 => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(values.into_iter().next().expect("len() == 1")),
        n => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

pub(super) fn take_optional_str(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<String>, RoleBindingReadError> {
    let values = literal_strings(quads, subject, predicate);
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(values.into_iter().next().expect("len() == 1"))),
        n => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

pub(super) fn take_one_iri(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<NamedNode, RoleBindingReadError> {
    let mut iris = collect_iris(quads, subject, predicate);
    match iris.len() {
        0 => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(iris.remove(0)),
        n => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

pub(super) fn take_optional_iri(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<NamedNode>, RoleBindingReadError> {
    let mut iris = collect_iris(quads, subject, predicate);
    match iris.len() {
        0 => Ok(None),
        1 => Ok(Some(iris.remove(0))),
        n => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

pub(super) fn take_one_bool(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<bool, RoleBindingReadError> {
    let raw = take_one_str(iri, quads, subject, predicate, label)?;
    match raw.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("{label} must be xsd:boolean, got {raw:?}"),
        }),
    }
}

pub(super) fn take_one_u32(
    iri: &NamedNode,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<u32, RoleBindingReadError> {
    let raw = take_one_str(iri, quads, subject, predicate, label)?;
    raw.parse::<u32>()
        .map_err(|_| RoleBindingReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("{label} must be a non-negative integer, got {raw:?}"),
        })
}

fn literal_strings(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !matches!(&q.subject, Subject::NamedNode(s) if s == subject) {
                return None;
            }
            match &q.object {
                Term::Literal(lit) => Some(lit.value().to_string()),
                _ => None,
            }
        })
        .collect()
}

fn collect_iris(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    quads
        .iter()
        .filter(|q| {
            q.predicate.as_str() == predicate
                && matches!(&q.subject, Subject::NamedNode(s) if s == subject)
        })
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn format_sparql_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
