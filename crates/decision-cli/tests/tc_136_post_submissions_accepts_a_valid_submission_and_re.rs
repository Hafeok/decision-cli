//! TC-136 — POST /submissions accepts a valid Submission and rejects
//! unauthorized or invalid payloads.
//!
//! Validates: FT-094 (decision-cli: `WorkerImageSubmission` HTTP endpoint
//! on pipeline-cli) end-to-end through axum's `Router` via
//! `tower::ServiceExt::oneshot`. Five claims this integration test pins
//! down against a real in-memory Oxigraph store:
//!
//! 1. A well-formed `POST /submissions` from an authorised worker repo
//!    returns 200 with the Submission IRI + a dispatch event id, and
//!    lands the Submission as a `dec:WorkerImageSubmission` /
//!    `dec:InitialRequest` (`BoundaryArtifact` subclass) in the graph.
//! 2. A missing Bearer token returns 401 and writes nothing.
//! 3. An unknown Bearer token returns 401 and writes nothing.
//! 4. A token whose bound identity does not match the declared
//!    `claimed_source_repo_uri` returns 403 and writes nothing.
//! 5. A payload that fails SHACL (missing digest pin, zero capability
//!    tags, etc.) returns 422 with a body whose `detail` lists the
//!    failed field, and writes nothing.
//!
//! Spec: `.product/tests/TC-136-post-submissions-accepts-a-valid-submission-and-re.md`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use oxi_events::GraphWriter;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde_json::{json, Value};
use tower::ServiceExt;

use decision_cli::core::ontology::worker_image_submission::SubmissionLifecycleState;
use decision_cli::submissions::{
    router, AppState, RateLimiter, RepoIdentity, SubmissionsService, TokenStore,
};

const TOKEN: &str = "tc-136-test-token";
const REPO: &str = "https://github.com/example/worker";

fn well_formed_body(repo_uri: &str) -> Value {
    json!({
        "id": "tc-136-sub",
        "candidate_registry_ref": "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "claimed_capability_tags": ["code-writer"],
        "claimed_compatible_roles": [],
        "claimed_sbom_ref": "ghcr.io/example/worker@sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe",
        "claimed_signature_subject": "https://github.com/example/worker/.github/workflows/build.yml@refs/heads/main",
        "claimed_signature_issuer": "https://token.actions.githubusercontent.com",
        "claimed_source_repo_uri": repo_uri,
        "claimed_source_commit_hash": "abc123",
        "claimed_build_run_url": "https://github.com/example/worker/actions/runs/123",
        "external_origin": "github-actions:example/worker/runs/123"
    })
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

fn count_submissions(store: &Store) -> usize {
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission } }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    sols.count()
}

async fn do_post(
    app: axum::Router,
    headers: Vec<(&str, &str)>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/submissions")
        .header(header::CONTENT_TYPE, "application/json");
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let req = req
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

#[tokio::test]
async fn well_formed_post_returns_200_and_lands_in_graph() {
    let harness = build_harness();
    let (status, body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        well_formed_body(REPO),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "well-formed POST must return 200");

    let submission_iri = body
        .get("submission_iri")
        .and_then(Value::as_str)
        .expect("response carries submission_iri");
    assert!(
        submission_iri.ends_with("/tc-136-sub"),
        "submission_iri should end with the supplied id: {submission_iri}"
    );

    let target_role = body
        .get("target_role")
        .and_then(Value::as_str)
        .expect("target_role");
    assert_eq!(target_role, "worker-curator");

    let dispatch_event_id = body
        .get("dispatch_event_id")
        .and_then(Value::as_str)
        .expect("dispatch_event_id");
    assert!(
        dispatch_event_id.starts_with("urn:uuid:"),
        "dispatch_event_id should be a UUID urn: {dispatch_event_id}"
    );

    assert_eq!(
        count_submissions(&harness.store),
        1,
        "exactly one Submission in graph"
    );

    // The persisted lifecycle state must be `received` — client cannot
    // influence the initial state.
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?state WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission ; \
                                          dec:submission_lifecycle_state ?state } }";
    let QueryResults::Solutions(sols) = harness.store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut seen = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("state") {
            seen.push(lit.value().to_string());
        }
    }
    assert_eq!(
        seen,
        vec![SubmissionLifecycleState::Received.as_str().to_string()]
    );

    // The InitialRequest co-type makes the Submission a BoundaryArtifact
    // subclass — verify it is declared.
    let q = "SELECT ?cls WHERE { GRAPH ?g { ?s a ?cls } }";
    let QueryResults::Solutions(sols) = harness.store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut types = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("cls") {
            types.push(n.as_str().to_string());
        }
    }
    assert!(
        types.contains(&"https://decision-cli.dev/ns#InitialRequest".to_string()),
        "Submission must carry rdf:type dec:InitialRequest (BoundaryArtifact subclass): {types:?}"
    );
}

#[tokio::test]
async fn missing_bearer_returns_401_and_writes_nothing() {
    let harness = build_harness();
    let (status, body) = do_post(harness.app, vec![], well_formed_body(REPO)).await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "missing token must be 401"
    );
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("unauthorised"),
        "401 body should carry error tag"
    );
    assert_eq!(
        count_submissions(&harness.store),
        0,
        "401 must not land Submission"
    );
}

#[tokio::test]
async fn unknown_bearer_returns_401_and_writes_nothing() {
    let harness = build_harness();
    let (status, _body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), "Bearer not-a-real-token")],
        well_formed_body(REPO),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(count_submissions(&harness.store), 0);
}

#[tokio::test]
async fn identity_mismatch_returns_403_and_writes_nothing() {
    let harness = build_harness();
    let other_repo = "https://github.com/other/repo";
    let (status, body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        well_formed_body(other_repo),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "identity mismatch must be 403"
    );
    let error = body.get("error").and_then(Value::as_str).unwrap_or("");
    assert_eq!(error, "identity_mismatch");
    let detail = body.get("detail").and_then(Value::as_str).unwrap_or("");
    assert!(
        detail.contains(REPO) && detail.contains(other_repo),
        "detail should name both identities: {detail}"
    );
    assert_eq!(count_submissions(&harness.store), 0);
}

#[tokio::test]
async fn shacl_violation_returns_422_and_writes_nothing() {
    let harness = build_harness();
    let mut body = well_formed_body(REPO);
    // Remove the digest pin so SHACL rejects.
    body["candidate_registry_ref"] = json!("ghcr.io/example/worker:latest");

    let (status, resp) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        body,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "SHACL violation must be 422"
    );
    let error = resp.get("error").and_then(Value::as_str).unwrap_or("");
    assert_eq!(error, "payload_invalid");
    let detail = resp.get("detail").and_then(Value::as_str).unwrap_or("");
    assert!(
        detail.contains("@sha256:"),
        "detail should cite the digest invariant: {detail}"
    );
    assert_eq!(count_submissions(&harness.store), 0);
}

#[tokio::test]
async fn shacl_violation_zero_capability_tags_returns_422() {
    let harness = build_harness();
    let mut body = well_formed_body(REPO);
    body["claimed_capability_tags"] = json!([]);

    let (status, resp) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        body,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let detail = resp.get("detail").and_then(Value::as_str).unwrap_or("");
    assert!(
        detail.contains("claimed_capability_tag"),
        "detail should cite the field: {detail}"
    );
    assert_eq!(count_submissions(&harness.store), 0);
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-136 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs the five structural claims of TC-136 so this one
/// function reproduces the exit-criterion in one shot.
#[tokio::test]
async fn tc_136_post_submissions_accepts_a_valid_submission_and_re() {
    // 1. Well-formed POST returns 200 and lands the Submission.
    let harness = build_harness();
    let (status, body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        well_formed_body(REPO),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let submission_iri = body
        .get("submission_iri")
        .and_then(Value::as_str)
        .expect("submission_iri");
    assert!(submission_iri.ends_with("/tc-136-sub"));
    assert_eq!(
        body.get("target_role").and_then(Value::as_str),
        Some("worker-curator")
    );
    let dispatch_event_id = body
        .get("dispatch_event_id")
        .and_then(Value::as_str)
        .expect("dispatch_event_id");
    assert!(dispatch_event_id.starts_with("urn:uuid:"));
    assert_eq!(count_submissions(&harness.store), 1);

    // 2. Missing Bearer → 401, no write.
    let harness = build_harness();
    let (status, body) = do_post(harness.app, vec![], well_formed_body(REPO)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("unauthorised")
    );
    assert_eq!(count_submissions(&harness.store), 0);

    // 3. Unknown token → 401, no write.
    let harness = build_harness();
    let (status, _) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), "Bearer wrong")],
        well_formed_body(REPO),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(count_submissions(&harness.store), 0);

    // 4. Identity mismatch → 403, no write.
    let harness = build_harness();
    let (status, body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        well_formed_body("https://github.com/other/repo"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("identity_mismatch")
    );
    assert_eq!(count_submissions(&harness.store), 0);

    // 5. SHACL violation (missing digest) → 422, no write.
    let harness = build_harness();
    let mut bad = well_formed_body(REPO);
    bad["candidate_registry_ref"] = json!("ghcr.io/example/worker:latest");
    let (status, body) = do_post(
        harness.app,
        vec![(header::AUTHORIZATION.as_str(), &format!("Bearer {TOKEN}"))],
        bad,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let detail = body.get("detail").and_then(Value::as_str).unwrap_or("");
    assert!(detail.contains("@sha256:"));
    assert_eq!(count_submissions(&harness.store), 0);
}
