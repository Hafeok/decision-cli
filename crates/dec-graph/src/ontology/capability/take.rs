//! Field-extraction helpers for the capability reader.

use oxigraph::model::{NamedNode, Quad, Term};

use super::read::CapabilityReadError;

pub(super) fn take_one_str(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<String, CapabilityReadError> {
    let values = literal_strings(quads, subject, predicate);
    match values.len() {
        0 => Err(CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(values.into_iter().next().expect("len() == 1")),
        n => Err(CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

pub(super) fn take_optional_str(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<String>, CapabilityReadError> {
    let values = literal_strings(quads, subject, predicate);
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(values.into_iter().next().expect("len() == 1"))),
        n => Err(CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

pub(super) fn take_one_u32(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<u32, CapabilityReadError> {
    let raw = take_one_str(iri, quads, subject, predicate, label)?;
    raw.parse::<u32>()
        .map_err(|_| CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("{label} must be a non-negative integer, got {raw:?}"),
        })
}

pub(super) fn take_optional_u8(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<Option<u8>, CapabilityReadError> {
    let raw = take_optional_str(iri, quads, subject, predicate)?;
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(s.parse::<u8>().map_err(|_| {
            CapabilityReadError::Malformed {
                iri: iri.to_string(),
                detail: format!("{label} must be a small integer, got {s:?}"),
            }
        })?)),
    }
}

pub(super) fn take_one_bool(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<bool, CapabilityReadError> {
    let raw = take_one_str(iri, quads, subject, predicate, label)?;
    parse_bool(&raw).ok_or_else(|| CapabilityReadError::Malformed {
        iri: iri.to_string(),
        detail: format!("{label} must be xsd:boolean, got {raw:?}"),
    })
}

pub(super) fn take_optional_bool(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<bool>, CapabilityReadError> {
    let raw = take_optional_str(iri, quads, subject, predicate)?;
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(parse_bool(&s).ok_or_else(|| {
            CapabilityReadError::Malformed {
                iri: iri.to_string(),
                detail: format!("{predicate} must be xsd:boolean, got {s:?}"),
            }
        })?)),
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub(super) fn take_optional_iri(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<NamedNode>, CapabilityReadError> {
    let mut iris: Vec<NamedNode> = quads
        .iter()
        .filter(|q| {
            q.predicate.as_str() == predicate
                && matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject)
        })
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    match iris.len() {
        0 => Ok(None),
        1 => Ok(Some(iris.remove(0))),
        n => Err(CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

fn literal_strings(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject) {
                return None;
            }
            match &q.object {
                Term::Literal(lit) => Some(lit.value().to_string()),
                _ => None,
            }
        })
        .collect()
}

pub(super) fn format_sparql_string(value: &str) -> String {
    // Quote and escape the value for use in a SPARQL literal expression.
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
