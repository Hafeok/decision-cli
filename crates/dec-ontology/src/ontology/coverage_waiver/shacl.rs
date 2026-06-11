//! Write-side SHACL validation for `dec:CoverageWaiver` (ADR-031 / FT-047).
//!
//! Mirrors the env / verdict / feedback validators in shape: every
//! `dec:CoverageWaiver` subject declared in the candidate quad set is
//! checked. Failures surface as a structured `WaiverShaclError`.

use oxrdf::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::vocab::{
    IRI_DCTERMS_CREATED, IRI_DEC_COVERAGE_WAIVER, IRI_DEC_WAIVER_FOR, IRI_DEC_WAIVER_REASON,
    IRI_PROV_WAS_ATTRIBUTED_TO, WAIVER_REASON_MIN_LEN,
};

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate waiver mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `CoverageWaiver`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for CoverageWaiver:\n{report}")]
pub struct WaiverShaclError {
    /// Rendered report (one line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<WaiverViolation>,
}

/// Run the ADR-031 SHACL shape against every `CoverageWaiver` subject
/// declared in `quads`. Returns `Ok(())` when no waiver subjects are
/// present, so it is safe to call on every mutation.
pub fn validate_quads(quads: &[Quad]) -> Result<(), WaiverShaclError> {
    let subjects = waiver_subjects(quads);
    let mut violations: Vec<WaiverViolation> = Vec::new();
    for subject in subjects {
        violations.extend(validate_subject(quads, &subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(WaiverShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn waiver_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_COVERAGE_WAIVER {
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

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<WaiverViolation> {
    let mut violations = Vec::new();
    check_iri_object_once(quads, subject, IRI_DEC_WAIVER_FOR, &mut violations);
    check_reason(quads, subject, &mut violations);
    check_iri_object_once(quads, subject, IRI_PROV_WAS_ATTRIBUTED_TO, &mut violations);
    check_literal_once(quads, subject, IRI_DCTERMS_CREATED, &mut violations);
    violations
}

fn check_iri_object_once(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    violations: &mut Vec<WaiverViolation>,
) {
    let count = match count_iri_objects(quads, subject, predicate) {
        Ok(n) => n,
        Err(()) => {
            violations.push(violation(
                subject,
                predicate,
                &format!("{predicate} must be an IRI"),
            ));
            return;
        }
    };
    report_count_cardinality(subject, predicate, count, violations);
}

fn count_iri_objects(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Result<usize, ()> {
    let mut count = 0usize;
    for q in quads {
        if q.predicate.as_str() != predicate {
            continue;
        }
        if !subject_matches(q, subject) {
            continue;
        }
        if !matches!(&q.object, Term::NamedNode(_)) {
            return Err(());
        }
        count += 1;
    }
    Ok(count)
}

fn report_count_cardinality(
    subject: &NamedNode,
    predicate: &str,
    count: usize,
    violations: &mut Vec<WaiverViolation>,
) {
    if count == 0 {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {predicate} (sh:minCount 1)"),
        ));
    } else if count > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {predicate}, found {count}"),
        ));
    }
}

fn check_literal_once(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    violations: &mut Vec<WaiverViolation>,
) {
    let count = inspect_literal_objects(quads, subject, predicate, violations);
    if count == 0 {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {predicate} (sh:minCount 1)"),
        ));
        return;
    }
    if count > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {predicate}, found {count}"),
        ));
    }
}

fn inspect_literal_objects(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    violations: &mut Vec<WaiverViolation>,
) -> usize {
    let mut count = 0usize;
    for q in quads {
        if q.predicate.as_str() != predicate {
            continue;
        }
        if !subject_matches(q, subject) {
            continue;
        }
        count += 1;
        validate_literal_object(subject, predicate, &q.object, violations);
    }
    count
}

fn validate_literal_object(
    subject: &NamedNode,
    predicate: &str,
    object: &Term,
    violations: &mut Vec<WaiverViolation>,
) {
    match object {
        Term::Literal(lit) if lit.value().is_empty() => {
            violations.push(violation(
                subject,
                predicate,
                &format!("{predicate} must be a non-empty literal"),
            ));
        }
        Term::Literal(_) => {}
        _ => {
            violations.push(violation(
                subject,
                predicate,
                &format!("{predicate} must be a literal"),
            ));
        }
    }
}

fn check_reason(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<WaiverViolation>) {
    let values = literal_values(quads, subject, IRI_DEC_WAIVER_REASON);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_WAIVER_REASON,
            "missing required dec:waiverReason (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_WAIVER_REASON,
            &format!(
                "expected exactly one dec:waiverReason, found {}",
                values.len()
            ),
        ));
    }
    check_reason_min_length(subject, &values, violations);
}

fn check_reason_min_length(
    subject: &NamedNode,
    values: &[String],
    violations: &mut Vec<WaiverViolation>,
) {
    for v in values {
        let non_ws = v.chars().filter(|c| !c.is_whitespace()).count();
        if non_ws < WAIVER_REASON_MIN_LEN {
            violations.push(violation(
                subject,
                IRI_DEC_WAIVER_REASON,
                &format!(
                    "dec:waiverReason must be at least {WAIVER_REASON_MIN_LEN} \
                     non-whitespace characters (got {non_ws})"
                ),
            ));
        }
    }
}

fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
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

fn subject_matches(q: &Quad, subject: &NamedNode) -> bool {
    match &q.subject {
        oxrdf::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> WaiverViolation {
    WaiverViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[WaiverViolation]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::coverage_waiver::types::CoverageWaiver;
    use crate::vocab::waivers_named_graph;
    use chrono::Utc;

    fn ok_waiver() -> CoverageWaiver {
        CoverageWaiver {
            id: "CW-001".to_string(),
            waiver_for: NamedNode::new_unchecked("https://decision-cli.dev/ns/feature/FT-U"),
            reason: "Doc-only feature; verification is review-based.".to_string(),
            attributed_to: NamedNode::new_unchecked("urn:dec:agent:cli"),
            created: Utc::now(),
            uncovered_at_waive: vec![NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/tc/TC-T2",
            )],
        }
    }

    #[test]
    fn well_formed_waiver_passes() {
        let w = ok_waiver();
        let quads = w.to_quads(waivers_named_graph());
        validate_quads(&quads).expect("should pass");
    }

    #[test]
    fn missing_reason_is_caught() {
        let w = ok_waiver();
        let quads: Vec<Quad> = w
            .to_quads(waivers_named_graph())
            .into_iter()
            .filter(|q| q.predicate.as_str() != IRI_DEC_WAIVER_REASON)
            .collect();
        let err = validate_quads(&quads).expect_err("must fail");
        assert!(err.report.contains("waiverReason"));
    }

    #[test]
    fn short_reason_is_caught() {
        let mut w = ok_waiver();
        w.reason = "too short".to_string();
        let quads = w.to_quads(waivers_named_graph());
        let err = validate_quads(&quads).expect_err("must fail");
        assert!(err.report.contains("non-whitespace"));
    }

    #[test]
    fn whitespace_only_reason_is_caught() {
        let mut w = ok_waiver();
        w.reason = "                                  ".to_string();
        let quads = w.to_quads(waivers_named_graph());
        let err = validate_quads(&quads).expect_err("must fail");
        assert!(err.report.contains("non-whitespace"));
    }

    #[test]
    fn missing_waiver_for_is_caught() {
        let w = ok_waiver();
        let quads: Vec<Quad> = w
            .to_quads(waivers_named_graph())
            .into_iter()
            .filter(|q| q.predicate.as_str() != IRI_DEC_WAIVER_FOR)
            .collect();
        let err = validate_quads(&quads).expect_err("must fail");
        assert!(err.report.contains("waiverFor"));
    }
}
