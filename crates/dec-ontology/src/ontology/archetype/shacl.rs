//! Write-side SHACL validation for `dec:Archetype` (FT-147 / ADR-082 /
//! ADR-084 §1).
//!
//! Rust-side mirror of `shapes/archetype.shacl.ttl`, invoked by the
//! dec-graph stream-writer chokepoint on every archetype mutation
//! (ADR-041). The load-bearing constraint is E102: an archetype with an
//! empty seam-audit set is the one decomposition strictly worse than the
//! broad-worker baseline (ADR-084), so it never reaches the store.

use std::collections::BTreeMap;

use oxrdf::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::vocab::{
    ARCHETYPE_STATUS_VALUES, ARCHETYPE_VARIANCE_VALUES, IRI_DEC_APPLICATION_CONTRACT,
    IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT, IRI_DEC_ARCHETYPE,
    IRI_DEC_ARCHETYPE_LAYER_ESTIMATE, IRI_DEC_ARCHETYPE_STATUS, IRI_DEC_ARCHETYPE_TITLE,
    IRI_DEC_COVERAGE_NOTE, IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE, IRI_DEC_INSTANCE_VARIANCE,
    IRI_DEC_SEAM_AUDIT,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Error code for the ADR-084 §1 mandatory seam-audit gate.
pub const E102_CODE: &str = "E102_ArchetypeMissingSeamAudits";

/// One violation observed against a candidate archetype mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchetypeViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate the violation is against.
    pub path: String,
    /// Human-readable rule statement (E102 violations carry [`E102_CODE`]).
    pub message: String,
}

/// Aggregated SHACL failure for an archetype mutation.
#[derive(Debug, Error)]
#[error("archetype SHACL validation failed:\n{report}")]
pub struct ArchetypeShaclError {
    /// One line per violation.
    pub report: String,
    /// Structured violations.
    pub violations: Vec<ArchetypeViolation>,
}

/// Validate every `dec:Archetype` subject present in `quads`.
///
/// Subjects without the `rdf:type dec:Archetype` quad are ignored — the
/// chokepoint calls every artifact-type validator over the same insert
/// set, and each validator owns only its class.
pub fn validate_quads(quads: &[Quad]) -> Result<(), ArchetypeShaclError> {
    let mut violations = Vec::new();

    for subject in archetype_subjects(quads) {
        let by_pred = predicates_for(quads, &subject);
        let mut violate = |path: &str, message: String| {
            violations.push(ArchetypeViolation {
                subject: subject.as_str().to_string(),
                path: path.to_string(),
                message,
            });
        };

        // ADR-084 §1 — the gate this artifact type exists to enforce.
        if by_pred.get(IRI_DEC_SEAM_AUDIT).map_or(0, Vec::len) == 0 {
            violate(
                IRI_DEC_SEAM_AUDIT,
                format!(
                    "{E102_CODE}: seam-audit set must be non-empty (sh:minCount 1, ADR-084 §1)"
                ),
            );
        }

        for (path, label) in [
            (IRI_DEC_ARCHETYPE_TITLE, "dec:title"),
            (IRI_DEC_APPLICATION_CONTRACT, "dec:applicationContract"),
            (
                IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE,
                "dec:infrastructureContractTemplate",
            ),
            (
                IRI_DEC_ARCHETYPE_LAYER_ESTIMATE,
                "dec:archetypeLayerEstimate",
            ),
            (
                IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT,
                "dec:applicationContractHeldInvariant",
            ),
            (IRI_DEC_COVERAGE_NOTE, "dec:coverageNote"),
        ] {
            if by_pred.get(path).map_or(0, Vec::len) == 0 {
                violate(path, format!("missing required {label} (sh:minCount 1)"));
            }
        }

        check_vocab(
            &by_pred,
            IRI_DEC_ARCHETYPE_STATUS,
            "dec:status",
            ARCHETYPE_STATUS_VALUES,
            &mut violate,
        );
        check_vocab(
            &by_pred,
            IRI_DEC_INSTANCE_VARIANCE,
            "dec:instanceVariance",
            ARCHETYPE_VARIANCE_VALUES,
            &mut violate,
        );
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
    Err(ArchetypeShaclError { report, violations })
}

fn check_vocab(
    by_pred: &BTreeMap<&str, Vec<&Term>>,
    path: &str,
    label: &str,
    allowed: &[&str],
    violate: &mut impl FnMut(&str, String),
) {
    match by_pred.get(path) {
        None => violate(path, format!("missing required {label} (sh:minCount 1)")),
        Some(terms) if terms.is_empty() => {
            violate(path, format!("missing required {label} (sh:minCount 1)"));
        }
        Some(terms) => {
            for t in terms {
                let ok = matches!(t, Term::Literal(l) if allowed.contains(&l.value()));
                if !ok {
                    violate(
                        path,
                        format!("{label} must be one of {allowed:?} (sh:in), got {t}"),
                    );
                }
            }
        }
    }
}

fn archetype_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut subjects = Vec::new();
    for q in quads {
        if q.predicate.as_str() == RDF_TYPE
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_ARCHETYPE)
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
