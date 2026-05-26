//! Unit tests for FT-093's worker-manifest substrate.

use super::*;

const CANONICAL_MANIFEST: &str = r#"
# implementer worker manifest
[worker]
name = "implementer"
sdk_version = "0.3.0"
wire_protocol = "1.0"

[capabilities]
tags = ["code-writer", "frontier-reasoning"]
compatible_roles = ["https://decision-cli.dev/ns/role/implementer"]

[runtime]
kind = "subscribed"
entrypoint = "implementer.main:run"
"#;

fn build_outputs() -> ReleaseBuildOutputs {
    ReleaseBuildOutputs {
        registry_ref: "ghcr.io/example/implementer@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        sbom_ref: "ghcr.io/example/implementer@sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe".to_string(),
        signature_subject: "https://github.com/example/implementer/.github/workflows/release.yml@refs/tags/implementer-v1.2.0".to_string(),
        signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        source_repo_uri: "https://github.com/example/implementer".to_string(),
        source_commit_hash: "abc123def4567890abcdef0123456789abcdef01".to_string(),
        build_run_url: "https://github.com/example/implementer/actions/runs/424242".to_string(),
    }
}

#[test]
fn canonical_manifest_round_trips() {
    let m = parse_worker_manifest(CANONICAL_MANIFEST).expect("canonical manifest must parse");
    assert_eq!(m.worker.name, "implementer");
    assert_eq!(m.worker.sdk_version, "0.3.0");
    assert_eq!(m.worker.wire_protocol, "1.0");
    assert_eq!(
        m.capabilities.tags,
        vec!["code-writer".to_string(), "frontier-reasoning".to_string()]
    );
    assert_eq!(
        m.capabilities.compatible_roles,
        vec!["https://decision-cli.dev/ns/role/implementer".to_string()]
    );
    assert_eq!(m.runtime.kind, RuntimeKind::Subscribed);
    assert_eq!(m.runtime.entrypoint, "implementer.main:run");
    assert_eq!(m.tag_prefix(), "implementer-v");
}

#[test]
fn missing_required_table_is_refused() {
    let raw = r#"
[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let err = parse_worker_manifest(raw).expect_err("missing [worker] must fail");
    assert!(
        matches!(err, ManifestParseError::MissingTable { ref table } if table == "worker"),
        "got {err:?}"
    );
}

#[test]
fn missing_required_key_is_refused() {
    let raw = r#"
[worker]
name = "implementer"
# sdk_version missing

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let err = parse_worker_manifest(raw).expect_err("missing sdk_version must fail");
    assert!(
        matches!(
            err,
            ManifestParseError::MissingKey { ref table, ref key }
                if table == "worker" && key == "sdk_version"
        ),
        "got {err:?}"
    );
}

#[test]
fn wire_protocol_falls_back_to_default() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let m = parse_worker_manifest(raw).expect("manifest without wire_protocol must parse");
    assert_eq!(m.worker.wire_protocol, DEFAULT_WIRE_PROTOCOL_VERSION);
}

#[test]
fn unknown_table_is_refused() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"

[secrets]
api_key = "leak"
"#;
    let err = parse_worker_manifest(raw).expect_err("unknown table must fail");
    assert!(matches!(err, ManifestParseError::Syntax { .. }), "got {err:?}");
}

#[test]
fn unknown_key_in_table_is_refused() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"
secret = "leak"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let err = parse_worker_manifest(raw).expect_err("unknown key must fail");
    assert!(
        matches!(
            err,
            ManifestParseError::UnknownKey { ref table, ref key }
                if table == "worker" && key == "secret"
        ),
        "got {err:?}"
    );
}

#[test]
fn unsupported_runtime_value_is_refused_at_parse_time() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "unicorn"
entrypoint = "x"
"#;
    let err = parse_worker_manifest(raw).expect_err("bad runtime.kind must fail");
    assert!(
        matches!(
            err,
            ManifestParseError::UnsupportedValue { ref table, ref key, .. }
                if table == "runtime" && key == "kind"
        ),
        "got {err:?}"
    );
}

#[test]
fn array_in_string_position_is_refused() {
    let raw = r#"
[worker]
name = ["implementer"]
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let err = parse_worker_manifest(raw).expect_err("array-shaped name must fail");
    assert!(
        matches!(
            err,
            ManifestParseError::WrongShape { ref table, ref key, .. }
                if table == "worker" && key == "name"
        ),
        "got {err:?}"
    );
}

#[test]
fn assemble_payload_threads_manifest_fields_through() {
    let m = parse_worker_manifest(CANONICAL_MANIFEST).expect("manifest must parse");
    let outs = build_outputs();
    let payload = assemble_submission_payload(&m, &outs).expect("assembly must succeed");
    assert_eq!(payload.candidate_registry_ref, outs.registry_ref);
    assert_eq!(payload.claimed_sbom_ref, outs.sbom_ref);
    assert_eq!(payload.claimed_signature_subject, outs.signature_subject);
    assert_eq!(payload.claimed_signature_issuer, outs.signature_issuer);
    assert_eq!(payload.claimed_source_repo_uri, outs.source_repo_uri);
    assert_eq!(payload.claimed_source_commit_hash, outs.source_commit_hash);
    assert_eq!(payload.claimed_build_run_url, outs.build_run_url);
    assert_eq!(
        payload.claimed_capability_tags,
        vec!["code-writer".to_string(), "frontier-reasoning".to_string()]
    );
    assert_eq!(
        payload.claimed_compatible_roles,
        vec!["https://decision-cli.dev/ns/role/implementer".to_string()]
    );
}

#[test]
fn assemble_payload_refuses_invoked_runtime() {
    let raw = r#"
[worker]
name = "future-worker"
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "invoked"
entrypoint = "x"
"#;
    let m = parse_worker_manifest(raw).expect("invoked is a parser-accepted value");
    let err = assemble_submission_payload(&m, &build_outputs())
        .expect_err("invoked must be refused at assembly");
    assert_eq!(err, AssembleSubmissionError::UnsupportedRuntime);
}

#[test]
fn assemble_payload_refuses_zero_capability_tags() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"

[capabilities]
tags = []

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let m = parse_worker_manifest(raw).expect("empty tags array parses");
    let err = assemble_submission_payload(&m, &build_outputs())
        .expect_err("zero tags must be refused at assembly");
    assert_eq!(err, AssembleSubmissionError::NoCapabilityTags);
}

#[test]
fn assemble_payload_refuses_missing_build_output() {
    let m = parse_worker_manifest(CANONICAL_MANIFEST).expect("manifest parse");
    let mut outs = build_outputs();
    outs.registry_ref = String::new();
    let err = assemble_submission_payload(&m, &outs)
        .expect_err("empty registry_ref must be refused");
    assert_eq!(
        err,
        AssembleSubmissionError::MissingBuildOutput {
            field: "registry_ref"
        }
    );
}

#[test]
fn empty_compatible_roles_default_to_empty_vec() {
    let raw = r#"
[worker]
name = "implementer"
sdk_version = "0.3.0"

[capabilities]
tags = ["code-writer"]

[runtime]
kind = "subscribed"
entrypoint = "x"
"#;
    let m = parse_worker_manifest(raw).expect("manifest parse");
    assert!(m.capabilities.compatible_roles.is_empty());
    let payload =
        assemble_submission_payload(&m, &build_outputs()).expect("assembly with empty roles ok");
    assert!(payload.claimed_compatible_roles.is_empty());
}

#[test]
fn payload_serialises_into_submission_payload_json_shape() {
    // Round-trip: serialise the core struct, then deserialise it as the
    // feature-side SubmissionPayload. Field-name parity guarantees the
    // workflow's curl-built JSON body matches what the HTTP handler
    // accepts.
    use crate::features::submissions::SubmissionPayload;

    let m = parse_worker_manifest(CANONICAL_MANIFEST).expect("manifest parse");
    let payload = assemble_submission_payload(&m, &build_outputs()).expect("assembly");
    let as_json = serde_json::to_value(&payload).expect("serialise core payload");
    let lifted: SubmissionPayload =
        serde_json::from_value(as_json).expect("deserialise as SubmissionPayload");
    assert_eq!(lifted.candidate_registry_ref, payload.candidate_registry_ref);
    assert_eq!(lifted.claimed_capability_tags, payload.claimed_capability_tags);
    assert_eq!(lifted.claimed_sbom_ref, payload.claimed_sbom_ref);
    assert_eq!(
        lifted.claimed_signature_subject,
        payload.claimed_signature_subject
    );
    assert_eq!(lifted.claimed_source_repo_uri, payload.claimed_source_repo_uri);
    // id and external_origin are derived by the handler.
    assert!(lifted.id.is_none());
    assert!(lifted.external_origin.is_none());
}
