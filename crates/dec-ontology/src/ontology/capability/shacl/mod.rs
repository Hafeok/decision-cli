//! Write-side SHACL validation for `dec:Capability` (FT-054 / ADR-033).
//!
//! Mirrors the verdict / verification_bench validator pattern: every
//! `dec:Capability` subject declared in the candidate quad set is
//! checked against the ADR-033 §schema invariants. Violations are
//! returned as structured records so the caller can surface a
//! `SchemaViolation { artifact, detail }`-style error.

mod checks;
mod helpers;
mod subject;
mod unique;

use oxrdf::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::vocab::IRI_DEC_CAPABILITY;

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate capability mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against (`dec:cost_input_per_m`, etc.).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `Capability`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for Capability:\n{report}")]
pub struct CapabilityShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<CapabilityViolation>,
}

/// Run the FT-054 / ADR-033 SHACL shape against every `Capability`
/// subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), CapabilityShaclError> {
    let subjects = capability_subjects(quads);
    let mut violations: Vec<CapabilityViolation> = Vec::new();
    for subject in &subjects {
        violations.extend(subject::validate_subject(quads, subject));
    }
    // Cross-subject uniqueness: (capability_id, version) unique among active.
    violations.extend(unique::check_active_unique(quads, &subjects));
    if violations.is_empty() {
        return Ok(());
    }
    Err(CapabilityShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn capability_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_CAPABILITY {
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

fn render_violations(violations: &[CapabilityViolation]) -> String {
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
