//! Write-side SHACL validation for `dec:WorkerImage` (FT-086 / ADR-055).

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Quad, Term};
use thiserror::Error;

use crate::sbom_referrer::validate_oci_referrer_uri;
use dec_ontology::vocab::{
    ELIGIBILITY_CANDIDATE, ELIGIBILITY_DEPRECATED, ELIGIBILITY_PULLED, ELIGIBILITY_QUALIFIED,
    IRI_DEC_BUILD_RUN_URL, IRI_DEC_CAPABILITY_TAG, IRI_DEC_ELIGIBILITY_STATUS,
    IRI_DEC_REGISTRY_REF, IRI_DEC_SBOM_REF, IRI_DEC_SIGNED_BY_ISSUER, IRI_DEC_SIGNED_BY_SUBJECT,
    IRI_DEC_SOURCE_COMMIT_HASH, IRI_DEC_SOURCE_REPO_URI, IRI_DEC_WORKER_IMAGE,
    IRI_DEC_WORKER_IMAGE_ID, IRI_DEC_WORKER_IMAGE_NAME, IRI_DEC_WORKER_IMAGE_VERSION,
};

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate WorkerImage mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerImageViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `WorkerImage`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for WorkerImage:\n{report}")]
pub struct WorkerImageShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<WorkerImageViolation>,
}

/// Run the FT-086 / ADR-055 SHACL shape against every `WorkerImage`
/// subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), WorkerImageShaclError> {
    let subjects = worker_image_subjects(quads);
    let mut violations: Vec<WorkerImageViolation> = Vec::new();
    for subject in &subjects {
        violations.extend(validate_subject(quads, subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(WorkerImageShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn worker_image_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_WORKER_IMAGE {
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

fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<WorkerImageViolation> {
    let mut v = Vec::new();
    require_identity_fields(quads, subject, &mut v);
    require_signing_fields(quads, subject, &mut v);
    require_source_fields(quads, subject, &mut v);
    v
}

fn require_identity_fields(quads: &[Quad], subject: &NamedNode, v: &mut Vec<WorkerImageViolation>) {
    require_string_one(
        quads,
        subject,
        IRI_DEC_WORKER_IMAGE_ID,
        "dec:worker_image_id",
        v,
    );
    require_string_one(
        quads,
        subject,
        IRI_DEC_WORKER_IMAGE_NAME,
        "dec:worker_image_name",
        v,
    );
    require_semver(quads, subject, v);
    require_registry_ref(quads, subject, v);
    require_eligibility(quads, subject, v);
    require_min_one_capability_tag(quads, subject, v);
}

fn require_signing_fields(quads: &[Quad], subject: &NamedNode, v: &mut Vec<WorkerImageViolation>) {
    require_string_one(
        quads,
        subject,
        IRI_DEC_SIGNED_BY_SUBJECT,
        "dec:signed_by_subject",
        v,
    );
    require_string_one(
        quads,
        subject,
        IRI_DEC_SIGNED_BY_ISSUER,
        "dec:signed_by_issuer",
        v,
    );
    require_string_one(quads, subject, IRI_DEC_SBOM_REF, "dec:sbom_ref", v);
    require_sbom_referrer_shape(quads, subject, v);
}

fn require_source_fields(quads: &[Quad], subject: &NamedNode, v: &mut Vec<WorkerImageViolation>) {
    require_string_one(
        quads,
        subject,
        IRI_DEC_SOURCE_REPO_URI,
        "dec:source_repo_uri",
        v,
    );
    require_string_one(
        quads,
        subject,
        IRI_DEC_SOURCE_COMMIT_HASH,
        "dec:source_commit_hash",
        v,
    );
    require_string_one(
        quads,
        subject,
        IRI_DEC_BUILD_RUN_URL,
        "dec:build_run_url",
        v,
    );
}

fn require_string_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<WorkerImageViolation>,
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

fn require_semver(quads: &[Quad], subject: &NamedNode, violations: &mut Vec<WorkerImageViolation>) {
    let values = literal_values(quads, subject, IRI_DEC_WORKER_IMAGE_VERSION);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_WORKER_IMAGE_VERSION,
            "missing required dec:worker_image_version (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_WORKER_IMAGE_VERSION,
            &format!(
                "expected exactly one dec:worker_image_version, found {}",
                values.len()
            ),
        ));
    }
    for v in &values {
        if !is_semver(v) {
            violations.push(violation(
                subject,
                IRI_DEC_WORKER_IMAGE_VERSION,
                &format!("dec:worker_image_version must be semver (major.minor.patch), got {v:?}"),
            ));
        }
    }
}

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn require_registry_ref(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_REGISTRY_REF);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_REGISTRY_REF,
            "missing required dec:registry_ref (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_REGISTRY_REF,
            &format!(
                "expected exactly one dec:registry_ref, found {}",
                values.len()
            ),
        ));
    }
    for v in &values {
        if !v.contains("@sha256:") {
            violations.push(violation(
                subject,
                IRI_DEC_REGISTRY_REF,
                &format!(
                    "dec:registry_ref must be an OCI reference pinned by @sha256: digest, got {v:?}"
                ),
            ));
        }
    }
}

fn require_eligibility(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_ELIGIBILITY_STATUS);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_ELIGIBILITY_STATUS,
            "missing required dec:eligibility_status (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_ELIGIBILITY_STATUS,
            &format!(
                "expected exactly one dec:eligibility_status, found {}",
                values.len()
            ),
        ));
    }
    let allowed: BTreeSet<&str> = [
        ELIGIBILITY_QUALIFIED,
        ELIGIBILITY_CANDIDATE,
        ELIGIBILITY_DEPRECATED,
        ELIGIBILITY_PULLED,
    ]
    .into_iter()
    .collect();
    for v in &values {
        if !allowed.contains(v.as_str()) {
            violations.push(violation(
                subject,
                IRI_DEC_ELIGIBILITY_STATUS,
                &format!(
                    "dec:eligibility_status must be one of {{qualified, candidate, deprecated, pulled}}, got {v:?}"
                ),
            ));
        }
    }
}

/// FT-091: validate that the `dec:sbom_ref` literal parses as a
/// syntactically-correct OCI referrer descriptor. Layered on top of the
/// base non-empty check so admitted WorkerImages carry a discoverable
/// SBOM referrer URI rather than an opaque string.
fn require_sbom_referrer_shape(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageViolation>,
) {
    for value in literal_values(quads, subject, IRI_DEC_SBOM_REF) {
        if value.is_empty() {
            continue;
        }
        if let Err(err) = validate_oci_referrer_uri(value.as_str()) {
            for u in &err.violations {
                violations.push(violation(
                    subject,
                    IRI_DEC_SBOM_REF,
                    &format!(
                        "dec:sbom_ref is not a syntactically-correct OCI referrer descriptor \
                         (FT-091 / ADR-059): [{code}] {detail}",
                        code = u.code,
                        detail = u.detail,
                    ),
                ));
            }
        }
    }
}

fn require_min_one_capability_tag(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<WorkerImageViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_CAPABILITY_TAG);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_CAPABILITY_TAG,
            "missing required dec:capability_tag (sh:minCount 1; image must claim ≥1 tag)",
        ));
    }
    if values.iter().any(String::is_empty) {
        violations.push(violation(
            subject,
            IRI_DEC_CAPABILITY_TAG,
            "dec:capability_tag values must be non-empty",
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

fn violation(subject: &NamedNode, path: &str, detail: &str) -> WorkerImageViolation {
    WorkerImageViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[WorkerImageViolation]) -> String {
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
