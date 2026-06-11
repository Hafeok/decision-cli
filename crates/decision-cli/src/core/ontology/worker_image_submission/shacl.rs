//! Write-side SHACL validation for `dec:WorkerImageSubmission` (FT-087 / ADR-040 / ADR-055).
//!
//! Mirrors the field-level invariants in
//! `assets/shapes/worker-image-submission.ttl`:
//!
//! - `submission_id` required, non-empty.
//! - `candidate_registry_ref` required, contains `@sha256:`.
//! - `claimed_capability_tag` cardinality ≥ 1, non-empty.
//! - `claimed_sbom_ref`, `claimed_signature_subject`, `claimed_signature_issuer` required.
//! - `claimed_source_repo_uri`, `claimed_source_commit_hash`, `claimed_build_run_url` required.
//! - `submission_lifecycle_state` ∈ {received, under-review, admitted, rejected}.
//! - `external_origin` required (the BoundaryArtifact escape hatch invariant).
//!
//! BoundaryArtifact class membership is the structural invariant tested
//! separately by [`crate::core::ontology::boundary_artifact::validate_boundary_artifact`]
//! over the same quad set.

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::core::ontology::EXTERNAL_ORIGIN_PROP;
use crate::core::sbom_referrer::validate_oci_referrer_uri;
use crate::core::vocab::{
    IRI_DEC_CANDIDATE_REGISTRY_REF, IRI_DEC_CLAIMED_BUILD_RUN_URL, IRI_DEC_CLAIMED_CAPABILITY_TAG,
    IRI_DEC_CLAIMED_SBOM_REF, IRI_DEC_CLAIMED_SIGNATURE_ISSUER, IRI_DEC_CLAIMED_SIGNATURE_SUBJECT,
    IRI_DEC_CLAIMED_SOURCE_COMMIT_HASH, IRI_DEC_CLAIMED_SOURCE_REPO_URI, IRI_DEC_SUBMISSION_ID,
    IRI_DEC_SUBMISSION_LIFECYCLE_STATE, SUBMISSION_STATE_ADMITTED, SUBMISSION_STATE_RECEIVED,
    SUBMISSION_STATE_REJECTED, SUBMISSION_STATE_UNDER_REVIEW,
};

use super::types::{RDF_TYPE, WORKER_IMAGE_SUBMISSION_CLASS_IRI};

/// One SHACL violation against a candidate WorkerImageSubmission mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerImageSubmissionViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `WorkerImageSubmission`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for WorkerImageSubmission:\n{report}")]
pub struct WorkerImageSubmissionShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<WorkerImageSubmissionViolation>,
}

/// Run the FT-087 SHACL shape against every `WorkerImageSubmission`
/// subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), WorkerImageSubmissionShaclError> {
    let subjects = submission_subjects(quads);
    let mut violations: Vec<WorkerImageSubmissionViolation> = Vec::new();
    for subject in &subjects {
        violations.extend(validate_subject(quads, subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(WorkerImageSubmissionShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn submission_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != WORKER_IMAGE_SUBMISSION_CLASS_IRI {
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

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<WorkerImageSubmissionViolation> {
    let mut v = Vec::new();
    require_string_one(
        quads,
        subject,
        IRI_DEC_SUBMISSION_ID,
        "dec:submission_id",
        &mut v,
    );
    require_registry_ref(quads, subject, &mut v);
    require_min_one_capability_tag(quads, subject, &mut v);
    check_required_string_block(quads, subject, &mut v);
    require_sbom_referrer_shape(quads, subject, &mut v);
    require_lifecycle_state(quads, subject, &mut v);
    require_external_origin(quads, subject, &mut v);
    v
}

/// FT-091: validate that every `claimed_sbom_ref` literal parses as a
/// syntactically-correct OCI referrer descriptor. The base `sh:minCount 1`
/// + non-empty check in [`check_required_string_block`] already rejects
/// missing or empty values; this rule layers the FT-091 OCI-shape check
/// on top so a Submission carrying `ghcr.io/foo:latest` is rejected at
/// write time rather than at Curator-bundle assembly.
fn require_sbom_referrer_shape(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
) {
    for value in literal_values(quads, subject, IRI_DEC_CLAIMED_SBOM_REF) {
        if value.is_empty() {
            // Already reported by check_required_string_block.
            continue;
        }
        if let Err(err) = validate_oci_referrer_uri(value.as_str()) {
            for u in &err.violations {
                violations.push(violation(
                    subject,
                    IRI_DEC_CLAIMED_SBOM_REF,
                    &format!(
                        "dec:claimed_sbom_ref is not a syntactically-correct OCI referrer \
                         descriptor (FT-091 / ADR-059): [{code}] {detail}",
                        code = u.code,
                        detail = u.detail,
                    ),
                ));
            }
        }
    }
}

/// Bundles the eight single-cardinality non-empty string properties FT-087
/// requires on a Submission's signature / SBOM / provenance triple set.
fn check_required_string_block(
    quads: &[Quad],
    subject: &NamedNode,
    v: &mut Vec<WorkerImageSubmissionViolation>,
) {
    for (iri, label) in REQUIRED_STRING_PROPERTIES {
        require_string_one(quads, subject, iri, label, v);
    }
}

const REQUIRED_STRING_PROPERTIES: &[(&str, &str)] = &[
    (IRI_DEC_CLAIMED_SBOM_REF, "dec:claimed_sbom_ref"),
    (
        IRI_DEC_CLAIMED_SIGNATURE_SUBJECT,
        "dec:claimed_signature_subject",
    ),
    (
        IRI_DEC_CLAIMED_SIGNATURE_ISSUER,
        "dec:claimed_signature_issuer",
    ),
    (
        IRI_DEC_CLAIMED_SOURCE_REPO_URI,
        "dec:claimed_source_repo_uri",
    ),
    (
        IRI_DEC_CLAIMED_SOURCE_COMMIT_HASH,
        "dec:claimed_source_commit_hash",
    ),
    (IRI_DEC_CLAIMED_BUILD_RUN_URL, "dec:claimed_build_run_url"),
];

fn require_string_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
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
    if values.iter().any(String::is_empty) {
        violations.push(violation(
            subject,
            predicate,
            &format!("{label} must be a non-empty string"),
        ));
    }
}

fn require_registry_ref(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_CANDIDATE_REGISTRY_REF);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_CANDIDATE_REGISTRY_REF,
            "missing required dec:candidate_registry_ref (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_CANDIDATE_REGISTRY_REF,
            &format!(
                "expected exactly one dec:candidate_registry_ref, found {}",
                values.len()
            ),
        ));
    }
    for v in &values {
        if !v.contains("@sha256:") {
            violations.push(violation(
                subject,
                IRI_DEC_CANDIDATE_REGISTRY_REF,
                &format!(
                    "dec:candidate_registry_ref must be an OCI reference pinned by @sha256: digest, got {v:?}"
                ),
            ));
        }
    }
}

fn require_min_one_capability_tag(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_CLAIMED_CAPABILITY_TAG);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_CLAIMED_CAPABILITY_TAG,
            "missing required dec:claimed_capability_tag (sh:minCount 1; Submission must claim ≥1 tag)",
        ));
    }
    if values.iter().any(String::is_empty) {
        violations.push(violation(
            subject,
            IRI_DEC_CLAIMED_CAPABILITY_TAG,
            "dec:claimed_capability_tag values must be non-empty",
        ));
    }
}

fn require_lifecycle_state(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_SUBMISSION_LIFECYCLE_STATE);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_SUBMISSION_LIFECYCLE_STATE,
            "missing required dec:submission_lifecycle_state (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_SUBMISSION_LIFECYCLE_STATE,
            &format!(
                "expected exactly one dec:submission_lifecycle_state, found {}",
                values.len()
            ),
        ));
    }
    let allowed: BTreeSet<&str> = [
        SUBMISSION_STATE_RECEIVED,
        SUBMISSION_STATE_UNDER_REVIEW,
        SUBMISSION_STATE_ADMITTED,
        SUBMISSION_STATE_REJECTED,
    ]
    .into_iter()
    .collect();
    for v in &values {
        if !allowed.contains(v.as_str()) {
            violations.push(violation(
                subject,
                IRI_DEC_SUBMISSION_LIFECYCLE_STATE,
                &format!(
                    "dec:submission_lifecycle_state must be one of {{received, under-review, admitted, rejected}}, got {v:?}"
                ),
            ));
        }
    }
}

fn require_external_origin(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageSubmissionViolation>,
) {
    let values = literal_values(quads, subject, EXTERNAL_ORIGIN_PROP);
    if values.is_empty() {
        violations.push(violation(
            subject,
            EXTERNAL_ORIGIN_PROP,
            "missing required dec:external_origin — every WorkerImageSubmission is a BoundaryArtifact and MUST document how it entered the system (FT-071 / ADR-040)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            EXTERNAL_ORIGIN_PROP,
            &format!(
                "expected exactly one dec:external_origin, found {}",
                values.len()
            ),
        ));
    }
    if values.iter().any(String::is_empty) {
        violations.push(violation(
            subject,
            EXTERNAL_ORIGIN_PROP,
            "dec:external_origin must be a non-empty string",
        ));
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
        oxigraph::model::Subject::NamedNode(s) => s == subject,
        _ => false,
    }
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> WorkerImageSubmissionViolation {
    WorkerImageSubmissionViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[WorkerImageSubmissionViolation]) -> String {
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
