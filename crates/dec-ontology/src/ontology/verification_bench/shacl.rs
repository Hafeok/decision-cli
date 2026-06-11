//! Write-side SHACL validation for `dec:VerificationBench` (ADR-028).
//!
//! Mirrors the verdict validator pattern (FT-020): every
//! `dec:VerificationBench` subject declared in the candidate quad
//! set is checked against the ADR-028 §SHACL invariants. Violations are
//! returned as structured records so the caller can surface a
//! `SchemaViolation { artifact: EnvId, detail }`-style error.

use std::collections::BTreeSet;

use oxrdf::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::vocab::{
    IRI_DEC_ALLOWED_OPS, IRI_DEC_BENCH_TYPE, IRI_DEC_ENDPOINT, IRI_DEC_FIXTURE_SOURCE,
    IRI_DEC_SAFETY_CLASS, IRI_DEC_VERIFICATION_BENCH, SAFETY_ISOLATED, SAFETY_PRODUCTION_READONLY,
    SAFETY_SHARED_NON_DESTRUCTIVE,
};

use super::shacl_list::{allowed_ops_heads, list_is_nil, walk_list};
use super::types::RDF_TYPE;

/// Prefix used by `bench_type` strings to denote a *remote* environment
/// (per ADR-028: `remote-http`, `remote-sparql`, …).
pub const REMOTE_BENCH_TYPE_PREFIX: &str = "remote-";

/// One SHACL violation against a candidate environment mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against (`dec:benchType`, etc.).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `VerificationBench`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for VerificationBench:\n{report}")]
pub struct EnvShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<EnvViolation>,
}

/// Run the ADR-028 SHACL shape against every `VerificationBench`
/// subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), EnvShaclError> {
    let subjects = env_subjects(quads);
    let mut violations: Vec<EnvViolation> = Vec::new();
    for subject in subjects {
        violations.extend(validate_subject(quads, &subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(EnvShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn env_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_VERIFICATION_BENCH {
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

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<EnvViolation> {
    let mut violations = Vec::new();
    let bench_type_values = literal_values(quads, subject, IRI_DEC_BENCH_TYPE);
    check_bench_type(subject, &bench_type_values, &mut violations);
    check_safety_class(quads, subject, &mut violations);
    check_allowed_ops(quads, subject, &mut violations);
    check_endpoint_conditional(quads, subject, &bench_type_values, &mut violations);
    check_fixture_source(quads, subject, &mut violations);
    violations
}

/// FT-053 / ADR-032 — `dec:fixtureSource` is optional, max-count 1,
/// non-empty string. The path's filesystem presence is the authoring
/// surface's responsibility (`features::verify_bench_new::validate`); the
/// shape only enforces shape.
fn check_fixture_source(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<EnvViolation>) {
    let values = literal_values(quads, subject, IRI_DEC_FIXTURE_SOURCE);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_FIXTURE_SOURCE,
            &format!(
                "expected at most one dec:fixtureSource, found {}",
                values.len()
            ),
        ));
    }
    for v in &values {
        if v.is_empty() {
            violations.push(violation(
                subject,
                IRI_DEC_FIXTURE_SOURCE,
                "dec:fixtureSource must be a non-empty string (sh:minLength 1)",
            ));
        }
    }
}

fn check_bench_type(subject: &NamedNode, values: &[String], violations: &mut Vec<EnvViolation>) {
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_BENCH_TYPE,
            "missing required dec:benchType (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_BENCH_TYPE,
            &format!("expected exactly one dec:benchType, found {}", values.len()),
        ));
    }
    for v in values {
        if v.is_empty() {
            violations.push(violation(
                subject,
                IRI_DEC_BENCH_TYPE,
                "dec:benchType must be a non-empty string (sh:minLength 1)",
            ));
        }
    }
}

fn check_safety_class(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<EnvViolation>) {
    let values = literal_values(quads, subject, IRI_DEC_SAFETY_CLASS);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_SAFETY_CLASS,
            "missing required dec:safetyClass (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_SAFETY_CLASS,
            &format!(
                "expected exactly one dec:safetyClass, found {}",
                values.len()
            ),
        ));
    }
    check_safety_class_values(subject, &values, violations);
}

fn check_safety_class_values(
    subject: &NamedNode,
    values: &[String],
    violations: &mut Vec<EnvViolation>,
) {
    let allowed: BTreeSet<&str> = [
        SAFETY_ISOLATED,
        SAFETY_SHARED_NON_DESTRUCTIVE,
        SAFETY_PRODUCTION_READONLY,
    ]
    .into_iter()
    .collect();
    for v in values {
        if !allowed.contains(v.as_str()) {
            violations.push(violation(
                subject,
                IRI_DEC_SAFETY_CLASS,
                &format!(
                    "dec:safetyClass must be one of {{{a}, {b}, {c}}}, got {v:?}",
                    a = SAFETY_ISOLATED,
                    b = SAFETY_SHARED_NON_DESTRUCTIVE,
                    c = SAFETY_PRODUCTION_READONLY,
                ),
            ));
        }
    }
}

fn check_allowed_ops(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<EnvViolation>) {
    let heads = allowed_ops_heads(quads, subject);
    if !check_allowed_ops_head_cardinality(subject, &heads, violations) {
        return;
    }
    let head = &heads[0];
    if list_is_nil(head) {
        violations.push(violation(
            subject,
            IRI_DEC_ALLOWED_OPS,
            "dec:allowedOps must be a non-empty rdf:List (head must not be rdf:nil)",
        ));
        return;
    }
    let items = walk_list(quads, head);
    if items.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_ALLOWED_OPS,
            "dec:allowedOps rdf:List must contain at least one operation token",
        ));
    }
}

/// Returns `false` if no head exists (caller must early-return), `true` otherwise.
fn check_allowed_ops_head_cardinality(
    subject: &NamedNode,
    heads: &[Term],
    violations: &mut Vec<EnvViolation>,
) -> bool {
    if heads.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_ALLOWED_OPS,
            "missing required dec:allowedOps rdf:List (sh:minCount 1)",
        ));
        return false;
    }
    if heads.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_ALLOWED_OPS,
            &format!(
                "expected exactly one dec:allowedOps rdf:List, found {}",
                heads.len()
            ),
        ));
    }
    true
}

fn check_endpoint_conditional(
    quads: &[Quad],
    subject: &NamedNode,
    bench_type_values: &[String],
    violations: &mut Vec<EnvViolation>,
) {
    let endpoint_values = literal_values(quads, subject, IRI_DEC_ENDPOINT);
    if endpoint_values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_ENDPOINT,
            &format!(
                "expected at most one dec:endpoint, found {}",
                endpoint_values.len()
            ),
        ));
    }
    check_endpoint_locality(subject, bench_type_values, &endpoint_values, violations);
}

fn check_endpoint_locality(
    subject: &NamedNode,
    bench_type_values: &[String],
    endpoint_values: &[String],
    violations: &mut Vec<EnvViolation>,
) {
    let is_remote = bench_type_values
        .iter()
        .any(|t| t.starts_with(REMOTE_BENCH_TYPE_PREFIX));
    let is_local = bench_type_values
        .iter()
        .any(|t| !t.starts_with(REMOTE_BENCH_TYPE_PREFIX));
    if is_remote && endpoint_values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_ENDPOINT,
            "remote bench types (envType starts with \"remote-\") require dec:endpoint",
        ));
    }
    if is_local && !endpoint_values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_ENDPOINT,
            "local bench types (envType does not start with \"remote-\") must NOT carry dec:endpoint",
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

fn subject_matches(q: &Quad, subject: &NamedNode) -> bool {
    match &q.subject {
        oxrdf::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> EnvViolation {
    EnvViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[EnvViolation]) -> String {
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
