//! Unit tests for the OCI manifest validator (FT-088).

use super::*;

fn well_formed() -> WorkerOciManifest {
    let mut labels = std::collections::BTreeMap::new();
    labels.insert(capability_tag_label("code-writer"), "true".to_string());
    labels.insert(capability_tag_label("implementer"), "true".to_string());
    labels.insert(LABEL_DDD_SDK_VERSION.to_string(), "0.1.0".to_string());
    labels.insert(LABEL_DDD_WIRE_PROTOCOL.to_string(), "1.0.0".to_string());

    let mut annotations = std::collections::BTreeMap::new();
    annotations.insert(
        OCI_ANNOTATION_SOURCE.to_string(),
        "https://github.com/example/worker".to_string(),
    );
    annotations.insert(
        OCI_ANNOTATION_REVISION.to_string(),
        "abc123def456789abcdef0123456789abcdef0123".to_string(),
    );

    WorkerOciManifest {
        labels,
        annotations,
        platforms: vec![
            Platform::new("linux", "amd64"),
            Platform::new("linux", "arm64"),
        ],
    }
}

#[test]
fn admits_well_formed_manifest() {
    validate_worker_oci_manifest(&well_formed()).expect("well-formed manifest must pass");
}

#[test]
fn capability_tag_extraction_is_deterministic() {
    let m = well_formed();
    let tags: Vec<String> = m.capability_tags().into_iter().collect();
    assert_eq!(tags, vec!["code-writer".to_string(), "implementer".to_string()]);
}

#[test]
fn rejects_missing_capability_tags() {
    let mut m = well_formed();
    m.labels
        .retain(|k, _| !k.starts_with(CAPABILITY_TAG_LABEL_PREFIX));
    let err =
        validate_worker_oci_manifest(&m).expect_err("manifest with no capability tags must fail");
    assert!(
        err.violations.iter().any(|v| v.code == "ddd:capability-tag"),
        "expected ddd:capability-tag violation: {:?}",
        err.violations
    );
}

#[test]
fn rejects_missing_sdk_version_label() {
    let mut m = well_formed();
    m.labels.remove(LABEL_DDD_SDK_VERSION);
    let err = validate_worker_oci_manifest(&m).expect_err("missing sdk-version must fail");
    assert!(err.report.contains("ddd.sdk-version"), "{}", err.report);
}

#[test]
fn rejects_non_semver_sdk_version() {
    let mut m = well_formed();
    m.labels
        .insert(LABEL_DDD_SDK_VERSION.to_string(), "latest".to_string());
    let err =
        validate_worker_oci_manifest(&m).expect_err("non-semver sdk-version must fail");
    assert!(err.report.contains("semver"), "{}", err.report);
}

#[test]
fn rejects_missing_wire_protocol_label() {
    let mut m = well_formed();
    m.labels.remove(LABEL_DDD_WIRE_PROTOCOL);
    let err = validate_worker_oci_manifest(&m).expect_err("missing wire-protocol must fail");
    assert!(err.report.contains("ddd.wire-protocol"), "{}", err.report);
}

#[test]
fn rejects_missing_source_annotation() {
    let mut m = well_formed();
    m.annotations.remove(OCI_ANNOTATION_SOURCE);
    let err = validate_worker_oci_manifest(&m).expect_err("missing source annotation must fail");
    assert!(
        err.violations
            .iter()
            .any(|v| v.code == "oci:annotation.source"),
        "expected oci:annotation.source violation: {:?}",
        err.violations
    );
}

#[test]
fn rejects_missing_revision_annotation() {
    let mut m = well_formed();
    m.annotations.remove(OCI_ANNOTATION_REVISION);
    let err =
        validate_worker_oci_manifest(&m).expect_err("missing revision annotation must fail");
    assert!(
        err.violations
            .iter()
            .any(|v| v.code == "oci:annotation.revision"),
        "expected oci:annotation.revision violation: {:?}",
        err.violations
    );
}

#[test]
fn rejects_single_arch_image_missing_arm64() {
    let mut m = well_formed();
    m.platforms = vec![Platform::new("linux", "amd64")];
    let err = validate_worker_oci_manifest(&m).expect_err("single-arch image must fail");
    assert!(
        err.violations.iter().any(|v| v.code == "oci:platforms"),
        "expected oci:platforms violation: {:?}",
        err.violations
    );
    assert!(err.report.contains("linux/arm64"), "{}", err.report);
}

#[test]
fn rejects_single_arch_image_missing_amd64() {
    let mut m = well_formed();
    m.platforms = vec![Platform::new("linux", "arm64")];
    let err = validate_worker_oci_manifest(&m).expect_err("single-arch image must fail");
    assert!(err.report.contains("linux/amd64"), "{}", err.report);
}

#[test]
fn accepts_image_with_extra_platforms() {
    let mut m = well_formed();
    m.platforms.push(Platform::new("linux", "ppc64le"));
    validate_worker_oci_manifest(&m).expect("extra platforms must not break admission");
}

#[test]
fn parse_capability_tag_rejects_non_true_values() {
    assert!(parse_capability_tag("ddd.capability-tag.foo", "false").is_none());
    assert!(parse_capability_tag("ddd.capability-tag.foo", "1").is_none());
    assert!(parse_capability_tag("ddd.capability-tag.foo", "").is_none());
}

#[test]
fn parse_capability_tag_returns_suffix_for_well_formed() {
    assert_eq!(
        parse_capability_tag("ddd.capability-tag.code-writer", "true"),
        Some("code-writer")
    );
}

#[test]
fn parse_capability_tag_rejects_non_prefix_keys() {
    assert!(parse_capability_tag("ddd.sdk-version", "true").is_none());
    assert!(parse_capability_tag("org.opencontainers.image.source", "true").is_none());
}

#[test]
fn capability_tag_label_round_trips() {
    let key = capability_tag_label("verifier");
    assert_eq!(key, "ddd.capability-tag.verifier");
    assert_eq!(parse_capability_tag(&key, "true"), Some("verifier"));
}

#[test]
fn collects_every_violation_in_one_pass() {
    // Empty manifest — should produce every required violation at once.
    let err = validate_worker_oci_manifest(&WorkerOciManifest::empty())
        .expect_err("empty manifest must fail");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    assert!(codes.contains(&"ddd:capability-tag"));
    assert!(codes.contains(&"ddd:sdk-version"));
    assert!(codes.contains(&"ddd:wire-protocol"));
    assert!(codes.contains(&"oci:annotation.source"));
    assert!(codes.contains(&"oci:annotation.revision"));
    assert!(codes.contains(&"oci:platforms"));
}
