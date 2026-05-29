//! Write-side SHACL validation for verification-graph artifacts (FT-036 / ADR-028).
//!
//! Covers the per-kind step shapes via `dec:stepType`-driven dispatch.
//!
//! Mirrors the verification-environment validator pattern (FT-035): every
//! candidate subject declared in the inserted quad set is checked against
//! the relevant shape. The step-kind-specific shape is selected by the
//! `dec:stepType` literal. Violations are returned as structured records
//! so the caller can surface `Error::SchemaViolation { artifact, detail }`.

use oxigraph::model::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_BIND_AS, IRI_DEC_COMMAND, IRI_DEC_CONDITION, IRI_DEC_BENCH, IRI_DEC_METHOD,
    IRI_DEC_PATH, IRI_DEC_QUERY, IRI_DEC_STEPS, IRI_DEC_STEP_TYPE, IRI_DEC_TARGET, IRI_DEC_TIMEOUT,
    IRI_DEC_URL, IRI_DEC_VERIFICATION_GRAPH, IRI_DEC_VERIFICATION_STEP, IRI_DEC_VERIFIES,
    SEED_STEP_KINDS, STEP_KIND_CAPTURE, STEP_KIND_FILE_ASSERTION, STEP_KIND_HTTP_REQUEST,
    STEP_KIND_SHELL_COMMAND, STEP_KIND_SPARQL_ASSERTION, STEP_KIND_WAIT_FOR,
};

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate graph or step mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against (`dec:command`, etc.).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a graph or step.
#[derive(Debug, Error)]
#[error("SHACL validation failed for VerificationGraph/Step:\n{report}")]
pub struct GraphShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<GraphViolation>,
}

/// Error type returned when a step carries an unrecognised `dec:stepType`.
#[derive(Debug, Error)]
#[error("unknown dec:stepType literal: {value:?}")]
pub struct UnknownStepKindError {
    /// The literal value that failed to parse.
    pub value: String,
}

/// Run the FT-036 SHACL shapes over every graph and step subject declared in
/// `quads`. Returns an error aggregating per-subject violations.
pub fn validate_quads(quads: &[Quad]) -> Result<(), GraphShaclError> {
    let mut violations: Vec<GraphViolation> = Vec::new();
    for subject in graph_subjects(quads) {
        violations.extend(validate_graph_subject(quads, &subject));
    }
    for subject in step_subjects(quads) {
        violations.extend(validate_step_subject(quads, &subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(GraphShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn graph_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    type_subjects(quads, IRI_DEC_VERIFICATION_GRAPH)
}

fn step_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    type_subjects(quads, IRI_DEC_VERIFICATION_STEP)
}

fn type_subjects(quads: &[Quad], class_iri: &str) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != class_iri {
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

fn validate_graph_subject(quads: &[Quad], subject: &NamedNode) -> Vec<GraphViolation> {
    let mut violations = Vec::new();
    check_single_iri(quads, subject, IRI_DEC_VERIFIES, &mut violations);
    check_single_iri(quads, subject, IRI_DEC_BENCH, &mut violations);
    check_steps_present(quads, subject, &mut violations);
    violations
}

fn check_steps_present(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<GraphViolation>) {
    let heads: Vec<Term> = quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != IRI_DEC_STEPS {
                return None;
            }
            if !subject_matches(q, subject) {
                return None;
            }
            Some(q.object.clone())
        })
        .collect();
    if heads.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_STEPS,
            "missing required dec:steps rdf:List (sh:minCount 1)",
        ));
    }
    if heads.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_STEPS,
            &format!("expected exactly one dec:steps head, found {}", heads.len()),
        ));
    }
}

fn validate_step_subject(quads: &[Quad], subject: &NamedNode) -> Vec<GraphViolation> {
    let mut violations = Vec::new();
    let step_types = literal_values(quads, subject, IRI_DEC_STEP_TYPE);
    let Some(kind) = check_step_type_cardinality(subject, &step_types, &mut violations) else {
        return violations;
    };
    if !SEED_STEP_KINDS.contains(&kind.as_str()) {
        violations.push(violation(
            subject,
            IRI_DEC_STEP_TYPE,
            &format!("dec:stepType must be one of {SEED_STEP_KINDS:?}; got {kind:?}"),
        ));
        return violations;
    }
    violations.extend(validate_kind_specific(quads, subject, &kind));
    violations
}

fn check_step_type_cardinality(
    subject: &NamedNode,
    step_types: &[String],
    violations: &mut Vec<GraphViolation>,
) -> Option<String> {
    if step_types.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_STEP_TYPE,
            "missing required dec:stepType (sh:minCount 1)",
        ));
        return None;
    }
    if step_types.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_STEP_TYPE,
            &format!(
                "expected exactly one dec:stepType, found {}",
                step_types.len()
            ),
        ));
    }
    Some(step_types[0].clone())
}

fn validate_kind_specific(quads: &[Quad], subject: &NamedNode, kind: &str) -> Vec<GraphViolation> {
    let mut out = Vec::new();
    match kind {
        STEP_KIND_SHELL_COMMAND => {
            require_field(quads, subject, IRI_DEC_COMMAND, "shell-command", &mut out);
        }
        STEP_KIND_SPARQL_ASSERTION => {
            require_field(quads, subject, IRI_DEC_QUERY, "sparql-assertion", &mut out);
            require_field(quads, subject, IRI_DEC_TARGET, "sparql-assertion", &mut out);
        }
        STEP_KIND_FILE_ASSERTION => {
            require_field(quads, subject, IRI_DEC_PATH, "file-assertion", &mut out);
        }
        STEP_KIND_HTTP_REQUEST => {
            require_field(quads, subject, IRI_DEC_URL, "http-request", &mut out);
            require_field(quads, subject, IRI_DEC_METHOD, "http-request", &mut out);
        }
        STEP_KIND_WAIT_FOR => {
            require_field(quads, subject, IRI_DEC_TIMEOUT, "wait-for", &mut out);
            require_field(quads, subject, IRI_DEC_CONDITION, "wait-for", &mut out);
        }
        STEP_KIND_CAPTURE => {
            require_field(quads, subject, IRI_DEC_BIND_AS, "capture", &mut out);
        }
        _ => {}
    }
    out
}

fn require_field(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    kind: &str,
    violations: &mut Vec<GraphViolation>,
) {
    if !has_predicate(quads, subject, predicate) {
        violations.push(violation(
            subject,
            predicate,
            &format!("{kind} step requires {predicate}"),
        ));
    }
}

fn check_single_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    violations: &mut Vec<GraphViolation>,
) {
    let values = collect_iri_values(quads, subject, predicate);
    if values.is_empty() {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {predicate} (sh:minCount 1; IRI node)"),
        ));
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {predicate}, found {}", values.len()),
        ));
    }
}

fn collect_iri_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !subject_matches(q, subject) {
                return None;
            }
            if let Term::NamedNode(n) = &q.object {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect()
}

fn has_predicate(quads: &[Quad], subject: &NamedNode, predicate: &str) -> bool {
    quads
        .iter()
        .any(|q| q.predicate.as_str() == predicate && subject_matches(q, subject))
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
    matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject)
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> GraphViolation {
    GraphViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[GraphViolation]) -> String {
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
