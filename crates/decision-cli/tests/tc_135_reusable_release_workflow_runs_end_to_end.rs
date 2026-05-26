//! TC-135 — Reusable release workflow runs end-to-end and posts a
//! `WorkerImageSubmission`.
//!
//! Validates: FT-093 · ADR-061 · ADR-013 · ADR-016 · ADR-044.
//! Spec: `.product/tests/TC-135-reusable-release-workflow-runs-end-to-end-and-post.md`
//!
//! The actual GitHub Actions workflow itself cannot be exercised at
//! unit-test latency (per the same convention TC-133 documents for
//! FT-091's signing primitive). This checkpoint test instead pins the
//! *contract* the workflow produces — a Submission whose fields are
//! lifted from a `worker.toml` manifest and the workflow's build
//! outputs — and verifies that the admission substrate accepts the
//! result end-to-end through axum.
//!
//! Six structural claims this single integration test ties together:
//!
//! 1. The canonical FT-093 `worker.toml` manifest at
//!    `tests/data/ft_093_worker_manifest.toml` parses cleanly into a
//!    `core::worker_manifest::WorkerManifest`. Mirrors the template
//!    that ships at `docs/templates/worker.toml`.
//! 2. `core::worker_manifest::assemble_submission_payload` lifts
//!    `(manifest + ReleaseBuildOutputs)` into a
//!    `SubmissionPayloadFields` whose serialised JSON shape matches
//!    `features::submissions::SubmissionPayload` exactly. This is the
//!    contract the workflow's curl command depends on.
//! 3. Posting that payload to `/submissions` with the matching bearer
//!    token returns 200 with a `submission_id` + `dispatch_event_id`,
//!    lands a `dec:WorkerImageSubmission` in the orchestration graph,
//!    and the dispatch target role is `worker-curator`.
//! 4. The persisted Submission carries every manifest-derived
//!    capability tag in `dec:claimed_capability_tag`, the FT-088
//!    sigstore + provenance fields, and the SBOM referrer URI verbatim.
//! 5. The reusable workflow file `.github/workflows/release-worker-full.yml`
//!    exists and declares the FT-093 build → push → sign → submit
//!    primitive set in its job graph.
//! 6. The consumer template `docs/templates/release.yml` pins
//!    `release-worker-full.yml@v1` per ADR-061's explicit-opt-in
//!    versioning contract.
//!
//! The single-entry checkpoint test
//! `tc_135_reusable_release_workflow_runs_end_to_end_and_post` re-runs
//! every claim in order so the product-cli runner can flip TC-135 to
//! `passing` on a single function invocation.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use oxi_events::GraphWriter;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde_json::Value;
use tower::ServiceExt;

use decision_cli::core::ontology::worker_image_submission::SubmissionLifecycleState;
use decision_cli::core::worker_manifest::{
    assemble_submission_payload, parse_worker_manifest, ReleaseBuildOutputs, RuntimeKind,
};
use decision_cli::submissions::{
    router, AppState, RateLimiter, RepoIdentity, SubmissionPayload, SubmissionsService, TokenStore,
};

const TOKEN: &str = "tc-135-test-token";
const REPO: &str = "https://github.com/example/implementer";
const REGISTRY_DIGEST_HEX: &str =
    "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
const SBOM_DIGEST_HEX: &str =
    "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";
const COMMIT_SHA: &str = "abc123def4567890abcdef0123456789abcdef01";

/// Repo-root path resolution. `CARGO_MANIFEST_DIR` for the
/// decision-cli crate is `<repo>/crates/decision-cli`; the workflow
/// files live two levels up under `.github/workflows/`.
fn repo_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .and_then(Path::parent)
        .expect("repo root resolves from crate manifest dir")
        .to_path_buf()
}

fn canonical_manifest_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("ft_093_worker_manifest.toml")
}

fn release_workflow_path() -> std::path::PathBuf {
    repo_root()
        .join(".github")
        .join("workflows")
        .join("release-worker-full.yml")
}

fn consumer_template_path() -> std::path::PathBuf {
    repo_root()
        .join("docs")
        .join("templates")
        .join("release.yml")
}

/// Build outputs the FT-093 workflow would produce on a successful run:
/// the `buildx push` digest, the `cosign attach sbom` referrer URI, the
/// `cosign sign --keyless` identity, and the GitHub Actions provenance
/// fields.
fn canonical_build_outputs() -> ReleaseBuildOutputs {
    ReleaseBuildOutputs {
        registry_ref: format!("ghcr.io/example/implementer@sha256:{REGISTRY_DIGEST_HEX}"),
        sbom_ref: format!("ghcr.io/example/implementer@sha256:{SBOM_DIGEST_HEX}"),
        signature_subject: format!(
            "https://github.com/example/implementer/.github/workflows/release.yml@refs/tags/implementer-v1.2.0"
        ),
        signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        source_repo_uri: REPO.to_string(),
        source_commit_hash: COMMIT_SHA.to_string(),
        build_run_url: "https://github.com/example/implementer/actions/runs/424242".to_string(),
    }
}

struct Harness {
    app: axum::Router,
    store: Arc<Store>,
}

fn build_harness() -> Harness {
    let store = Arc::new(Store::new().expect("memory store"));
    let writer = Arc::new(GraphWriter::open(store.clone()).expect("graph writer"));
    let identity = RepoIdentity::new(REPO).expect("identity");
    let tokens = TokenStore::single(TOKEN, identity).expect("token store");
    let limiter = Arc::new(RateLimiter::with_default_policy());
    let service = SubmissionsService::new(writer, tokens, limiter);
    let app = router(AppState { service });
    Harness { app, store }
}

async fn post_payload(app: axum::Router, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/submissions")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::from(
            serde_json::to_vec(&body).expect("serialise body"),
        ))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("send request");
    let status = resp.status();
    let body_bytes = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let parsed: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
    };
    (status, parsed)
}

fn capability_tags_in_graph(store: &Store) -> Vec<String> {
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?tag WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission ; \
                                          dec:claimed_capability_tag ?tag } }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("tag") {
            out.push(lit.value().to_string());
        }
    }
    out.sort();
    out
}

fn single_literal(store: &Store, predicate: &str) -> String {
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH ?g {{ ?s <{predicate}> ?o }} }}"
    );
    let QueryResults::Solutions(sols) = store.query(&q).expect("query ok") else {
        panic!("expected solutions for {predicate}");
    };
    let mut out: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("o") {
            out.push(lit.value().to_string());
        }
    }
    assert_eq!(out.len(), 1, "expected exactly one literal for {predicate}, got {out:?}");
    out.pop().expect("non-empty")
}

#[test]
fn canonical_manifest_fixture_parses_into_subscribed_implementer() {
    let raw = std::fs::read_to_string(canonical_manifest_path())
        .expect("canonical manifest fixture is committed under tests/data/");
    let m = parse_worker_manifest(&raw).expect("manifest must parse");
    assert_eq!(m.worker.name, "implementer");
    assert_eq!(m.worker.sdk_version, "0.3.0");
    assert_eq!(m.worker.wire_protocol, "1.0");
    assert_eq!(m.runtime.kind, RuntimeKind::Subscribed);
    assert_eq!(m.runtime.entrypoint, "implementer.main:run");
    assert_eq!(m.tag_prefix(), "implementer-v");
    assert_eq!(
        m.capabilities.tags,
        vec!["code-writer".to_string(), "frontier-reasoning".to_string()]
    );
}

#[test]
fn assembler_output_round_trips_through_submission_payload_json() {
    let raw = std::fs::read_to_string(canonical_manifest_path()).expect("manifest");
    let manifest = parse_worker_manifest(&raw).expect("parse");
    let payload = assemble_submission_payload(&manifest, &canonical_build_outputs())
        .expect("assembly");
    let as_json = serde_json::to_value(&payload).expect("serialise");
    let lifted: SubmissionPayload =
        serde_json::from_value(as_json).expect("SubmissionPayload deserialise");
    assert_eq!(lifted.candidate_registry_ref, payload.candidate_registry_ref);
    assert_eq!(lifted.claimed_sbom_ref, payload.claimed_sbom_ref);
    assert_eq!(
        lifted.claimed_capability_tags,
        payload.claimed_capability_tags
    );
    assert!(lifted.id.is_none());
    assert!(lifted.external_origin.is_none());
}

#[tokio::test]
async fn assembled_payload_lands_in_graph_via_submissions_endpoint() {
    let harness = build_harness();
    let raw = std::fs::read_to_string(canonical_manifest_path()).expect("manifest");
    let manifest = parse_worker_manifest(&raw).expect("parse");
    let outputs = canonical_build_outputs();
    let payload = assemble_submission_payload(&manifest, &outputs).expect("assembly");
    let body = serde_json::to_value(&payload).expect("body");

    let (status, response) = post_payload(harness.app, body).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "FT-094 must accept FT-093-assembled payload: {response:?}"
    );
    assert_eq!(
        response.get("target_role").and_then(Value::as_str),
        Some("worker-curator"),
        "dispatched role must be the WorkerCurator"
    );
    assert!(response
        .get("dispatch_event_id")
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with("urn:uuid:")));
    assert!(response
        .get("submission_iri")
        .and_then(Value::as_str)
        .is_some());

    // Capability tags lifted from the manifest must reach the graph.
    let tags = capability_tags_in_graph(&harness.store);
    assert_eq!(
        tags,
        vec!["code-writer".to_string(), "frontier-reasoning".to_string()]
    );

    // SBOM referrer URI lifted verbatim.
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#claimed_sbom_ref"
        ),
        outputs.sbom_ref
    );
    // Lifecycle pinned to received.
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#submission_lifecycle_state"
        ),
        SubmissionLifecycleState::Received.as_str().to_string()
    );
    // External origin default for FT-094 is the build run URL.
    assert_eq!(
        single_literal(&harness.store, "https://decision-cli.dev/ns#external_origin"),
        outputs.build_run_url
    );
    // Signature subject preserved.
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#claimed_signature_subject"
        ),
        outputs.signature_subject
    );
}

#[test]
fn release_workflow_yaml_declares_the_full_release_primitive_set() {
    let path = release_workflow_path();
    let body = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "FT-093 reusable workflow must exist at {}: {err}",
            path.display()
        )
    });
    let expectations: &[(&str, &str)] = &[
        ("workflow_call trigger", "workflow_call:"),
        ("worker_name input", "worker_name:"),
        ("worker_manifest_path input", "worker_manifest_path:"),
        ("worker_dockerfile_path input", "worker_dockerfile_path:"),
        ("image_repo input", "image_repo:"),
        ("submission_endpoint input", "submission_endpoint:"),
        ("submission_token secret", "submission_token:"),
        ("buildx multi-arch build", "docker buildx build"),
        ("ghcr.io push", "ghcr.io"),
        (
            "delegates to FT-089/FT-091 signing primitive",
            "release-worker.yml",
        ),
        ("POST /submissions step", "POST /submissions"),
        ("candidate_registry_ref jq field", "candidate_registry_ref"),
        ("claimed_capability_tags jq field", "claimed_capability_tags"),
        ("claimed_sbom_ref jq field", "claimed_sbom_ref"),
        ("claimed_signature_subject jq field", "claimed_signature_subject"),
        ("bearer-token auth header", "Authorization: Bearer"),
    ];
    for (claim, needle) in expectations {
        assert!(
            body.contains(needle),
            "release-worker-full.yml missing `{needle}` (FT-093 claim: {claim})"
        );
    }
}

#[test]
fn consumer_template_pins_to_v1_per_adr_061() {
    let body = std::fs::read_to_string(consumer_template_path())
        .expect("consumer template must exist under docs/templates/release.yml");
    assert!(
        body.contains("release-worker-full.yml@v1"),
        "consumer release.yml must pin `release-worker-full.yml@v1` per ADR-061"
    );
    assert!(
        body.contains("PIPELINE_SUBMISSION_TOKEN"),
        "consumer release.yml must thread the FT-094 submission token secret"
    );
    assert!(
        body.contains("id-token: write"),
        "consumer release.yml must request the OIDC id-token permission for keyless cosign (FT-089)"
    );
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-135 by this function name in `tests/*.rs` and
/// flips TC-135 to `passing` only when this test runs and exits 0. The
/// body re-runs the six structural claims of TC-135 so this one
/// function reproduces the exit-criterion end-to-end.
#[tokio::test]
async fn tc_135_reusable_release_workflow_runs_end_to_end_and_post() {
    // 1. The canonical manifest fixture parses cleanly.
    let raw = std::fs::read_to_string(canonical_manifest_path())
        .expect("canonical manifest fixture is committed under tests/data/");
    let manifest = parse_worker_manifest(&raw).expect("manifest must parse");
    assert_eq!(manifest.worker.name, "implementer");
    assert_eq!(manifest.runtime.kind, RuntimeKind::Subscribed);

    // 2. Manifest + ReleaseBuildOutputs assemble into a payload whose
    //    JSON shape matches features::submissions::SubmissionPayload.
    let outputs = canonical_build_outputs();
    let payload = assemble_submission_payload(&manifest, &outputs).expect("assembly");
    let body = serde_json::to_value(&payload).expect("serialise payload");
    let lifted: SubmissionPayload =
        serde_json::from_value(body.clone()).expect("payload round-trip via SubmissionPayload");
    assert_eq!(
        lifted.candidate_registry_ref,
        payload.candidate_registry_ref,
        "JSON shape parity between core::worker_manifest::SubmissionPayloadFields and features::submissions::SubmissionPayload"
    );

    // 3. POSTing the assembled payload to /submissions returns 200 with
    //    a submission_id + dispatch_event_id and lands a
    //    dec:WorkerImageSubmission in the graph.
    let harness = build_harness();
    let (status, response) = post_payload(harness.app, body).await;
    assert_eq!(status, StatusCode::OK, "FT-094 must accept FT-093 payload");
    assert_eq!(
        response.get("target_role").and_then(Value::as_str),
        Some("worker-curator")
    );
    assert!(response
        .get("dispatch_event_id")
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with("urn:uuid:")));
    let submission_iri = response
        .get("submission_iri")
        .and_then(Value::as_str)
        .expect("submission_iri");
    assert!(submission_iri.starts_with("https://decision-cli.dev/ns/worker-image-submission/"));

    // 4. Persisted Submission carries every manifest-derived capability
    //    tag and the sigstore + provenance fields verbatim.
    assert_eq!(
        capability_tags_in_graph(&harness.store),
        vec!["code-writer".to_string(), "frontier-reasoning".to_string()]
    );
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#claimed_sbom_ref"
        ),
        outputs.sbom_ref
    );
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#candidate_registry_ref"
        ),
        outputs.registry_ref
    );
    assert_eq!(
        single_literal(
            &harness.store,
            "https://decision-cli.dev/ns#claimed_source_commit_hash"
        ),
        outputs.source_commit_hash
    );

    // 5. The reusable workflow YAML exists and declares each primitive
    //    step (buildx, push, sign delegation, submit).
    let workflow_body = std::fs::read_to_string(release_workflow_path())
        .expect("release-worker-full.yml must exist");
    for needle in [
        "workflow_call:",
        "docker buildx build",
        "ghcr.io",
        "release-worker.yml",
        "POST /submissions",
        "claimed_capability_tags",
        "claimed_sbom_ref",
        "Authorization: Bearer",
    ] {
        assert!(
            workflow_body.contains(needle),
            "release-worker-full.yml missing `{needle}`"
        );
    }

    // 6. Consumer template pins @v1 per ADR-061.
    let consumer_body = std::fs::read_to_string(consumer_template_path())
        .expect("consumer template release.yml must exist");
    assert!(
        consumer_body.contains("release-worker-full.yml@v1"),
        "consumer template must pin to @v1 per ADR-061"
    );
    assert!(
        consumer_body.contains("PIPELINE_SUBMISSION_TOKEN"),
        "consumer template must thread the FT-094 submission token secret"
    );
}
