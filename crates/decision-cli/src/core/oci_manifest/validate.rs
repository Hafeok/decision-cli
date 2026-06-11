//! Admission-time validator for worker OCI manifests (FT-088 / ADR-056 / ADR-057).
//!
//! Single entry point [`validate_worker_oci_manifest`]: accepts a
//! distilled [`WorkerOciManifest`] and returns `Ok(())` when the image
//! conforms to the conventions in [`super`], or a structured error
//! enumerating every violation. The error preserves the per-violation
//! detail so the `WorkerCurator` admission flow (FT-094) can surface it
//! verbatim in the rejection Feedback.

use thiserror::Error;

use super::labels::{
    LABEL_DDD_SDK_VERSION, LABEL_DDD_WIRE_PROTOCOL, MIN_REQUIRED_PLATFORMS,
    OCI_ANNOTATION_REVISION, OCI_ANNOTATION_SOURCE,
};
use super::manifest::WorkerOciManifest;

/// One violation against a candidate worker OCI manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciManifestViolation {
    /// Stable machine-readable code identifying the violation kind.
    /// Mirrors the SHACL-style `sh:path` discriminator used elsewhere
    /// in `core::ontology`.
    pub code: &'static str,
    /// Operator-friendly explanation, sufficient on its own for a CI
    /// rejection log line.
    pub detail: String,
}

/// Structured failure for [`validate_worker_oci_manifest`].
#[derive(Debug, Error)]
#[error("worker OCI manifest violates FT-088 conventions:\n{report}")]
pub struct OciManifestValidationError {
    /// Rendered report (one bulleted line per violation).
    pub report: String,
    /// Raw violations, in evaluation order.
    pub violations: Vec<OciManifestViolation>,
}

/// Validate a candidate worker OCI manifest against the FT-088 conventions.
///
/// Conformance requirements (see module docs):
///
/// 1. At least one capability-tag label (`ddd.capability-tag.<tag>=true`).
/// 2. SDK-version label (`ddd.sdk-version=<semver>`).
/// 3. Wire-protocol label (`ddd.wire-protocol=<semver>`).
/// 4. Source-repo annotation (`org.opencontainers.image.source`).
/// 5. Revision annotation (`org.opencontainers.image.revision`).
/// 6. At minimum the platforms declared in [`MIN_REQUIRED_PLATFORMS`].
///
/// The validator visits every rule (does not short-circuit on the first
/// failure) so CI can fix every violation in a single iteration.
pub fn validate_worker_oci_manifest(
    m: &WorkerOciManifest,
) -> Result<(), OciManifestValidationError> {
    let mut violations: Vec<OciManifestViolation> = Vec::new();

    check_capability_tags(m, &mut violations);
    check_semver_label(m, LABEL_DDD_SDK_VERSION, "ddd:sdk-version", &mut violations);
    check_semver_label(
        m,
        LABEL_DDD_WIRE_PROTOCOL,
        "ddd:wire-protocol",
        &mut violations,
    );
    check_non_empty_annotation(
        m,
        OCI_ANNOTATION_SOURCE,
        "oci:annotation.source",
        &mut violations,
    );
    check_non_empty_annotation(
        m,
        OCI_ANNOTATION_REVISION,
        "oci:annotation.revision",
        &mut violations,
    );
    check_required_platforms(m, &mut violations);

    if violations.is_empty() {
        return Ok(());
    }
    Err(OciManifestValidationError {
        report: render(&violations),
        violations,
    })
}

fn check_capability_tags(m: &WorkerOciManifest, v: &mut Vec<OciManifestViolation>) {
    let tags = m.capability_tags();
    if tags.is_empty() {
        v.push(OciManifestViolation {
            code: "ddd:capability-tag",
            detail: "manifest declares no capability-tag labels — required: at least one label of \
                     the form `ddd.capability-tag.<tag>=true` (ADR-057)"
                .to_string(),
        });
        return;
    }
    for tag in &tags {
        if tag.trim().is_empty() {
            v.push(OciManifestViolation {
                code: "ddd:capability-tag",
                detail: "capability-tag label has empty suffix; expected \
                         `ddd.capability-tag.<tag>=true` with non-empty <tag>"
                    .to_string(),
            });
        }
    }
}

fn check_semver_label(
    m: &WorkerOciManifest,
    label_key: &str,
    code: &'static str,
    v: &mut Vec<OciManifestViolation>,
) {
    match m.labels.get(label_key) {
        None => v.push(OciManifestViolation {
            code,
            detail: format!(
                "missing required label `{label_key}` — ADR-056 requires every worker image to \
                 pin this version"
            ),
        }),
        Some(value) => {
            if value.trim().is_empty() {
                v.push(OciManifestViolation {
                    code,
                    detail: format!("label `{label_key}` must be a non-empty semver value"),
                });
                return;
            }
            if !is_semver_shape(value) {
                v.push(OciManifestViolation {
                    code,
                    detail: format!(
                        "label `{label_key}` value {value:?} is not a semver string \
                         (expected `MAJOR.MINOR.PATCH`)"
                    ),
                });
            }
        }
    }
}

fn check_non_empty_annotation(
    m: &WorkerOciManifest,
    key: &str,
    code: &'static str,
    v: &mut Vec<OciManifestViolation>,
) {
    match m.annotations.get(key) {
        None => v.push(OciManifestViolation {
            code,
            detail: format!("missing required OCI annotation `{key}` (FT-088 source provenance)"),
        }),
        Some(value) if value.trim().is_empty() => v.push(OciManifestViolation {
            code,
            detail: format!("OCI annotation `{key}` must be non-empty"),
        }),
        Some(_) => (),
    }
}

fn check_required_platforms(m: &WorkerOciManifest, v: &mut Vec<OciManifestViolation>) {
    let declared = m.platform_keys();
    let mut missing: Vec<&str> = Vec::new();
    for required in MIN_REQUIRED_PLATFORMS {
        if !declared.iter().any(|d| d == required) {
            missing.push(required);
        }
    }
    if !missing.is_empty() {
        let required = MIN_REQUIRED_PLATFORMS;
        v.push(OciManifestViolation {
            code: "oci:platforms",
            detail: format!(
                "manifest list is missing required platforms {missing:?}; ADR-056 floor is \
                 {required:?}, manifest declared {declared:?}"
            ),
        });
    }
}

/// Minimal semver shape check — three dot-separated numeric segments,
/// optionally suffixed with `-<pre-release>` or `+<build>` per semver
/// 2.0. We do not implement full parsing here; the goal is to catch the
/// common typo classes (missing patch component, alphabetic-only
/// versions) before CI ships the image. Comprehensive parsing is a
/// future concern if and when downstream consumers care.
fn is_semver_shape(value: &str) -> bool {
    // Strip optional build metadata after `+`, then optional pre-release
    // after `-`, leaving the numeric `MAJOR.MINOR.PATCH` core.
    let without_build = match value.split_once('+') {
        Some((core, _)) => core,
        None => value,
    };
    let core = match without_build.split_once('-') {
        Some((c, _)) => c,
        None => without_build,
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn render(violations: &[OciManifestViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in violations {
        let _ = writeln!(out, "  • [{}] {}", v.code, v.detail);
    }
    out
}
