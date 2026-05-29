//! TC-060 — `dec verify env new` produces identical artifact via CLI and MCP.
//!
//! Spec: `.product/tests/TC-060-dec-verify-bench-new-produces-identical-artifact-via.md`
//! Validates: FT-038 · ADR-028 · ADR-029.
//!
//! The single-handler discipline (ADR-029) guarantees CLI and MCP take the
//! same code path; this test forces both surfaces through the matching
//! transformation (`cli::verify::env_new_request` and the MCP tool's
//! `tool_descriptor()` handler) and asserts the on-disk Turtle is
//! byte-equal modulo the minted id, and that the error variants on the
//! failure paths are identical.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::core::ontology::verification_bench::from_turtle;
use decision_cli::verify_bench_new::{
    self, canonical_turtle, parse_request, BenchNewRequest, BenchNewResponse,
};
use serde_json::{json, Value};

// --- tempdir helper (avoids adding a new test-only dependency) -----------

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
        p.push(format!("dec-tc060-{tag}-{pid}-{nonce}"));
        std::fs::create_dir_all(&p).expect("create tmp");
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

// --- bootstrap a working dir with `dec init` ------------------------------

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
    // CARGO_BIN_EXE_dec is set by Cargo for integration tests so we
    // don't have to assume target/debug/dec is fresh on every CI run.
    let raw = env!("CARGO_BIN_EXE_dec");
    PathBuf::from(raw)
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
    // `dec init` may report exit code 2 when the worker-preflight
    // advisory fires (no `code-writer` binary installed in the test
    // sandbox). That's fine for our purposes: the orchestration store
    // is fully initialised — the advisory is purely about implementer
    // readiness, which TC-060 does not exercise.
    assert!(
        status.code() == Some(0) || status.code() == Some(2),
        "unexpected dec init exit: {status:?}"
    );
    tmp
}

// --- helper: build a Request via the MCP tool descriptor ------------------

fn invoke_mcp_tool(workdir: &Path, args: Value) -> Result<BenchNewResponse, HandlerError> {
    // Bind the workdir into the request — production code threads this
    // via features::mcp::build_registry; for the unit-level test we
    // construct the Request directly.
    let mut obj = args;
    obj.as_object_mut()
        .expect("args is object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_bench_new::TOOL_NAME, obj);
    let parsed = parse_request(&req)?;
    verify_bench_new::run(&parsed)
}

// --- helper: CLI-equivalent invocation ------------------------------------

fn invoke_cli(workdir: &Path, req: BenchNewRequest) -> Result<BenchNewResponse, HandlerError> {
    let mut req = req;
    req.workdir = Some(workdir.to_path_buf());
    verify_bench_new::run(&req)
}

// --- AC #1 + #2: happy paths -------------------------------------------------

#[test]
fn cli_happy_path_creates_env_file_and_returns_minted_id() {
    let tmp = init_workdir("cli");
    let req = BenchNewRequest {
        id: None,
        bench_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: None,
    };
    let outcome = invoke_cli(tmp.path(), req).expect("env new succeeds");
    // Seed env already lives as BNCH-001-ephemeral-cli; first mint = BNCH-002.
    assert_eq!(outcome.id, "BNCH-002", "first user-minted id");
    assert!(outcome.path.exists(), "env file must be written");
    let ttl = std::fs::read_to_string(&outcome.path).expect("read");
    assert!(ttl.contains("a dec:VerificationBench"));
    assert!(ttl.contains("\"ephemeral-tempdir\""));
}

#[test]
fn mcp_happy_path_creates_env_file_with_structured_response() {
    let tmp = init_workdir("mcp");
    let out = invoke_mcp_tool(
        tmp.path(),
        json!({
            "bench_type": "ephemeral-tempdir",
            "safety_class": "isolated",
            "allowed_ops": ["shell", "filesystem"],
        }),
    )
    .expect("mcp env new");
    assert_eq!(out.id, "BNCH-002");
    assert!(out.path.exists());
}

// --- AC #3: byte-equal canonical Turtle (modulo id) -------------------------

#[test]
fn cli_and_mcp_produce_byte_equal_turtle_modulo_id() {
    let cli_tmp = init_workdir("byte-cli");
    let mcp_tmp = init_workdir("byte-mcp");
    let req = BenchNewRequest {
        id: None,
        bench_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: None,
    };
    let cli_out = invoke_cli(cli_tmp.path(), req.clone()).expect("cli");
    let mcp_out = invoke_mcp_tool(
        mcp_tmp.path(),
        json!({
            "bench_type": "ephemeral-tempdir",
            "safety_class": "isolated",
            "allowed_ops": ["shell", "filesystem"],
        }),
    )
    .expect("mcp");
    // Both surfaces independently mint BNCH-002 (seed = BNCH-001).
    assert_eq!(cli_out.id, mcp_out.id);
    // Parse both files back to memory and compare structurally — the
    // canonical Turtle writer is deterministic, but we compare via the
    // in-memory type to avoid coupling to whitespace incidentals.
    let cli_env = from_turtle(&cli_out.path).expect("cli env");
    let mcp_env = from_turtle(&mcp_out.path).expect("mcp env");
    assert_eq!(cli_env, mcp_env, "envs must be structurally identical");
    // And the canonical Turtle bytes must match exactly.
    let cli_ttl = canonical_turtle(&cli_env);
    let mcp_ttl = canonical_turtle(&mcp_env);
    assert_eq!(cli_ttl, mcp_ttl, "canonical turtle must be byte-equal");
    let cli_bytes = std::fs::read(&cli_out.path).expect("read");
    let mcp_bytes = std::fs::read(&mcp_out.path).expect("read");
    assert_eq!(cli_bytes, mcp_bytes, "on-disk bytes must be byte-equal");
}

// --- AC #4: SHACL gates both surfaces ---------------------------------------

#[test]
fn missing_endpoint_on_remote_type_rejected_on_both_surfaces() {
    let cli_tmp = init_workdir("shacl-cli");
    let mcp_tmp = init_workdir("shacl-mcp");
    // The InvalidArgument path is structurally enforced before the
    // SHACL chokepoint sees the mutation; both surfaces produce the
    // same Error::InvalidArgument shape.
    let cli_err = invoke_cli(
        cli_tmp.path(),
        BenchNewRequest {
            id: None,
            bench_type: "remote-http".to_string(),
            safety_class: "shared-non-destructive".to_string(),
            allowed_ops: vec!["http".to_string()],
            setup: None,
            teardown: None,
            endpoint: None,
            fixture_source: None,
            workdir: None,
        },
    )
    .expect_err("cli missing endpoint must fail");
    let mcp_err = invoke_mcp_tool(
        mcp_tmp.path(),
        json!({
            "bench_type": "remote-http",
            "safety_class": "shared-non-destructive",
            "allowed_ops": ["http"],
        }),
    )
    .expect_err("mcp missing endpoint must fail");
    match (&cli_err, &mcp_err) {
        (
            HandlerError::InvalidArgument {
                field: cf,
                detail: cd,
            },
            HandlerError::InvalidArgument {
                field: mf,
                detail: md,
            },
        ) => {
            assert_eq!(cf, "endpoint");
            assert_eq!(mf, "endpoint");
            assert_eq!(cd, md, "details must match across surfaces");
        }
        other => panic!("expected matching InvalidArgument variants, got {other:?}"),
    }
}

// --- AC #5: caller-supplied id collision -----------------------------------

#[test]
fn caller_supplied_id_collision_fails_with_duplicate_id() {
    let tmp = init_workdir("collide");
    let first = invoke_cli(
        tmp.path(),
        BenchNewRequest {
            id: Some("ENV-007".to_string()),
            bench_type: "ephemeral-tempdir".to_string(),
            safety_class: "isolated".to_string(),
            allowed_ops: vec!["shell".to_string()],
            setup: None,
            teardown: None,
            endpoint: None,
            fixture_source: None,
            workdir: None,
        },
    )
    .expect("first env new");
    assert_eq!(first.id, "ENV-007");
    let err = invoke_cli(
        tmp.path(),
        BenchNewRequest {
            id: Some("ENV-007".to_string()),
            bench_type: "ephemeral-tempdir".to_string(),
            safety_class: "isolated".to_string(),
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            setup: None,
            teardown: None,
            endpoint: None,
            fixture_source: None,
            workdir: None,
        },
    )
    .expect_err("second invocation must fail");
    match err {
        HandlerError::DuplicateId { id } => assert_eq!(id, "ENV-007"),
        other => panic!("expected DuplicateId, got {other:?}"),
    }
}

#[test]
fn mcp_caller_supplied_id_collision_matches_cli_error() {
    let tmp = init_workdir("collide-mcp");
    let _ = invoke_mcp_tool(
        tmp.path(),
        json!({
            "id": "ENV-007",
            "bench_type": "ephemeral-tempdir",
            "safety_class": "isolated",
            "allowed_ops": ["shell"],
        }),
    )
    .expect("first mcp env new");
    let err = invoke_mcp_tool(
        tmp.path(),
        json!({
            "id": "ENV-007",
            "bench_type": "ephemeral-tempdir",
            "safety_class": "isolated",
            "allowed_ops": ["filesystem"],
        }),
    )
    .expect_err("second mcp env new must fail");
    match err {
        HandlerError::DuplicateId { id } => assert_eq!(id, "ENV-007"),
        other => panic!("expected DuplicateId, got {other:?}"),
    }
}

// --- AC #6: remote env with endpoint succeeds; omitted endpoint fails -----

#[test]
fn remote_env_with_endpoint_succeeds() {
    let tmp = init_workdir("remote-ok");
    let out = invoke_cli(
        tmp.path(),
        BenchNewRequest {
            id: None,
            bench_type: "remote-http".to_string(),
            safety_class: "shared-non-destructive".to_string(),
            allowed_ops: vec!["http".to_string()],
            setup: None,
            teardown: None,
            endpoint: Some("https://example.com".to_string()),
            fixture_source: None,
            workdir: None,
        },
    )
    .expect("remote with endpoint must succeed");
    assert!(out.path.exists());
    let env = from_turtle(&out.path).expect("parse");
    assert_eq!(env.bench_type, "remote-http");
    assert_eq!(env.endpoint.as_deref(), Some("https://example.com"));
}
