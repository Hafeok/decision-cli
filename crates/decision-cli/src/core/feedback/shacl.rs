//! Write-side SHACL validation for `dec:Feedback` (FT-026 / ADR-022).

use oxigraph::model::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_ADDRESSING_ARTIFACT, IRI_DEC_CLOSED_BY, IRI_DEC_DISPOSITION_OVERRIDE, IRI_DEC_EVIDENCE,
    IRI_DEC_FEEDBACK, IRI_DEC_FEEDBACK_CLASS, IRI_DEC_IN_STREAM, IRI_DEC_LIFECYCLE_STATE,
    IRI_DEC_RECEIVING_SESSION, IRI_DEC_RECOMMENDATION, IRI_DEC_REJECTION_REASON, IRI_DEC_ROUTED_AT,
    IRI_DEC_SEVERITY, IRI_DEC_SOURCE_ARTIFACT, IRI_DEC_SOURCE_SESSION, IRI_DEC_SUPERSEDED_BY,
    IRI_DEC_TARGET_ROLE,
};

use super::artifact::RDF_TYPE;
use super::lifecycle_shacl::{check_lifecycle_companion_fields, check_lifecycle_state_in};

/// One SHACL violation observed against a candidate feedback mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `Feedback` mutation.
#[derive(Debug, Error)]
#[error("SHACL validation failed for Feedback:\n{report}")]
pub struct FeedbackShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<FeedbackViolation>,
}

/// Run the ADR-022 SHACL shape against every `Feedback` subject declared
/// in `quads`. The shape encoded here only enforces required fields and
/// cardinality — class-vocabulary `sh:in` (FT-028) and lifecycle-state
/// transition rules (FT-027) layer on top.
pub fn validate_quads(quads: &[Quad]) -> Result<(), FeedbackShaclError> {
    let subjects = feedback_subjects(quads);
    let mut violations: Vec<FeedbackViolation> = Vec::new();
    for subject in subjects {
        violations.extend(validate_subject(quads, &subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(FeedbackShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn feedback_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_FEEDBACK {
            continue;
        }
        if let oxigraph::model::Subject::NamedNode(s) = &q.subject {
            if !out.iter().any(|n| n == s) {
                out.push(s.clone());
            }
        }
    }
    out
}

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<FeedbackViolation> {
    let mut violations = Vec::new();
    check_required_fields(quads, subject, &mut violations);
    check_optional_literal_cardinality(quads, subject, &mut violations);
    check_optional_iri_cardinality(quads, subject, &mut violations);
    check_lifecycle_state_in(quads, subject, &mut violations);
    check_lifecycle_companion_fields(quads, subject, &mut violations);
    violations
}

fn check_required_fields(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    let required_literals = [
        (IRI_DEC_FEEDBACK_CLASS, "dec:feedbackClass", false),
        (IRI_DEC_LIFECYCLE_STATE, "dec:lifecycleState", false),
        (IRI_DEC_TARGET_ROLE, "dec:targetRole", false),
        (IRI_DEC_EVIDENCE, "dec:evidence", true),
    ];
    for (predicate, label, require_nonempty) in required_literals {
        check_required_literal(quads, subject, predicate, label, require_nonempty, violations);
    }
    let required_iris = [
        (IRI_DEC_SOURCE_SESSION, "dec:sourceSession"),
        (IRI_DEC_IN_STREAM, "dec:inStream"),
    ];
    for (predicate, label) in required_iris {
        check_required_iri(quads, subject, predicate, label, violations);
    }
}

fn check_optional_literal_cardinality(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    let optional_literals = [
        (IRI_DEC_SEVERITY, "dec:severity"),
        (IRI_DEC_RECOMMENDATION, "dec:recommendation"),
        (IRI_DEC_REJECTION_REASON, "dec:rejectionReason"),
        (IRI_DEC_DISPOSITION_OVERRIDE, "dec:dispositionOverride"),
        (IRI_DEC_ROUTED_AT, "dec:routedAt"),
    ];
    for (predicate, label) in optional_literals {
        check_optional_literal_max_one(quads, subject, predicate, label, violations);
    }
}

fn check_optional_iri_cardinality(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    let optional_iris = [
        (IRI_DEC_SOURCE_ARTIFACT, "dec:sourceArtifact"),
        (IRI_DEC_ADDRESSING_ARTIFACT, "dec:addressingArtifact"),
        (IRI_DEC_SUPERSEDED_BY, "dec:supersededBy"),
        (IRI_DEC_CLOSED_BY, "dec:closedBy"),
        (IRI_DEC_RECEIVING_SESSION, "dec:receivingSession"),
    ];
    for (predicate, label) in optional_iris {
        check_optional_iri_max_one(quads, subject, predicate, label, violations);
    }
}

fn check_required_literal(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    require_nonempty: bool,
    violations: &mut Vec<FeedbackViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {label} (sh:minCount 1)"),
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    if require_nonempty {
        for v in &values {
            if v.is_empty() {
                violations.push(violation(
                    subject,
                    predicate,
                    &format!("{label} must be non-empty (sh:minLength 1)"),
                ));
            }
        }
    }
}

fn check_optional_literal_max_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<FeedbackViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected at most one {label}, found {}", values.len()),
        ));
    }
}

fn check_required_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<FeedbackViolation>,
) {
    let values = iri_values(quads, subject, predicate);
    if values.is_empty() {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {label} (sh:minCount 1)"),
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
}

fn check_optional_iri_max_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<FeedbackViolation>,
) {
    let values = iri_values(quads, subject, predicate);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected at most one {label}, found {}", values.len()),
        ));
    }
}

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

pub(super) fn violation(subject: &NamedNode, path: &str, detail: &str) -> FeedbackViolation {
    FeedbackViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[FeedbackViolation]) -> String {
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
