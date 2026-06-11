//! Syntactic validator for SBOM OCI referrer descriptor URIs (FT-091 / ADR-059).
//!
//! Single entry point [`validate_oci_referrer_uri`]: accepts a `&str` and
//! either returns a parsed [`OciReferrerUri`] (when every rule passes)
//! or a structured error enumerating every violation. The validator
//! visits every rule rather than short-circuiting so the WorkerCurator's
//! rejection Feedback can name every defect in a single iteration.

use thiserror::Error;

use super::uri::{OciReferrerUri, SHA256_HEX_DIGITS};

/// Stable digest-algorithm prefix on an OCI digest reference. The SBOM
/// referrer flow only supports SHA-256 — this is the cosign-canonical
/// digest algorithm and the only one OCI v1.1 registries are required to
/// honour.
pub const DIGEST_ALGORITHM_PREFIX: &str = "sha256:";

/// One violation against a candidate SBOM referrer URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciReferrerUriViolation {
    /// Stable machine-readable code identifying the violation kind.
    /// Mirrors the SHACL-style `sh:path` discriminator used elsewhere
    /// in `core::ontology` and `core::oci_manifest`.
    pub code: &'static str,
    /// Operator-friendly explanation, sufficient on its own for a CI
    /// rejection log line.
    pub detail: String,
}

/// Structured failure for [`validate_oci_referrer_uri`].
#[derive(Debug, Error)]
#[error("SBOM referrer URI violates FT-091 conventions:\n{report}")]
pub struct OciReferrerUriValidationError {
    /// Rendered report (one bulleted line per violation).
    pub report: String,
    /// Raw violations, in evaluation order.
    pub violations: Vec<OciReferrerUriViolation>,
}

/// Validate a candidate SBOM referrer URI against the FT-091 syntactic conventions.
///
/// Conformance requirements (see module docs):
///
/// 1. Non-empty string.
/// 2. Contains the `@sha256:` digest separator (refuses mutable tag refs).
/// 3. The pre-`@` reference segment has a non-empty registry host and a
///    non-empty repository path (`<host>/<repo>` at minimum).
/// 4. The repository path uses lowercase OCI-distribution name characters.
/// 5. The post-`@` digest is `sha256:` + exactly 64 lowercase hex digits.
pub fn validate_oci_referrer_uri(
    raw: &str,
) -> Result<OciReferrerUri, OciReferrerUriValidationError> {
    let mut violations: Vec<OciReferrerUriViolation> = Vec::new();

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        violations.push(OciReferrerUriViolation {
            code: "sbom:non-empty",
            detail: "SBOM referrer URI must be a non-empty string".to_string(),
        });
        return Err(into_error(violations));
    }

    let Some((reference, digest_with_algo)) = trimmed.split_once('@') else {
        violations.push(OciReferrerUriViolation {
            code: "sbom:digest-pinned",
            detail: format!(
                "SBOM referrer URI {trimmed:?} is missing the `@sha256:` digest separator; \
                 mutable tag references are refused — pin by digest"
            ),
        });
        return Err(into_error(violations));
    };

    let (registry, repository) = check_reference(reference, &mut violations);
    let digest_hex = check_digest(digest_with_algo, &mut violations);

    if violations.is_empty() {
        return Ok(OciReferrerUri {
            registry: registry.to_string(),
            repository: repository.to_string(),
            digest_hex: digest_hex.to_string(),
        });
    }
    Err(into_error(violations))
}

/// Split `<host>/<repo-path>` and validate each side. Returns the (possibly
/// empty) host + repository slices regardless of whether violations were
/// recorded, so the caller can still surface every defect at once.
fn check_reference<'a>(
    reference: &'a str,
    v: &mut Vec<OciReferrerUriViolation>,
) -> (&'a str, &'a str) {
    let Some((registry, repository)) = reference.split_once('/') else {
        v.push(OciReferrerUriViolation {
            code: "sbom:reference-shape",
            detail: format!(
                "SBOM referrer URI must look like `<registry>/<repo>@sha256:<digest>`; \
                 got reference {reference:?} with no `/` separator"
            ),
        });
        return (reference, "");
    };

    if registry.is_empty() {
        v.push(OciReferrerUriViolation {
            code: "sbom:registry",
            detail: "SBOM referrer URI is missing the registry host before `/`".to_string(),
        });
    }
    if repository.is_empty() {
        v.push(OciReferrerUriViolation {
            code: "sbom:repository",
            detail: "SBOM referrer URI is missing the repository path after the registry"
                .to_string(),
        });
    } else if !is_lowercase_oci_repo(repository) {
        v.push(OciReferrerUriViolation {
            code: "sbom:repository-chars",
            detail: format!(
                "SBOM referrer URI repository {repository:?} contains characters outside the \
                 OCI distribution-spec lowercase set [a-z0-9._/-]"
            ),
        });
    }
    (registry, repository)
}

/// Validate the `sha256:<64 hex>` digest component.
fn check_digest<'a>(digest_with_algo: &'a str, v: &mut Vec<OciReferrerUriViolation>) -> &'a str {
    let Some(hex) = digest_with_algo.strip_prefix(DIGEST_ALGORITHM_PREFIX) else {
        v.push(OciReferrerUriViolation {
            code: "sbom:digest-algorithm",
            detail: format!(
                "SBOM referrer URI digest {digest_with_algo:?} must use the \
                 `sha256:` algorithm prefix; other digest algorithms are out of scope for slice 1"
            ),
        });
        return "";
    };

    if hex.len() != SHA256_HEX_DIGITS {
        v.push(OciReferrerUriViolation {
            code: "sbom:digest-length",
            detail: format!(
                "SBOM referrer URI digest hex must be exactly {SHA256_HEX_DIGITS} chars; \
                 got {len} chars in {hex:?}",
                len = hex.len()
            ),
        });
    }
    if !is_lowercase_hex(hex) {
        v.push(OciReferrerUriViolation {
            code: "sbom:digest-charset",
            detail: format!(
                "SBOM referrer URI digest hex {hex:?} must contain only lowercase hex chars \
                 [0-9a-f]"
            ),
        });
    }
    hex
}

/// OCI distribution-spec repository names: lowercase letters, digits,
/// `.`, `_`, `-`, `/`. We do not implement the full grammar (which
/// disallows leading separators, requires components of bounded length,
/// etc.) — this is a "common typo classes" check that catches the broad
/// failure modes without becoming a full OCI reference parser.
fn is_lowercase_oci_repo(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/'))
}

fn is_lowercase_hex(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn render(violations: &[OciReferrerUriViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in violations {
        let _ = writeln!(out, "  • [{}] {}", v.code, v.detail);
    }
    out
}

fn into_error(violations: Vec<OciReferrerUriViolation>) -> OciReferrerUriValidationError {
    OciReferrerUriValidationError {
        report: render(&violations),
        violations,
    }
}
