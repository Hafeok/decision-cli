//! TC-061 — `dec verify env list` returns ordered envs with safety-class
//!          and type filters (FT-039 / ADR-029 / ADR-028).
//!
//! Spec: `.product/tests/TC-061-dec-verify-env-list-returns-ordered-envs-with-safe.md`
//! Validates: FT-039 — CLI surface, MCP twin, single handler.
//!
//! The eight acceptance criteria map onto eight `#[test]` functions
//! (one for each AC), all sharing a tempdir / `dec init` fixture that
//! seeds three pre-authored envs covering multiple safety classes and
//! types.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::verify_env_list::{self, EnvListRequest, EnvSummary, OutputFormat};
use decision_cli::verify_env_new::{self, EnvNewRequest};
use serde_json::{json, Value};

// --- tempdir helper -------------------------------------------------------

struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new(tag: &str) -> Self {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("dec-tc061-{tag}-{pid}-{nonce}"));
        std::fs::create_dir_all(&p).expect("tmp");
        Self { path: p }
    }
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// --- bootstrap ------------------------------------------------------------

fn write_seed(dir: &Path) {
    let streams = dir.join("streams");
    std::fs::create_dir_all(&streams).expect("streams");
    let body = "@prefix dec: <https://decision-cli.dev/ns#> .\n\
                @prefix va:  <https://decision-cli.dev/ns/value-actions/> .\n\
                <stream:decision-cli-development> a dec:ValueStream ;\n\
                    dec:name                \"decision-cli-development\" ;\n\
                    dec:title               \"decision-cli Development\" ;\n\
                    dec:description         \"Value stream for shipping decision-cli features.\" ;\n\
                    dec:terminalValueAction va:shipped-feature ;\n\
                    dec:authorizedGoals     \"ship\" , \"land\" .\n";
    std::fs::write(streams.join("decision-cli-development.ttl"), body).expect("seed");
}

fn dec_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dec"))
}

fn init_workdir(tag: &str) -> TmpDir {
    let tmp = TmpDir::new(tag);
    write_seed(tmp.path());
    let stream_path = tmp.path().join("streams/decision-cli-development.ttl");
    let status = Command::new(dec_binary())
        .arg("init")
        .arg("--from")
        .arg(&stream_path)
        .current_dir(tmp.path())
        .status()
        .expect("spawn dec init");
    assert!(
        status.code() == Some(0) || status.code() == Some(2),
        "unexpected dec init exit: {status:?}"
    );
    tmp
}

/// Author three additional envs on top of the seed (`ENV-001-ephemeral-cli`):
///   * `ENV-002` — isolated, ephemeral-tempdir (extra ops to vary the row).
///   * `ENV-003` — shared-non-destructive, remote-http.
///   * `ENV-004` — production-readonly, remote-http (with endpoint).
fn seed_three_envs(workdir: &Path) {
    new_env(
        workdir,
        EnvNewRequest {
            id: Some("ENV-002".to_string()),
            env_type: "ephemeral-tempdir".to_string(),
            safety_class: "isolated".to_string(),
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            setup: None,
            teardown: None,
            endpoint: None,
            workdir: Some(workdir.to_path_buf()),
        },
    );
    new_env(
        workdir,
        EnvNewRequest {
            id: Some("ENV-003".to_string()),
            env_type: "remote-http".to_string(),
            safety_class: "shared-non-destructive".to_string(),
            allowed_ops: vec!["http".to_string()],
            setup: None,
            teardown: None,
            endpoint: Some("https://dev.example.com".to_string()),
            workdir: Some(workdir.to_path_buf()),
        },
    );
    new_env(
        workdir,
        EnvNewRequest {
            id: Some("ENV-004".to_string()),
            env_type: "remote-http".to_string(),
            safety_class: "production-readonly".to_string(),
            allowed_ops: vec!["http-readonly".to_string()],
            setup: None,
            teardown: None,
            endpoint: Some("https://prod.example.com".to_string()),
            workdir: Some(workdir.to_path_buf()),
        },
    );
}

fn new_env(_workdir: &Path, req: EnvNewRequest) {
    verify_env_new::run(&req).expect("env new must succeed for fixture");
}

fn run_list(workdir: &Path, req: EnvListRequest) -> Vec<EnvSummary> {
    let mut req = req;
    if req.workdir.is_none() {
        req.workdir = Some(workdir.to_path_buf());
    }
    let resp = verify_env_list::run(&req).expect("env list ok");
    resp.envs
}

fn mcp_invoke(workdir: &Path, args: Value) -> Result<verify_env_list::EnvListResponse, HandlerError> {
    let mut obj = args;
    obj.as_object_mut()
        .expect("args is object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_env_list::TOOL_NAME, obj);
    let parsed = verify_env_list::parse_request(&req)?;
    verify_env_list::run(&parsed)
}

// --- AC #1: empty store (only the seeded ephemeral-cli env) -----------------

#[test]
fn ac1_empty_store_returns_single_seed_env() {
    let tmp = init_workdir("ac1");
    let envs = run_list(tmp.path(), EnvListRequest::default());
    assert_eq!(envs.len(), 1, "exactly the seeded env should be present");
    assert!(
        envs[0].id.starts_with("ENV-001"),
        "expected ENV-001 prefix, got {:?}",
        envs[0].id
    );
    assert_eq!(envs[0].safety_class, "isolated");
    assert_eq!(envs[0].env_type, "ephemeral-tempdir");

    // --format json over the same store returns a one-element array.
    let resp = verify_env_list::EnvListResponse { envs };
    let json = verify_env_list::render_json(&resp);
    let v: Value = serde_json::from_str(&json).expect("json");
    assert!(v.is_array());
    assert_eq!(v.as_array().expect("arr").len(), 1);
}

// --- AC #2: order ----------------------------------------------------------

#[test]
fn ac2_envs_returned_in_ascending_numeric_order() {
    let tmp = init_workdir("ac2");
    seed_three_envs(tmp.path());
    let envs = run_list(tmp.path(), EnvListRequest::default());
    let ids: Vec<&str> = envs.iter().map(|e| e.id.as_str()).collect();
    // Order is ENV-001-ephemeral-cli, ENV-002, ENV-003, ENV-004
    // (the seed env's suffix sorts after pure ENV-001 — but here only
    // ENV-001-ephemeral-cli exists at the "001" slot, so ascending by
    // numeric tail then string puts it first).
    assert_eq!(ids.len(), 4);
    assert!(ids[0].starts_with("ENV-001"));
    assert_eq!(ids[1], "ENV-002");
    assert_eq!(ids[2], "ENV-003");
    assert_eq!(ids[3], "ENV-004");
}

// --- AC #3: filter by safety class -----------------------------------------

#[test]
fn ac3_filter_by_safety_class() {
    let tmp = init_workdir("ac3");
    seed_three_envs(tmp.path());
    let req = EnvListRequest {
        safety_class: Some("isolated".to_string()),
        ..Default::default()
    };
    let envs = run_list(tmp.path(), req);
    // Seed (ephemeral-cli) is isolated; ENV-002 is isolated.
    assert!(envs.iter().all(|e| e.safety_class == "isolated"));
    assert!(
        envs.iter().any(|e| e.id == "ENV-002"),
        "isolated set must include ENV-002"
    );
    assert!(
        !envs.iter().any(|e| e.id == "ENV-003"),
        "isolated set must NOT include ENV-003 (shared-non-destructive)"
    );
}

// --- AC #4: filter by env type --------------------------------------------

#[test]
fn ac4_filter_by_env_type() {
    let tmp = init_workdir("ac4");
    seed_three_envs(tmp.path());
    let req = EnvListRequest {
        env_type: Some("remote-http".to_string()),
        ..Default::default()
    };
    let envs = run_list(tmp.path(), req);
    assert!(envs.iter().all(|e| e.env_type == "remote-http"));
    let ids: Vec<&str> = envs.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["ENV-003", "ENV-004"]);
}

// --- AC #5: combined filters apply conjunctively ---------------------------

#[test]
fn ac5_combined_filters_are_conjunctive() {
    let tmp = init_workdir("ac5");
    seed_three_envs(tmp.path());
    let req = EnvListRequest {
        safety_class: Some("shared-non-destructive".to_string()),
        env_type: Some("remote-http".to_string()),
        ..Default::default()
    };
    let envs = run_list(tmp.path(), req);
    assert_eq!(envs.len(), 1);
    assert_eq!(envs[0].id, "ENV-003");
    assert_eq!(envs[0].safety_class, "shared-non-destructive");
    assert_eq!(envs[0].env_type, "remote-http");
}

// --- AC #6: JSON shape -----------------------------------------------------

#[test]
fn ac6_json_shape_omits_absent_optional_fields() {
    let tmp = init_workdir("ac6");
    seed_three_envs(tmp.path());
    let envs = run_list(tmp.path(), EnvListRequest::default());
    let resp = verify_env_list::EnvListResponse { envs };
    let s = verify_env_list::render_json(&resp);
    let arr: Value = serde_json::from_str(&s).expect("json");
    let arr = arr.as_array().expect("array");
    // Every entry has required keys.
    for entry in arr {
        let obj = entry.as_object().expect("object");
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("env_type"));
        assert!(obj.contains_key("safety_class"));
        assert!(obj.contains_key("allowed_ops"));
    }
    // ENV-002 has no endpoint / setup / teardown — those keys must be absent.
    let env002 = arr
        .iter()
        .find(|v| v["id"] == "ENV-002")
        .expect("ENV-002 in JSON output");
    assert!(env002.get("endpoint").is_none(), "endpoint must be omitted");
    assert!(env002.get("setup").is_none(), "setup must be omitted");
    assert!(env002.get("teardown").is_none(), "teardown must be omitted");
    // ENV-003 carries an endpoint — must be present.
    let env003 = arr
        .iter()
        .find(|v| v["id"] == "ENV-003")
        .expect("ENV-003 in JSON output");
    assert_eq!(env003["endpoint"], "https://dev.example.com");
}

// --- AC #7: MCP parity (CLI JSON ≡ MCP `envs` array) -----------------------

#[test]
fn ac7_mcp_parity_with_cli_json_output() {
    let tmp = init_workdir("ac7");
    seed_three_envs(tmp.path());
    // CLI path.
    let cli_envs = run_list(tmp.path(), EnvListRequest::default());
    let cli_resp = verify_env_list::EnvListResponse {
        envs: cli_envs.clone(),
    };
    let cli_json: Value =
        serde_json::from_str(&verify_env_list::render_json(&cli_resp)).expect("cli json");
    // MCP path: invoke through `tool_descriptor` / parse_request.
    let mcp_resp = mcp_invoke(tmp.path(), json!({})).expect("mcp ok");
    // Compare element-for-element.
    assert_eq!(mcp_resp.envs.len(), cli_envs.len(), "lengths must match");
    let mcp_arr = serde_json::to_value(&mcp_resp.envs).expect("ser");
    assert_eq!(cli_json, mcp_arr, "CLI JSON ≡ MCP `envs` array");
}

// --- AC #8: invalid filter -------------------------------------------------

#[test]
fn ac8_invalid_safety_class_returns_invalid_argument() {
    let tmp = init_workdir("ac8-cli");
    seed_three_envs(tmp.path());
    let req = EnvListRequest {
        safety_class: Some("yolo".to_string()),
        workdir: Some(tmp.path().to_path_buf()),
        ..Default::default()
    };
    let err = verify_env_list::run(&req).expect_err("yolo must fail");
    match err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "safety_class"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
    // MCP surface returns the same structured error.
    let tmp2 = init_workdir("ac8-mcp");
    seed_three_envs(tmp2.path());
    let mcp_err = mcp_invoke(tmp2.path(), json!({"safety_class": "yolo"}))
        .expect_err("yolo must fail on MCP too");
    match mcp_err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "safety_class"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

// --- Extra: invalid format on the CLI surface ------------------------------

#[test]
fn invalid_format_value_exits_with_invalid_argument() {
    // The CLI helper `cli::verify::env_list_request` lives in the binary
    // crate; we exercise the same map directly via OutputFormat::parse.
    assert!(OutputFormat::parse("yaml").is_none());
}

// --- Extra: round-trip CLI invocation via the binary ----------------------

#[test]
fn binary_cli_table_format_smoke() {
    let tmp = init_workdir("binary-smoke");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("list")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env list");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("ENV-001"), "expected ENV-001 in output: {stdout}");
}

#[test]
fn binary_cli_json_format_outputs_array() {
    let tmp = init_workdir("binary-json");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("list")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env list --format json");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert!(v.is_array());
}
