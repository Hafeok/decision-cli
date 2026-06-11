//! Cross-layer parity test (relocated from dec-harness's
//! worker_manifest tests by FT-169): the harness-side
//! `assemble_submission_payload` output must deserialise as the
//! feature-side HTTP `SubmissionPayload`. The test sees both layers, so
//! it lives in the binary crate, the only place that depends on both.

use decision_cli::core::worker_manifest::{
    assemble_submission_payload, parse_worker_manifest, ReleaseBuildOutputs,
};
use decision_cli::features::submissions::SubmissionPayload;

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
fn payload_serialises_into_submission_payload_json_shape() {
    // Round-trip: serialise the core struct, then deserialise it as the
    // feature-side SubmissionPayload. Field-name parity guarantees the
    // workflow's curl-built JSON body matches what the HTTP handler
    // accepts.
    let m = parse_worker_manifest(CANONICAL_MANIFEST).expect("manifest parse");
    let payload = assemble_submission_payload(&m, &build_outputs()).expect("assembly");
    let as_json = serde_json::to_value(&payload).expect("serialise core payload");
    let lifted: SubmissionPayload =
        serde_json::from_value(as_json).expect("deserialise as SubmissionPayload");
    assert_eq!(
        lifted.candidate_registry_ref,
        payload.candidate_registry_ref
    );
    assert_eq!(
        lifted.claimed_capability_tags,
        payload.claimed_capability_tags
    );
    assert_eq!(lifted.claimed_sbom_ref, payload.claimed_sbom_ref);
    assert_eq!(
        lifted.claimed_signature_subject,
        payload.claimed_signature_subject
    );
    assert_eq!(
        lifted.claimed_source_repo_uri,
        payload.claimed_source_repo_uri
    );
    // id and external_origin are derived by the handler.
}
