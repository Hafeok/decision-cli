//! Rust-side SHACL validators for the BoundaryArtifact shape family —
//! `:BoundaryArtifactShape`, `:MigrationBackfillShape` (FT-071 / ADR-040 / ADR-042).
//!
//! Mirror the NodeShapes declared in `assets/shapes/boundary-artifact.ttl`:
//!
//!   * `:BoundaryArtifactShape` — requires exactly one non-empty
//!     `dec:external_origin` literal of datatype `xsd:string`.
//!   * `:MigrationBackfillShape` — requires exactly one
//!     `dec:isMigrationBackfill true^^xsd:boolean` triple.
//!
//! These validators consume a candidate quad set and a specific subject
//! IRI; the per-type composition at the StreamWriter chokepoint (FT-073,
//! per ADR-041) will invoke them alongside the universal mechanical
//! validator (FT-069). For slice-1 they are invoked directly by FT-071's
//! exit-criterion test (TC-121).

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::core::ontology::{
    EXTERNAL_ORIGIN_PROP, IS_MIGRATION_BACKFILL_PROP,
};

const IRI_XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const IRI_XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// One SHACL violation observed against a BoundaryArtifact (or
/// MigrationBackfill) subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryArtifactViolation {
    /// Artifact subject the violation is attached to.
    pub subject: String,
    /// Property path the violation is against (a `dec:` IRI).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation against the boundary shapes.
#[derive(Debug, Error)]
#[error("SHACL validation failed for boundary-artifact shapes:\n{report}")]
pub struct BoundaryArtifactShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<BoundaryArtifactViolation>,
}

/// Validate `subject` against `:BoundaryArtifactShape`'s
/// `dec:external_origin` constraint. Returns `Ok(())` only when the
/// subject carries exactly one non-empty xsd:string literal at the
/// `dec:external_origin` property.
pub fn validate_boundary_artifact(
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<(), BoundaryArtifactShaclError> {
    let mut violations: Vec<BoundaryArtifactViolation> = Vec::new();
    check_external_origin(quads, subject, &mut violations);
    if violations.is_empty() {
        return Ok(());
    }
    Err(BoundaryArtifactShaclError {
        report: render_violations(&violations),
        violations,
    })
}

/// Validate `subject` against `:MigrationBackfillShape`'s
/// `dec:isMigrationBackfill true` constraint. Returns `Ok(())` only when
/// the subject carries exactly one `xsd:boolean` literal with lexical
/// value `"true"` at the `dec:isMigrationBackfill` property.
pub fn validate_migration_backfill(
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<(), BoundaryArtifactShaclError> {
    let mut violations: Vec<BoundaryArtifactViolation> = Vec::new();
    check_is_migration_backfill(quads, subject, &mut violations);
    if violations.is_empty() {
        return Ok(());
    }
    Err(BoundaryArtifactShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn check_external_origin(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<BoundaryArtifactViolation>,
) {
    let literals = typed_literal_values(quads, subject, EXTERNAL_ORIGIN_PROP);
    if literals.is_empty() {
        violations.push(violation(
            subject,
            EXTERNAL_ORIGIN_PROP,
            "missing required dec:external_origin (sh:minCount 1) — every dec:BoundaryArtifact requires exactly one non-empty xsd:string literal at dec:external_origin (FT-071 / ADR-040)",
        ));
        return;
    }
    if literals.len() > 1 {
        violations.push(violation(
            subject,
            EXTERNAL_ORIGIN_PROP,
            &format!(
                "expected exactly one dec:external_origin (sh:maxCount 1), found {}",
                literals.len()
            ),
        ));
    }
    for (value, datatype) in &literals {
        if datatype != IRI_XSD_STRING {
            violations.push(violation(
                subject,
                EXTERNAL_ORIGIN_PROP,
                &format!(
                    "dec:external_origin must be xsd:string (sh:datatype), got datatype <{datatype}>"
                ),
            ));
            continue;
        }
        if value.is_empty() {
            violations.push(violation(
                subject,
                EXTERNAL_ORIGIN_PROP,
                "dec:external_origin literal must be non-empty (sh:minLength 1)",
            ));
        }
    }
}

fn check_is_migration_backfill(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<BoundaryArtifactViolation>,
) {
    let literals = typed_literal_values(quads, subject, IS_MIGRATION_BACKFILL_PROP);
    if literals.is_empty() {
        violations.push(violation(
            subject,
            IS_MIGRATION_BACKFILL_PROP,
            "missing required dec:isMigrationBackfill (sh:minCount 1) — every dec:MigrationBackfill requires dec:isMigrationBackfill true^^xsd:boolean (FT-071 / ADR-042)",
        ));
        return;
    }
    if literals.len() > 1 {
        violations.push(violation(
            subject,
            IS_MIGRATION_BACKFILL_PROP,
            &format!(
                "expected exactly one dec:isMigrationBackfill (sh:maxCount 1), found {}",
                literals.len()
            ),
        ));
    }
    validate_is_migration_backfill_literals(subject, &literals, violations);
}

fn validate_is_migration_backfill_literals(
    subject: &NamedNode,
    literals: &[(String, String)],
    violations: &mut Vec<BoundaryArtifactViolation>,
) {
    for (value, datatype) in literals {
        if datatype != IRI_XSD_BOOLEAN {
            violations.push(violation(
                subject,
                IS_MIGRATION_BACKFILL_PROP,
                &format!(
                    "dec:isMigrationBackfill must be xsd:boolean (sh:datatype), got datatype <{datatype}>"
                ),
            ));
            continue;
        }
        if value != "true" {
            violations.push(violation(
                subject,
                IS_MIGRATION_BACKFILL_PROP,
                &format!(
                    "dec:isMigrationBackfill must have value `true` (sh:hasValue true), got {value:?}"
                ),
            ));
        }
    }
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
                Term::Literal(lit) => Some((
                    lit.value().to_string(),
                    lit.datatype().as_str().to_string(),
                )),
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

fn violation(subject: &NamedNode, path: &str, detail: &str) -> BoundaryArtifactViolation {
    BoundaryArtifactViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[BoundaryArtifactViolation]) -> String {
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
