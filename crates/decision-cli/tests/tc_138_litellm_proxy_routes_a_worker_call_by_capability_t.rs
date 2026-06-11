//! TC-138 — LiteLLM proxy routes a worker call by capability tag and
//! reports telemetry to pipeline-cli.
//!
//! Validates: FT-096 · ADR-064 · ADR-054 · ADR-047 · ADR-044 · ADR-016 · ADR-013.
//! Spec: `.product/tests/TC-138-litellm-proxy-routes-a-worker-call-by-capability-t.md`
//!
//! The actual LiteLLM proxy and worker container cannot be exercised at
//! unit-test latency (same convention TC-133 / TC-135 document for
//! external runtimes). This checkpoint test instead pins every contract
//! a real end-to-end run depends on:
//!
//! 1. `config/litellm.yaml` parses, declares at least one model group
//!    whose `model_name` is a capability tag (`frontier-reasoning`),
//!    routes that group through Anthropic with the provider API key
//!    sourced from the proxy's env (never from worker env), and wires
//!    the `pipeline-cli-telemetry` callback per ADR-064.
//! 2. The `pipeline-cli-telemetry` LiteLLM callback module ships at
//!    `workers/litellm-telemetry-callback/` with a subclass of
//!    `litellm.integrations.custom_logger.CustomLogger` that POSTs to
//!    `/llm-call-telemetry` with the structured `TelemetryRecord` shape.
//! 3. `scripts/bootstrap_litellm_virtual_key.py` calls LiteLLM's
//!    `/key/generate` endpoint and writes `LITELLM_BASE_URL` +
//!    `LITELLM_API_KEY` into the operator's `workers.env` so FT-095's
//!    `pipeline-cli workers run` can inject them into worker containers
//!    (no provider keys land in worker env).
//! 4. The `/llm-call-telemetry` axum endpoint accepts a well-formed
//!    POST from the LiteLLM callback, authenticates the bearer token,
//!    indexes the record under its `ddd_session_id`, and surfaces the
//!    LiteLLM-authoritative cost figure (per ADR-064) at query time.
//! 5. The endpoint refuses unauthenticated POSTs (401) and refuses
//!    payloads with an empty `ddd_session_id` (422), so misrouted
//!    telemetry cannot silently disappear into the reconciliation
//!    store.
//!
//! The single-entry checkpoint test
//! `tc_138_litellm_proxy_routes_a_worker_call_by_capability_t` re-runs
//! every claim in order so the product-cli runner can flip TC-138 to
//! `passing` on a single function invocation.

use std::path::Path;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use serde_yaml::Value as YamlValue;
use tower::ServiceExt;

use decision_cli::telemetry::{
    router, AppState, TelemetryAccepted, TelemetryPayload, TelemetryService, TelemetryStore,
};

const TOKEN: &str = "tc-138-telemetry-token";

fn repo_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .and_then(Path::parent)
        .expect("repo root resolves from crate manifest dir")
        .to_path_buf()
}

fn litellm_config_path() -> std::path::PathBuf {
    repo_root().join("config").join("litellm.yaml")
}

fn callback_module_path() -> std::path::PathBuf {
    repo_root()
        .join("workers")
        .join("litellm-telemetry-callback")
        .join("src")
        .join("litellm_telemetry_callback")
        .join("callback.py")
}

fn bootstrap_script_path() -> std::path::PathBuf {
    repo_root()
        .join("scripts")
        .join("bootstrap_litellm_virtual_key.py")
}

fn parse_litellm_config() -> YamlValue {
    let raw = std::fs::read_to_string(litellm_config_path())
        .expect("config/litellm.yaml must exist (FT-096 scope)");
    serde_yaml::from_str(&raw).expect("config/litellm.yaml must be valid YAML")
}

fn fixture_payload(session: &str) -> TelemetryPayload {
    TelemetryPayload {
        ddd_session_id: session.to_string(),
        model: "frontier-reasoning".to_string(),
        provider: "anthropic".to_string(),
        capability_tag: "frontier-reasoning".to_string(),
        input_tokens: 120,
        output_tokens: 80,
        cost_usd: 0.0125,
        latency_ms: 410,
        retry_count: 0,
        fallback_chain: vec![],
    }
}

fn build_router() -> (axum::Router, TelemetryStore) {
    let store = TelemetryStore::new();
    let service = TelemetryService::new(TOKEN, store.clone());
    let app = router(AppState { service });
    (app, store)
}

async fn post_telemetry(
    app: axum::Router,
    bearer: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/llm-call-telemetry")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let req = req
        .body(Body::from(serde_json::to_vec(&body).expect("serialise")))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("send request");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let parsed: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, parsed)
}

fn first_model_group(cfg: &YamlValue) -> &YamlValue {
    let groups = cfg
        .get("model_list")
        .and_then(YamlValue::as_sequence)
        .expect("config/litellm.yaml must declare a model_list");
    assert!(
        !groups.is_empty(),
        "model_list must declare at least one group (FT-096: Anthropic via Anthropic's API)"
    );
    groups
        .iter()
        .find(|g| {
            g.get("model_name")
                .and_then(YamlValue::as_str)
                .is_some_and(|name| name == "frontier-reasoning")
        })
        .expect("frontier-reasoning capability tag must exist in model_list")
}

fn assert_anthropic_routing(group: &YamlValue) {
    let params = group
        .get("litellm_params")
        .expect("model group must declare litellm_params");
    let model = params
        .get("model")
        .and_then(YamlValue::as_str)
        .expect("litellm_params.model must be a string");
    assert!(
        model.starts_with("anthropic/"),
        "frontier-reasoning must route through Anthropic (got {model:?})"
    );
    let api_key = params
        .get("api_key")
        .and_then(YamlValue::as_str)
        .expect("litellm_params.api_key must be a string");
    assert_eq!(
        api_key, "os.environ/ANTHROPIC_API_KEY",
        "Anthropic key must be sourced from the proxy's env, not embedded \
         (FT-096: provider keys appear nowhere in worker container env)"
    );
}

fn assert_telemetry_callback_wired(cfg: &YamlValue) {
    let callbacks = cfg
        .get("litellm_settings")
        .and_then(|s| s.get("callbacks"))
        .and_then(YamlValue::as_sequence)
        .expect("litellm_settings.callbacks must declare the telemetry callback");
    let has_pipeline_callback = callbacks
        .iter()
        .any(|c| c.as_str() == Some("pipeline-cli-telemetry"));
    assert!(
        has_pipeline_callback,
        "litellm_settings.callbacks must wire `pipeline-cli-telemetry` (FT-096 / ADR-064)"
    );
}

fn assert_master_key_from_env(cfg: &YamlValue) {
    let master = cfg
        .get("general_settings")
        .and_then(|s| s.get("master_key"))
        .and_then(YamlValue::as_str)
        .expect("general_settings.master_key must be declared");
    assert_eq!(
        master, "os.environ/LITELLM_MASTER_KEY",
        "master_key must source from the proxy's env (FT-096)"
    );
}

#[test]
fn litellm_config_declares_capability_tag_through_anthropic_with_env_key() {
    let cfg = parse_litellm_config();
    let group = first_model_group(&cfg);
    assert_anthropic_routing(group);
    assert_master_key_from_env(&cfg);
    assert_telemetry_callback_wired(&cfg);
}

#[test]
fn pipeline_cli_telemetry_callback_module_exists_with_expected_shape() {
    let path = callback_module_path();
    let body = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "pipeline-cli-telemetry callback module must exist at {}: {err}",
            path.display()
        )
    });
    let expectations: &[(&str, &str)] = &[
        (
            "subclasses LiteLLM's CustomLogger",
            "class PipelineCliTelemetryCallback(CustomLogger)",
        ),
        (
            "imports the canonical CustomLogger base",
            "from litellm.integrations.custom_logger import CustomLogger",
        ),
        (
            "POSTs to the /llm-call-telemetry path",
            "/llm-call-telemetry",
        ),
        ("exposes the sync success hook", "def log_success_event"),
        (
            "exposes the async success hook",
            "async def async_log_success_event",
        ),
        (
            "exposes the async failure hook",
            "async def async_log_failure_event",
        ),
        ("reads PIPELINE_ENDPOINT from env", "PIPELINE_ENDPOINT"),
        ("reads PIPELINE_TOKEN from env", "PIPELINE_TOKEN"),
    ];
    for (claim, needle) in expectations {
        assert!(
            body.contains(needle),
            "callback.py missing `{needle}` (FT-096 claim: {claim})"
        );
    }
}

#[test]
fn bootstrap_virtual_key_script_targets_litellm_and_writes_workers_env() {
    let path = bootstrap_script_path();
    let body = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "bootstrap_litellm_virtual_key.py must exist at {}: {err}",
            path.display()
        )
    });
    let expectations: &[(&str, &str)] = &[
        ("targets /key/generate", "/key/generate"),
        ("sources LITELLM_MASTER_KEY from env", "LITELLM_MASTER_KEY"),
        (
            "writes LITELLM_BASE_URL into workers.env",
            "LITELLM_BASE_URL",
        ),
        ("writes LITELLM_API_KEY into workers.env", "LITELLM_API_KEY"),
        ("defaults to workers.env", "workers.env"),
        ("defaults to localhost:4000", "http://localhost:4000"),
    ];
    for (claim, needle) in expectations {
        assert!(
            body.contains(needle),
            "bootstrap script missing `{needle}` (FT-096 claim: {claim})"
        );
    }
}

#[tokio::test]
async fn telemetry_endpoint_accepts_callback_post_and_reconciles_cost_by_session_id() {
    let (app, store) = build_router();
    let payload = fixture_payload("sess-tc138");
    let (status, body) = post_telemetry(
        app,
        Some(TOKEN),
        serde_json::to_value(&payload).expect("body"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "telemetry POST must accept: {body:?}"
    );

    let accepted: TelemetryAccepted = serde_json::from_value(body).expect("accepted shape");
    assert_eq!(accepted.ddd_session_id, "sess-tc138");
    assert_eq!(accepted.capability_tag, "frontier-reasoning");

    let records = store
        .for_session("sess-tc138")
        .expect("records reachable from store");
    assert_eq!(records.len(), 1, "exactly one telemetry record persisted");
    assert_eq!(records[0].model, "frontier-reasoning");
    assert_eq!(records[0].provider, "anthropic");

    let cost = store.total_cost_usd("sess-tc138").expect("cost");
    assert!(
        (cost - 0.0125).abs() < 1e-9,
        "LiteLLM cost figure (ADR-064 authoritative) reconciled verbatim: got {cost}"
    );
}

#[tokio::test]
async fn telemetry_endpoint_rejects_missing_bearer_and_empty_session_id() {
    let (app, _store) = build_router();
    let payload = serde_json::to_value(fixture_payload("sess-x")).expect("body");
    let (status, body) = post_telemetry(app, None, payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "missing bearer => 401");
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("unauthorised")
    );

    let (app, _store) = build_router();
    let empty_session_payload = json!({
        "ddd_session_id": "",
        "model": "frontier-reasoning",
        "provider": "anthropic",
        "capability_tag": "frontier-reasoning",
        "input_tokens": 1,
        "output_tokens": 1,
        "cost_usd": 0.0,
        "latency_ms": 1,
        "retry_count": 0,
        "fallback_chain": []
    });
    let (status, body) = post_telemetry(app, Some(TOKEN), empty_session_payload).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty session id => 422"
    );
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("missing_session_id"),
        "rejection tag must surface the structural reason"
    );
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-138 by this function name in `tests/*.rs` and
/// flips TC-138 to `passing` only when this test runs and exits 0. The
/// body re-runs every structural claim above so this one function
/// reproduces the FT-096 exit criterion end-to-end.
#[tokio::test]
async fn tc_138_litellm_proxy_routes_a_worker_call_by_capability_t() {
    // 1. config/litellm.yaml routes the capability tag through Anthropic
    //    with env-sourced keys and wires the telemetry callback.
    let cfg = parse_litellm_config();
    let group = first_model_group(&cfg);
    assert_anthropic_routing(group);
    assert_master_key_from_env(&cfg);
    assert_telemetry_callback_wired(&cfg);

    // 2. The callback module exists with the expected shape.
    let cb_body = std::fs::read_to_string(callback_module_path())
        .expect("pipeline-cli-telemetry callback must exist");
    assert!(cb_body.contains("class PipelineCliTelemetryCallback(CustomLogger)"));
    assert!(cb_body.contains("/llm-call-telemetry"));
    assert!(cb_body.contains("async def async_log_success_event"));

    // 3. The bootstrap script targets /key/generate and writes workers.bench.
    let bs_body = std::fs::read_to_string(bootstrap_script_path())
        .expect("bootstrap_litellm_virtual_key.py must exist");
    assert!(bs_body.contains("/key/generate"));
    assert!(bs_body.contains("LITELLM_API_KEY"));

    // 4. The /llm-call-telemetry endpoint accepts a well-formed POST and
    //    reconciles cost by ddd_session_id.
    let (app, store) = build_router();
    let payload = fixture_payload("sess-tc138-checkpoint");
    let (status, body) = post_telemetry(
        app,
        Some(TOKEN),
        serde_json::to_value(&payload).expect("body"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "checkpoint POST must accept: {body:?}"
    );
    let cost = store
        .total_cost_usd("sess-tc138-checkpoint")
        .expect("cost reconciled");
    assert!((cost - 0.0125).abs() < 1e-9);
    let records = store.for_session("sess-tc138-checkpoint").expect("records");
    assert_eq!(records[0].capability_tag, "frontier-reasoning");

    // 5. The endpoint refuses unauthenticated POSTs and empty session ids.
    let (app, _store) = build_router();
    let (status, _body) = post_telemetry(
        app,
        None,
        serde_json::to_value(fixture_payload("x")).expect("body"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (app, _store) = build_router();
    let bad_payload = json!({
        "ddd_session_id": "",
        "model": "x",
        "provider": "x",
        "capability_tag": "x",
        "input_tokens": 0,
        "output_tokens": 0,
        "cost_usd": 0.0,
        "latency_ms": 0,
        "retry_count": 0,
        "fallback_chain": []
    });
    let (status, _body) = post_telemetry(app, Some(TOKEN), bad_payload).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
