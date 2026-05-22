//! TC-062 — `dec verify env show` returns full env detail and
//!          ArtifactNotFound on unknown id (FT-040 / ADR-029 / ADR-028).
//!
//! Spec: `.product/tests/TC-062-dec-verify-env-show-returns-full-env-detail-and-ar.md`
//! Validates: FT-040 — CLI surface, MCP twin, single handler.
//!
//! The six acceptance criteria map onto six `#[test]` functions that
//! share a tempdir / `dec init` fixture which seeds the canonical
//! `ephemeral-cli` env plus one extra env authored via FT-038.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::core::ontology::verification_env::to_canonical_turtle;
use decision_cli::verify_env_new::{self, EnvNewRequest};
use decision_cli::verify_env_show::{
    self, EnvDocument, EnvShowRequest, EnvShowResponse, OutputFormat,
};
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
        p.push(format!("dec-tc062-{tag}-{pid}-{nonce}"));
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

/// Author one extra env on top of the seed so the fixture matches
/// TC-062's "dec init plus one additional env authored via FT-038".
fn seed_extra_env(workdir: &Path) {
    verify_env_new::run(&EnvNewRequest {
        id: Some("ENV-007".to_string()),
        env_type: "remote-http".to_string(),
        safety_class: "shared-non-destructive".to_string(),
        allowed_ops: vec!["http".to_string()],
        setup: None,
        teardown: None,
        endpoint: Some("https://dev.example.com".to_string()),
        fixture_source: None,
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("env new must succeed for fixture");
}

fn run_show(workdir: &Path, req: EnvShowRequest) -> Result<EnvShowResponse, HandlerError> {
    let mut req = req;
    if req.workdir.is_none() {
        req.workdir = Some(workdir.to_path_buf());
    }
    verify_env_show::run(&req)
}

fn mcp_invoke(workdir: &Path, args: Value) -> Result<EnvShowResponse, HandlerError> {
    let mut obj = args;
    obj.as_object_mut()
        .expect("args is object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_env_show::TOOL_NAME, obj);
    let parsed = verify_env_show::parse_request(&req)?;
    verify_env_show::run(&parsed)
}

// --- AC #1: show seeded env via text format --------------------------------

#[test]
fn ac1_show_seeded_env_returns_full_text_render() {
    let tmp = init_workdir("ac1");
    seed_extra_env(tmp.path());
    let req = EnvShowRequest {
        id: "ENV-001-ephemeral-cli".to_string(),
        format: Some(OutputFormat::Text),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show seed");
    let text = verify_env_show::render_text(&resp);
    // id, env-type, safety-class
    assert!(text.contains("ENV-001-ephemeral-cli"), "id missing: {text}");
    assert!(text.contains("ephemeral-tempdir"), "type missing: {text}");
    assert!(text.contains("isolated"), "safety missing: {text}");
    // each of the seed's allowed ops
    for op in ["shell", "filesystem", "sparql-local"] {
        assert!(text.contains(op), "allowed op {op} missing from: {text}");
    }
    // setup + teardown
    assert!(text.contains("mkdir"), "setup missing: {text}");
    assert!(text.contains("rm -rf"), "teardown missing: {text}");
    // trailing path footer
    assert!(text.contains("Path:"), "path footer missing: {text}");
    assert!(
        text.contains("ENV-001-ephemeral-cli.ttl"),
        "on-disk filename missing: {text}"
    );
}

// --- AC #2: JSON omits absent optional fields -----------------------------

#[test]
fn ac2_json_format_emits_full_env_document() {
    let tmp = init_workdir("ac2");
    seed_extra_env(tmp.path());
    // Show the extra env (no setup / teardown — those keys must be absent).
    let req = EnvShowRequest {
        id: "ENV-007".to_string(),
        format: Some(OutputFormat::Json),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show env-007");
    let s = verify_env_show::render_json(&resp);
    let v: Value = serde_json::from_str(&s).expect("json");
    assert!(v.is_object(), "expected object, got {s}");
    assert_eq!(v["id"], "ENV-007");
    assert_eq!(v["env_type"], "remote-http");
    assert_eq!(v["safety_class"], "shared-non-destructive");
    assert_eq!(v["endpoint"], "https://dev.example.com");
    let ops = v["allowed_ops"].as_array().expect("array");
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0], "http");
    // Optional fields without a value MUST be omitted (not null).
    assert!(v.get("setup").is_none(), "setup must be omitted, got {s}");
    assert!(
        v.get("teardown").is_none(),
        "teardown must be omitted, got {s}"
    );
}

// --- AC #3: round-trip JSON → Turtle equals on-disk file ------------------

#[test]
fn ac3_json_round_trips_back_to_canonical_turtle() {
    let tmp = init_workdir("ac3");
    seed_extra_env(tmp.path());
    let req = EnvShowRequest {
        id: "ENV-001-ephemeral-cli".to_string(),
        format: Some(OutputFormat::Json),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show seed");
    // Parse the JSON back into an `EnvDocument`, project to env, render
    // canonical Turtle. The result must match the on-disk Turtle byte-for-byte.
    let s = verify_env_show::render_json(&resp);
    let doc: EnvDocument = serde_json::from_str(&s).expect("doc");
    let env = doc.to_env().expect("to_env");
    let rendered = to_canonical_turtle(&env);
    let on_disk = std::fs::read_to_string(&resp.path).expect("read on-disk file");
    assert_eq!(
        rendered, on_disk,
        "round-trip Turtle must equal on-disk Turtle"
    );
}

// --- AC #4: MCP parity (CLI JSON ≡ MCP env object) ------------------------

#[test]
fn ac4_mcp_parity_with_cli_json_output() {
    let tmp = init_workdir("ac4");
    seed_extra_env(tmp.path());
    // CLI path: render the env document as JSON.
    let cli = run_show(
        tmp.path(),
        EnvShowRequest {
            id: "ENV-007".to_string(),
            format: Some(OutputFormat::Json),
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect("cli ok");
    let cli_json: Value =
        serde_json::from_str(&verify_env_show::render_json(&cli)).expect("cli json");
    // MCP path: same input via `parse_request` / `run`.
    let mcp = mcp_invoke(
        tmp.path(),
        json!({
            "id": "ENV-007",
            "format": "json",
        }),
    )
    .expect("mcp ok");
    let mcp_json = serde_json::to_value(&mcp.env).expect("ser mcp env");
    assert_eq!(
        cli_json, mcp_json,
        "CLI JSON output must equal MCP env object"
    );
    // Sanity: both produced the same envelope env value.
    assert_eq!(cli.env, mcp.env);
}

// --- AC #5: unknown id surfaces ArtifactNotFound --------------------------

#[test]
fn ac5_unknown_id_returns_artifact_not_found() {
    let tmp = init_workdir("ac5");
    seed_extra_env(tmp.path());
    let err = run_show(
        tmp.path(),
        EnvShowRequest {
            id: "ENV-999".to_string(),
            format: None,
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect_err("missing id must fail");
    match err {
        HandlerError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "VerificationEnvironment");
            assert_eq!(id, "ENV-999");
        }
        other => panic!("expected ArtifactNotFound, got {other:?}"),
    }
    // MCP surface returns the same structured error.
    let mcp_err = mcp_invoke(tmp.path(), json!({"id": "ENV-999"}))
        .expect_err("mcp must also fail with ArtifactNotFound");
    match mcp_err {
        HandlerError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "VerificationEnvironment");
            assert_eq!(id, "ENV-999");
        }
        other => panic!("expected ArtifactNotFound, got {other:?}"),
    }
    // Binary surface: unknown id must exit with code 1 and stderr that
    // names the kind and id.
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("show")
        .arg("ENV-999")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env show ENV-999");
    assert_eq!(
        output.status.code(),
        Some(1),
        "unknown id must exit 1, got {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VerificationEnvironment"),
        "stderr must name kind: {stderr}"
    );
    assert!(stderr.contains("ENV-999"), "stderr must name id: {stderr}");
}

// --- AC #6: malformed id surfaces InvalidArgument -------------------------

#[test]
fn ac6_malformed_id_returns_invalid_argument() {
    let tmp = init_workdir("ac6");
    seed_extra_env(tmp.path());
    let err = run_show(
        tmp.path(),
        EnvShowRequest {
            id: "not-an-id".to_string(),
            format: None,
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect_err("malformed id must fail");
    match err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
    // Binary surface: malformed id must exit with code 2 (usage error).
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("show")
        .arg("not-an-id")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env show not-an-id");
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed id must exit 2, got {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("id"),
        "stderr must name the offending field: {stderr}"
    );
}

// --- Extra: malformed --format exits 2 ------------------------------------

#[test]
fn malformed_format_exits_with_invalid_argument() {
    let tmp = init_workdir("fmt");
    seed_extra_env(tmp.path());
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("show")
        .arg("ENV-001-ephemeral-cli")
        .arg("--format")
        .arg("yaml")
        .current_dir(tmp.path())
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed format must exit 2, got {output:?}"
    );
}

// --- Extra: binary CLI smoke (text + json formats) ------------------------

#[test]
fn binary_cli_text_format_smoke() {
    let tmp = init_workdir("binary-text");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("show")
        .arg("ENV-001-ephemeral-cli")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env show");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains("ENV-001-ephemeral-cli"),
        "expected id in stdout: {stdout}"
    );
    assert!(stdout.contains("ephemeral-tempdir"));
    assert!(stdout.contains("Path:"));
}

#[test]
fn binary_cli_json_format_outputs_object() {
    let tmp = init_workdir("binary-json");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("env")
        .arg("show")
        .arg("ENV-001-ephemeral-cli")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify env show --format json");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert!(v.is_object());
    assert_eq!(v["id"], "ENV-001-ephemeral-cli");
}
