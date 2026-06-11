//! Write-side SHACL validation for `dec:VerificationVerdict` (ADR-018).

use std::collections::BTreeSet;

use oxrdf::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::vocab::{
    IRI_DEC_AMENDMENT_GUIDANCE, IRI_DEC_IN_STREAM, IRI_DEC_RATIONALE, IRI_DEC_VERDICT,
    IRI_DEC_VERIFICATION_VERDICT, IRI_DEC_VIOLATES, VERDICT_AMENDMENT_REQUIRED, VERDICT_APPROVED,
    VERDICT_REJECTED,
};

use super::types::{PROV_USED, PROV_WAS_GENERATED_BY, RDF_TYPE};

/// Minimum length of `dec:rationale` (ADR-018 SHACL `sh:minLength 20`).
pub const RATIONALE_MIN_LEN: usize = 20;

/// One SHACL violation observed against a candidate verdict mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Path the violation is against (a `dec:` or `prov:` IRI).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `VerificationVerdict`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for VerificationVerdict:\n{report}")]
pub struct VerdictShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<VerdictViolation>,
}

/// Run the ADR-018 SHACL shape against every `VerificationVerdict`
/// subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), VerdictShaclError> {
    let subjects = verdict_subjects(quads);
    let mut violations: Vec<VerdictViolation> = Vec::new();
    for subject in subjects {
        violations.extend(validate_subject(quads, &subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(VerdictShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn verdict_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_VERIFICATION_VERDICT {
            continue;
        }
        if let oxrdf::Subject::NamedNode(s) = &q.subject {
            if !out.iter().any(|n| n == s) {
                out.push(s.clone());
            }
        }
    }
    out
}

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<VerdictViolation> {
    let mut violations = Vec::new();
    let verdict_values = literal_values(quads, subject, IRI_DEC_VERDICT);
    check_verdict_cardinality(subject, &verdict_values, &mut violations);
    check_rationale(quads, subject, &mut violations);
    check_single_iri(
        quads,
        subject,
        PROV_WAS_GENERATED_BY,
        "prov:wasGeneratedBy",
        &mut violations,
    );
    check_at_least_one_iri(quads, subject, PROV_USED, "prov:used", &mut violations);
    check_single_iri(
        quads,
        subject,
        IRI_DEC_IN_STREAM,
        "dec:inStream",
        &mut violations,
    );
    check_amendment_guidance(quads, subject, &verdict_values, &mut violations);
    check_violates_conditional(quads, subject, &verdict_values, &mut violations);
    violations
}

fn check_verdict_cardinality(
    subject: &NamedNode,
    values: &[String],
    violations: &mut Vec<VerdictViolation>,
) {
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_VERDICT,
            "missing required dec:verdict (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_VERDICT,
            &format!("expected exactly one dec:verdict, found {}", values.len()),
        ));
    }
    check_verdict_allowed_values(subject, values, violations);
}

fn check_verdict_allowed_values(
    subject: &NamedNode,
    values: &[String],
    violations: &mut Vec<VerdictViolation>,
) {
    let allowed: BTreeSet<&str> = [
        VERDICT_APPROVED,
        VERDICT_REJECTED,
        VERDICT_AMENDMENT_REQUIRED,
    ]
    .into_iter()
    .collect();
    for v in values {
        if !allowed.contains(v.as_str()) {
            violations.push(violation(
                subject,
                IRI_DEC_VERDICT,
                &format!(
                    "dec:verdict must be one of {{approved, rejected, amendment-required}}, got {v:?}"
                ),
            ));
        }
    }
}

fn check_rationale(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<VerdictViolation>) {
    let values = literal_values(quads, subject, IRI_DEC_RATIONALE);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_RATIONALE,
            "missing required dec:rationale (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_RATIONALE,
            &format!("expected exactly one dec:rationale, found {}", values.len()),
        ));
    }
    for v in &values {
        if v.chars().count() < RATIONALE_MIN_LEN {
            violations.push(violation(
                subject,
                IRI_DEC_RATIONALE,
                &format!(
                    "dec:rationale must be ≥ {RATIONALE_MIN_LEN} chars (sh:minLength), got {}",
                    v.chars().count()
                ),
            ));
        }
    }
}

fn check_single_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<VerdictViolation>,
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

fn check_at_least_one_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<VerdictViolation>,
) {
    if iri_values(quads, subject, predicate).is_empty() {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {label} (sh:minCount 1)"),
        ));
    }
}

fn check_amendment_guidance(
    quads: &[Quad],
    subject: &NamedNode,
    verdict_values: &[String],
    violations: &mut Vec<VerdictViolation>,
) {
    let guidance = literal_values(quads, subject, IRI_DEC_AMENDMENT_GUIDANCE);
    if guidance.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_AMENDMENT_GUIDANCE,
            &format!(
                "expected at most one dec:amendmentGuidance, found {}",
                guidance.len()
            ),
        ));
    }
    let is_amendment = verdict_values
        .iter()
        .any(|v| v == VERDICT_AMENDMENT_REQUIRED);
    if is_amendment && guidance.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_AMENDMENT_GUIDANCE,
            "amendment-required verdict must carry exactly one dec:amendmentGuidance",
        ));
    }
}

fn check_violates_conditional(
    quads: &[Quad],
    subject: &NamedNode,
    verdict_values: &[String],
    violations: &mut Vec<VerdictViolation>,
) {
    let needs_violation = verdict_values
        .iter()
        .any(|v| v == VERDICT_REJECTED || v == VERDICT_AMENDMENT_REQUIRED);
    if !needs_violation {
        return;
    }
    if iri_values(quads, subject, IRI_DEC_VIOLATES).is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_VIOLATES,
            "rejected and amendment-required verdicts must cite at least one violated TC or ADR via dec:violates",
        ));
    }
}

/// Collect the literal objects of `(subject, predicate, ?)` quads.
pub fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
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

/// Collect the IRI objects of `(subject, predicate, ?)` quads.
pub fn iri_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
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
        oxrdf::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> VerdictViolation {
    VerdictViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[VerdictViolation]) -> String {
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
