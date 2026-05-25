//! TC-130 — Worker OCI image manifest exposes capability tags and SDK version labels.
//!
//! Validates: FT-088 · ADR-056 · ADR-057.
//! Spec: `.product/tests/TC-130-worker-oci-image-manifest-exposes-capability-tags.md`
//!
//! Three claims this integration test pins down end-to-end against the
//! OCI manifest validator shipped under `core::oci_manifest`:
//!
//! 1. A well-formed worker OCI manifest (capability-tag labels per
//!    ADR-057, SDK + wire-protocol semver labels, OCI source / revision
//!    annotations, multi-arch `linux/{amd64,arm64}` platforms) admits via
//!    [`validate_worker_oci_manifest`] and exposes its capability tags
//!    through [`WorkerOciManifest::capability_tags`] without "pulling"
//!    (i.e. without any additional input beyond the distilled manifest).
//! 2. The validator rejects every missing-label / missing-annotation /
//!    missing-platform case the FT-088 conventions require, and the
//!    rejection payload names the specific violation code so the
//!    admission flow (FT-094) can surface it verbatim to the worker
//!    author.
//! 3. The validator collects *every* violation in a single pass rather
//!    than short-circuiting, so a CI rejection produces one report
//!    enumerating all conformance gaps.

use std::collections::BTreeMap;

use decision_cli::core::oci_manifest::{
    capability_tag_label, parse_capability_tag, validate_worker_oci_manifest,
    CAPABILITY_TAG_LABEL_PREFIX, LABEL_DDD_SDK_VERSION, LABEL_DDD_WIRE_PROTOCOL,
    MIN_REQUIRED_PLATFORMS, OCI_ANNOTATION_REVISION, OCI_ANNOTATION_SOURCE, Platform,
    WorkerOciManifest,
};

/// Construct a manifest that mirrors what `docker manifest inspect`
/// would return for a slice-1-compliant worker image: two capability
/// tags claimed via labels, the two semver labels pinned, the two
/// standard OCI annotations populated, and both required platforms
/// declared in the manifest list.
fn well_formed_manifest() -> WorkerOciManifest {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert(capability_tag_label("code-writer"), "true".to_string());
    labels.insert(capability_tag_label("verifier"), "true".to_string());
    labels.insert(LABEL_DDD_SDK_VERSION.to_string(), "0.1.0".to_string());
    labels.insert(LABEL_DDD_WIRE_PROTOCOL.to_string(), "1.0.0".to_string());

    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
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
    validate_worker_oci_manifest(&well_formed_manifest())
        .expect("well-formed worker OCI manifest must pass FT-088 admission");
}

#[test]
fn capability_tags_are_discoverable_without_pulling() {
    let m = well_formed_manifest();
    let tags = m.capability_tags();
    assert!(
        tags.contains("code-writer"),
        "expected `code-writer` in capability tags, got {tags:?}"
    );
    assert!(
        tags.contains("verifier"),
        "expected `verifier` in capability tags, got {tags:?}"
    );
    assert_eq!(
        tags.len(),
        2,
        "expected exactly two capability tags, got {tags:?}"
    );
}

#[test]
fn sdk_and_wire_protocol_are_visible_via_accessors() {
    let m = well_formed_manifest();
    assert_eq!(m.sdk_version(), Some("0.1.0"));
    assert_eq!(m.wire_protocol(), Some("1.0.0"));
    assert_eq!(m.source_repo(), Some("https://github.com/example/worker"));
    assert_eq!(
        m.revision(),
        Some("abc123def456789abcdef0123456789abcdef0123")
    );
}

#[test]
fn rejects_image_missing_any_capability_tag() {
    let mut m = well_formed_manifest();
    m.labels
        .retain(|k, _| !k.starts_with(CAPABILITY_TAG_LABEL_PREFIX));
    let err = validate_worker_oci_manifest(&m)
        .expect_err("image with zero capability tags must be rejected by admission");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "ddd:capability-tag"));
}

#[test]
fn rejects_image_missing_sdk_version_label() {
    let mut m = well_formed_manifest();
    m.labels.remove(LABEL_DDD_SDK_VERSION);
    let err = validate_worker_oci_manifest(&m)
        .expect_err("image missing ddd.sdk-version must be rejected");
    assert!(err.report.contains(LABEL_DDD_SDK_VERSION), "{}", err.report);
}

#[test]
fn rejects_image_missing_wire_protocol_label() {
    let mut m = well_formed_manifest();
    m.labels.remove(LABEL_DDD_WIRE_PROTOCOL);
    let err = validate_worker_oci_manifest(&m)
        .expect_err("image missing ddd.wire-protocol must be rejected");
    assert!(
        err.report.contains(LABEL_DDD_WIRE_PROTOCOL),
        "{}",
        err.report
    );
}

#[test]
fn rejects_image_missing_source_provenance_annotations() {
    let mut m = well_formed_manifest();
    m.annotations.remove(OCI_ANNOTATION_SOURCE);
    m.annotations.remove(OCI_ANNOTATION_REVISION);
    let err = validate_worker_oci_manifest(&m)
        .expect_err("image missing source provenance annotations must be rejected");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    assert!(codes.contains(&"oci:annotation.source"));
    assert!(codes.contains(&"oci:annotation.revision"));
}

#[test]
fn rejects_single_arch_image() {
    let mut m = well_formed_manifest();
    m.platforms = vec![Platform::new("linux", "amd64")];
    let err = validate_worker_oci_manifest(&m)
        .expect_err("single-arch image must be rejected (missing linux/arm64)");
    assert!(err.violations.iter().any(|v| v.code == "oci:platforms"));
    assert!(err.report.contains("linux/arm64"), "{}", err.report);
}

#[test]
fn rejects_non_semver_pinned_versions() {
    let mut m = well_formed_manifest();
    m.labels
        .insert(LABEL_DDD_SDK_VERSION.to_string(), "latest".to_string());
    let err = validate_worker_oci_manifest(&m)
        .expect_err("non-semver ddd.sdk-version must be rejected");
    assert!(
        err.violations.iter().any(|v| v.code == "ddd:sdk-version"),
        "{:?}",
        err.violations
    );
}

#[test]
fn collects_every_violation_in_one_pass() {
    // Empty manifest — the validator MUST surface every required field
    // at once so a worker author can fix everything in a single CI
    // iteration rather than playing whack-a-mole.
    let err = validate_worker_oci_manifest(&WorkerOciManifest::empty())
        .expect_err("empty manifest must be rejected");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    for required in [
        "ddd:capability-tag",
        "ddd:sdk-version",
        "ddd:wire-protocol",
        "oci:annotation.source",
        "oci:annotation.revision",
        "oci:platforms",
    ] {
        assert!(
            codes.contains(&required),
            "empty manifest report missing required violation {required}; got {codes:?}"
        );
    }
}

#[test]
fn min_required_platforms_match_adr_056_floor() {
    // Belt-and-braces: ensure the constant the validator consults is
    // exactly what ADR-056 mandates as the multi-arch floor.
    let keys: Vec<&str> = MIN_REQUIRED_PLATFORMS.to_vec();
    assert!(keys.contains(&"linux/amd64"));
    assert!(keys.contains(&"linux/arm64"));
}

#[test]
fn capability_tag_label_key_format_matches_adr_057() {
    // The label key shape is a public contract — the manifest validator,
    // worker repo templates, and future bundle assemblers all rely on
    // it. Pin it down at the integration level so a refactor cannot
    // silently rename the convention.
    let key = capability_tag_label("implementer");
    assert_eq!(key, "ddd.capability-tag.implementer");
    assert_eq!(parse_capability_tag(&key, "true"), Some("implementer"));
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-130 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs the structural claims of TC-130 so this one function
/// reproduces the exit-criterion end-to-end.
#[test]
fn tc_130_worker_oci_image_manifest_exposes_capability_tags() {
    // 1. A well-formed worker OCI manifest passes admission.
    let m = well_formed_manifest();
    validate_worker_oci_manifest(&m).expect("well-formed manifest must pass FT-088");

    // 2. Capability tags are extractable from the manifest alone — the
    //    discovery path that ADR-057 mandates ("queryable for capability
    //    tags via `docker manifest inspect` without pulling").
    let tags = m.capability_tags();
    assert!(tags.contains("code-writer"));
    assert!(tags.contains("verifier"));

    // 3. SDK + wire-protocol version pins are visible on the manifest.
    assert_eq!(m.sdk_version(), Some("0.1.0"));
    assert_eq!(m.wire_protocol(), Some("1.0.0"));

    // 4. Source-provenance annotations are visible on the manifest.
    assert_eq!(m.source_repo(), Some("https://github.com/example/worker"));
    assert_eq!(
        m.revision(),
        Some("abc123def456789abcdef0123456789abcdef0123")
    );

    // 5. Multi-arch floor — both required platforms appear.
    let keys = m.platform_keys();
    assert!(keys.contains("linux/amd64"));
    assert!(keys.contains("linux/arm64"));

    // 6. Missing capability tags → rejected.
    let mut bad = m.clone();
    bad.labels
        .retain(|k, _| !k.starts_with(CAPABILITY_TAG_LABEL_PREFIX));
    let err =
        validate_worker_oci_manifest(&bad).expect_err("missing capability tags must fail");
    assert!(err
        .violations
        .iter()
        .any(|v| v.code == "ddd:capability-tag"));

    // 7. Missing SDK version label → rejected (and the violation names
    //    the SDK version label explicitly).
    let mut bad = m.clone();
    bad.labels.remove(LABEL_DDD_SDK_VERSION);
    let err = validate_worker_oci_manifest(&bad).expect_err("missing sdk-version must fail");
    assert!(err.report.contains(LABEL_DDD_SDK_VERSION), "{}", err.report);

    // 8. Missing wire-protocol label → rejected.
    let mut bad = m.clone();
    bad.labels.remove(LABEL_DDD_WIRE_PROTOCOL);
    let err =
        validate_worker_oci_manifest(&bad).expect_err("missing wire-protocol must fail");
    assert!(
        err.report.contains(LABEL_DDD_WIRE_PROTOCOL),
        "{}",
        err.report
    );

    // 9. Single-arch image → rejected with platform code.
    let mut bad = m.clone();
    bad.platforms = vec![Platform::new("linux", "amd64")];
    let err = validate_worker_oci_manifest(&bad)
        .expect_err("image missing linux/arm64 must fail multi-arch floor");
    assert!(err.violations.iter().any(|v| v.code == "oci:platforms"));

    // 10. Missing OCI source-provenance annotations → rejected.
    let mut bad = m.clone();
    bad.annotations.remove(OCI_ANNOTATION_SOURCE);
    bad.annotations.remove(OCI_ANNOTATION_REVISION);
    let err = validate_worker_oci_manifest(&bad)
        .expect_err("missing OCI source-provenance annotations must fail");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    assert!(codes.contains(&"oci:annotation.source"));
    assert!(codes.contains(&"oci:annotation.revision"));

    // 11. Validator collects every violation in one pass — an empty
    //     manifest produces a complete punch list, not the first hit.
    let err = validate_worker_oci_manifest(&WorkerOciManifest::empty())
        .expect_err("empty manifest must fail");
    let codes: Vec<&str> = err.violations.iter().map(|v| v.code).collect();
    for required in [
        "ddd:capability-tag",
        "ddd:sdk-version",
        "ddd:wire-protocol",
        "oci:annotation.source",
        "oci:annotation.revision",
        "oci:platforms",
    ] {
        assert!(codes.contains(&required), "{required} missing from punch list {codes:?}");
    }
}
