//! Rust-side SHACL validator for `:MechanicalProvenanceShape` (FT-069).
//!
//! Mirrors the universal NodeShape declared in
//! `assets/shapes/mechanical-provenance.ttl`. Caller passes a specific
//! artifact subject IRI and the candidate quad set; the validator
//! reports any missing or malformed mechanical-provenance triples. The
//! per-type composition (FT-072) wires this into every artifact type's
//! validator call at the StreamWriter chokepoint.

use oxrdf::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::vocab::{
    IRI_PROV_GENERATED_AT_TIME,
    IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL as IRI_PROV_WAS_ATTRIBUTED_TO, IRI_PROV_WAS_GENERATED_BY,
    IRI_XSD_DATE_TIME,
};

/// One SHACL violation observed against an artifact's mechanical block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanicalProvenanceViolation {
    /// Artifact subject the violation is attached to.
    pub subject: String,
    /// Property path the violation is against (a `prov:` IRI).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of an artifact's mechanical block.
#[derive(Debug, Error)]
#[error("SHACL validation failed for MechanicalProvenanceShape:\n{report}")]
pub struct MechanicalProvenanceShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<MechanicalProvenanceViolation>,
}

/// Validate every artifact subject in `subjects` against the universal
/// mechanical-provenance shape. Returns `Ok(())` only when every subject
/// conforms (`conforms = true` for SHACL parlance).
pub fn validate_quads(
    quads: &[Quad],
    subjects: &[NamedNode],
) -> Result<(), MechanicalProvenanceShaclError> {
    let mut violations: Vec<MechanicalProvenanceViolation> = Vec::new();
    for subject in subjects {
        violations.extend(validate_subject(quads, subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(MechanicalProvenanceShaclError {
        report: render_violations(&violations),
        violations,
    })
}

/// Validate a single artifact subject. Surfaces the per-property
/// violations as a flat list (no early exit) so the caller can render
/// them all in one report.
#[must_use]
pub fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<MechanicalProvenanceViolation> {
    let mut violations = Vec::new();
    check_was_generated_by(quads, subject, &mut violations);
    check_was_attributed_to(quads, subject, &mut violations);
    check_generated_at_time(quads, subject, &mut violations);
    violations
}

fn check_was_generated_by(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<MechanicalProvenanceViolation>,
) {
    let iris = iri_values(quads, subject, IRI_PROV_WAS_GENERATED_BY);
    if iris.is_empty() {
        violations.push(violation(
            subject,
            IRI_PROV_WAS_GENERATED_BY,
            "missing required prov:wasGeneratedBy (sh:minCount 1) — every artifact requires exactly one prov:wasGeneratedBy → dec:Session (FT-069 / ADR-038)",
        ));
        return;
    }
    if iris.len() > 1 {
        violations.push(violation(
            subject,
            IRI_PROV_WAS_GENERATED_BY,
            &format!(
                "expected exactly one prov:wasGeneratedBy (sh:maxCount 1), found {}",
                iris.len()
            ),
        ));
    }
}

fn check_was_attributed_to(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<MechanicalProvenanceViolation>,
) {
    let iris = iri_values(quads, subject, IRI_PROV_WAS_ATTRIBUTED_TO);
    if iris.is_empty() {
        violations.push(violation(
            subject,
            IRI_PROV_WAS_ATTRIBUTED_TO,
            "missing required prov:wasAttributedTo (sh:minCount 1) — every artifact requires at least one prov:wasAttributedTo → dec:Agent (FT-069 / ADR-038)",
        ));
    }
}

fn check_generated_at_time(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<MechanicalProvenanceViolation>,
) {
    let literals = typed_literal_values(quads, subject, IRI_PROV_GENERATED_AT_TIME);
    if literals.is_empty() {
        violations.push(violation(
            subject,
            IRI_PROV_GENERATED_AT_TIME,
            "missing required prov:generatedAtTime (sh:minCount 1) — every artifact requires exactly one prov:generatedAtTime (xsd:dateTime, FT-069 / ADR-038)",
        ));
        return;
    }
    if literals.len() > 1 {
        violations.push(violation(
            subject,
            IRI_PROV_GENERATED_AT_TIME,
            &format!(
                "expected exactly one prov:generatedAtTime (sh:maxCount 1), found {}",
                literals.len()
            ),
        ));
    }
    validate_generated_at_time_literals(subject, &literals, violations);
}

fn validate_generated_at_time_literals(
    subject: &NamedNode,
    literals: &[(String, String)],
    violations: &mut Vec<MechanicalProvenanceViolation>,
) {
    for (value, datatype) in literals {
        if datatype != IRI_XSD_DATE_TIME {
            violations.push(violation(
                subject,
                IRI_PROV_GENERATED_AT_TIME,
                &format!(
                    "prov:generatedAtTime must be xsd:dateTime (sh:datatype), got datatype <{datatype}>"
                ),
            ));
            continue;
        }
        if !is_lexical_date_time(value) {
            violations.push(violation(
                subject,
                IRI_PROV_GENERATED_AT_TIME,
                &format!(
                    "prov:generatedAtTime literal {value:?} is not a lexically-valid xsd:dateTime"
                ),
            ));
        }
    }
}

fn iri_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
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

fn typed_literal_values(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Vec<(String, String)> {
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
                Term::Literal(lit) => {
                    Some((lit.value().to_string(), lit.datatype().as_str().to_string()))
                }
                _ => None,
            }
        })
        .collect()
}

fn subject_matches(q: &Quad, subject: &NamedNode) -> bool {
    match &q.subject {
        Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> MechanicalProvenanceViolation {
    MechanicalProvenanceViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[MechanicalProvenanceViolation]) -> String {
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

/// Minimal lexical check for `xsd:dateTime`. SHACL's full validator
/// performs a complete grammar match; for slice 1 we accept any literal
/// matching `YYYY-MM-DDTHH:MM:SS(.fff)?(Z|±HH:MM)?` (the shape commit
/// path supplies RFC-3339 timestamps from the GraphWriter clock).
fn is_lexical_date_time(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return false;
    }
    // YYYY-MM-DDTHH:MM:SS
    is_digit_run(&bytes[0..4])
        && bytes[4] == b'-'
        && is_digit_run(&bytes[5..7])
        && bytes[7] == b'-'
        && is_digit_run(&bytes[8..10])
        && bytes[10] == b'T'
        && is_digit_run(&bytes[11..13])
        && bytes[13] == b':'
        && is_digit_run(&bytes[14..16])
        && bytes[16] == b':'
        && is_digit_run(&bytes[17..19])
}

fn is_digit_run(slice: &[u8]) -> bool {
    !slice.is_empty() && slice.iter().all(|b| b.is_ascii_digit())
}
