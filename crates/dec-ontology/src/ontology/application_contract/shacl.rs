//! Write-side SHACL validation for `dec:ApplicationContract` (FT-148 /
//! ADR-082 §3).
//!
//! Rust-side mirror of `shapes/application_contract.shacl.ttl`, invoked
//! by the dec-graph chokepoint. The six required Convention links must
//! be present and each linked Convention must be well-formed (name,
//! body_path, checkable). A `checkable: false` Convention is valid —
//! its dispatchability consequence propagates downstream (FT-150/153).

use std::collections::BTreeMap;

use oxrdf::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::vocab::{
    IRI_DEC_APPLICATION_CONTRACT_CLASS, IRI_DEC_CONTRACT_ARCHETYPE, IRI_DEC_CONVENTION_BODY_PATH,
    IRI_DEC_CONVENTION_CHECKABLE, IRI_DEC_CONVENTION_NAME, IRI_DEC_ENDPOINT_CONVENTION,
    IRI_DEC_FEATURE_ORGANISATION, IRI_DEC_LANGUAGE_RUNTIME, IRI_DEC_LAYERING_RULE,
    IRI_DEC_PERSISTENCE_MODEL,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// One violation observed against a candidate contract mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate the violation is against.
    pub path: String,
    /// Human-readable rule statement.
    pub message: String,
}

/// Aggregated SHACL failure for a contract mutation.
#[derive(Debug, Error)]
#[error("application-contract SHACL validation failed:\n{report}")]
pub struct ContractShaclError {
    /// One line per violation.
    pub report: String,
    /// Structured violations.
    pub violations: Vec<ContractViolation>,
}

const REQUIRED_CONVENTION_FIELDS: &[(&str, &str)] = &[
    (IRI_DEC_CONVENTION_NAME, "dec:conventionName"),
    (IRI_DEC_CONVENTION_BODY_PATH, "dec:conventionBodyPath"),
    (IRI_DEC_CONVENTION_CHECKABLE, "dec:conventionCheckable"),
];

const REQUIRED_CONVENTION_LINKS: &[(&str, &str)] = &[
    (IRI_DEC_LANGUAGE_RUNTIME, "dec:languageRuntime"),
    (IRI_DEC_LAYERING_RULE, "dec:layeringRule"),
    (IRI_DEC_FEATURE_ORGANISATION, "dec:featureOrganisation"),
    (IRI_DEC_PERSISTENCE_MODEL, "dec:persistenceModel"),
    (IRI_DEC_ENDPOINT_CONVENTION, "dec:endpointConvention"),
];

/// Validate every `dec:ApplicationContract` subject present in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), ContractShaclError> {
    let mut violations = Vec::new();

    for subject in contract_subjects(quads) {
        let by_pred = predicates_for(quads, &subject);
        if by_pred.get(IRI_DEC_CONTRACT_ARCHETYPE).is_none() {
            violations.push(ContractViolation {
                subject: subject.as_str().to_string(),
                path: IRI_DEC_CONTRACT_ARCHETYPE.to_string(),
                message: "missing required dec:archetype back-reference (sh:minCount 1)"
                    .to_string(),
            });
        }
        for (link, label) in REQUIRED_CONVENTION_LINKS {
            match by_pred.get(*link).and_then(|terms| match terms.first() {
                Some(Term::NamedNode(n)) => Some(n.clone()),
                _ => None,
            }) {
                None => violations.push(ContractViolation {
                    subject: subject.as_str().to_string(),
                    path: (*link).to_string(),
                    message: format!("missing required {label} Convention (sh:minCount 1)"),
                }),
                Some(convention) => {
                    validate_convention(quads, &convention, &mut violations);
                }
            }
        }
        // Cross-cutting entries are optional, but each must be well-formed.
        for q in quads {
            if q.subject == subject.clone().into()
                && q.predicate.as_str() == crate::vocab::IRI_DEC_CROSS_CUTTING
            {
                if let Term::NamedNode(c) = &q.object {
                    validate_convention(quads, c, &mut violations);
                }
            }
        }
    }

    if violations.is_empty() {
        return Ok(());
    }
    let report = violations
        .iter()
        .map(|v| {
            format!(
                "  • subject <{}> path <{}>: {}",
                v.subject, v.path, v.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Err(ContractShaclError { report, violations })
}

fn validate_convention(
    quads: &[Quad],
    convention: &NamedNode,
    violations: &mut Vec<ContractViolation>,
) {
    let by_pred = predicates_for(quads, convention);
    for (field, label) in REQUIRED_CONVENTION_FIELDS {
        let ok = matches!(
            by_pred.get(*field).and_then(|t| t.first()),
            Some(Term::Literal(l)) if !l.value().is_empty()
        );
        if !ok {
            violations.push(ContractViolation {
                subject: convention.as_str().to_string(),
                path: (*field).to_string(),
                message: format!("convention is missing non-empty {label} (sh:minCount 1)"),
            });
        }
    }
}

fn contract_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut subjects = Vec::new();
    for q in quads {
        if q.predicate.as_str() == RDF_TYPE
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_APPLICATION_CONTRACT_CLASS)
        {
            if let Subject::NamedNode(n) = &q.subject {
                if !subjects.contains(n) {
                    subjects.push(n.clone());
                }
            }
        }
    }
    subjects
}

fn predicates_for<'a>(quads: &'a [Quad], subject: &NamedNode) -> BTreeMap<&'a str, Vec<&'a Term>> {
    let mut map: BTreeMap<&str, Vec<&Term>> = BTreeMap::new();
    for q in quads {
        if q.subject == subject.clone().into() {
            map.entry(q.predicate.as_str()).or_default().push(&q.object);
        }
    }
    map
}
