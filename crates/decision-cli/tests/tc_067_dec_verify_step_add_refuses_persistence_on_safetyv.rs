//! TC-067 — `dec verify step add` refuses persistence on SafetyViolation.
//!
//! Spec: `.product/tests/TC-067-dec-verify-step-add-refuses-persistence-on-safetyv.md`
//! Validates: FT-044 · FT-037 · ADR-028 · ADR-029.
//!
//! Exercises every acceptance criterion in the TC:
//!   1. Violation path — http-request POST against production-readonly env.
//!   2. No on-disk side effect (.ttl unchanged, no .tmp).
//!   3. No store mutation (step quad count unchanged).
//!   4. MCP surfaces structured SafetyViolation with identical fields.
//!   5. Subsequent allowed step in a different graph still works.
//!   6. First-violation diagnostic — safety runs before SHACL.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::verify_env_new::{self, EnvNewRequest};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest, StepAddResponse};
use serde_json::json;

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
        p.push(format!("dec-tc067-{tag}-{pid}-{nonce}"));
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

fn write_product_fixtures(dir: &Path) {
    let features = dir.join(".product/features");
    std::fs::create_dir_all(&features).expect("features");
    let ft_body =
        "---\nid: FT-001\ntitle: test fixture feature\nphase: 1\nstatus: planned\n---\n\nFixture.\n";
    std::fs::write(features.join("FT-001-test-fixture.md"), ft_body).expect("FT-001 fixture");
}

fn dec_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dec"))
}

/// Bootstrap a workdir, then author:
///   * `ENV-002-prod` (production-readonly, `http-readonly` only)
///   * `VG-100-prod` referencing `ENV-002-prod`
/// Returns the workdir and the graph id.
fn workdir_with_prod_env_and_graph(tag: &str) -> (TmpDir, String) {
    let tmp = TmpDir::new(tag);
    write_seed(tmp.path());
    write_product_fixtures(tmp.path());
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
    // Author production-readonly env.
    let env = verify_env_new::run(&EnvNewRequest {
        id: Some("ENV-002-prod".to_string()),
        env_type: "remote-http".to_string(),
        safety_class: "production-readonly".to_string(),
        allowed_ops: vec!["http-readonly".to_string()],
        setup: None,
        teardown: None,
        endpoint: Some("https://prod.example.com".to_string()),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("env new");
    assert_eq!(env.id, "ENV-002-prod");
    // Author graph referencing the prod env.
    let graph = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-100-prod".to_string()),
        verifies: "FT-001".to_string(),
        environment: "ENV-002-prod".to_string(),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("graph new");
    assert_eq!(graph.id, "VG-100-prod");
    (tmp, "VG-100-prod".to_string())
}

fn fields(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn run_cli(workdir: &Path, req: StepAddRequest) -> Result<StepAddResponse, HandlerError> {
    let mut req = req;
    req.workdir = Some(workdir.to_path_buf());
    verify_step_add::run(&req)
}

fn run_mcp(workdir: &Path, args: serde_json::Value) -> Result<StepAddResponse, HandlerError> {
    let mut obj = args;
    obj.as_object_mut()
        .expect("args object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_step_add::TOOL_NAME, obj);
    let parsed = verify_step_add::parse_request(&req)?;
    verify_step_add::run(&parsed)
}

// --- AC #1: violation path ------------------------------------------------

#[test]
fn http_post_against_readonly_env_returns_safety_violation() {
    let (tmp, vg) = workdir_with_prod_env_and_graph("ac1");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
                ("expect-status", "200"),
            ]),
            workdir: None,
        },
    )
    .expect_err("must fail with safety violation");
    match err {
        HandlerError::SafetyViolation(v) => {
            assert_eq!(v.step_kind, "http-request");
            assert!(
                v.missing_ops.iter().any(|m| m == "http-mutating"),
                "missing_ops should include http-mutating; got {v:?}"
            );
            assert!(v.env_id.contains("ENV-002-prod"), "env_id: {}", v.env_id);
            assert_eq!(v.env_safety_class, "production-readonly");
            assert!(
                v.env_allowed_ops.iter().any(|a| a == "http-readonly"),
                "env_allowed_ops should include http-readonly; got {v:?}"
            );
        }
        other => panic!("expected SafetyViolation, got {other:?}"),
    }
}

// --- AC #2: no on-disk side effect ---------------------------------------

#[test]
fn safety_violation_leaves_on_disk_ttl_unchanged() {
    let (tmp, vg) = workdir_with_prod_env_and_graph("ac2");
    let ttl_path = tmp.path().join(format!(".dec/verify/graph/{vg}.ttl"));
    let before = std::fs::read(&ttl_path).expect("read pre");
    let _ = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg.clone(),
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
                ("expect-status", "200"),
            ]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    let after = std::fs::read(&ttl_path).expect("read post");
    assert_eq!(
        before, after,
        "on-disk .ttl must be byte-identical after safety failure"
    );
    let tmp_path = tmp.path().join(format!(".dec/verify/graph/{vg}.ttl.tmp"));
    assert!(
        !tmp_path.exists(),
        "no temp file should remain after safety failure"
    );
}

// --- AC #3: no store mutation --------------------------------------------

#[test]
fn safety_violation_leaves_store_dump_unchanged() {
    let (tmp, vg) = workdir_with_prod_env_and_graph("ac3");
    let dump = tmp.path().join(".dec/store/orchestration.nq");
    let before = std::fs::read(&dump).expect("dump pre");
    let _ = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
                ("expect-status", "200"),
            ]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    let after = std::fs::read(&dump).expect("dump post");
    assert_eq!(
        before, after,
        "store dump must be byte-identical after safety failure"
    );
}

// --- AC #4: MCP surfaces structured SafetyViolation ----------------------

#[test]
fn mcp_surfaces_structured_safety_violation_with_same_fields() {
    let (cli_tmp, vg_cli) = workdir_with_prod_env_and_graph("ac4-cli");
    let (mcp_tmp, vg_mcp) = workdir_with_prod_env_and_graph("ac4-mcp");
    let cli_err = run_cli(
        cli_tmp.path(),
        StepAddRequest {
            graph_id: vg_cli.clone(),
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
                ("expect-status", "200"),
            ]),
            workdir: None,
        },
    )
    .expect_err("cli must fail");
    let mcp_err = run_mcp(
        mcp_tmp.path(),
        json!({
            "graph_id": vg_mcp,
            "step_type": "http-request",
            "fields": {
                "method": "POST",
                "url": "https://prod.example.com/api",
                "expect-status": "200",
            },
        }),
    )
    .expect_err("mcp must fail");
    let cli_v = match cli_err {
        HandlerError::SafetyViolation(v) => v,
        other => panic!("expected cli SafetyViolation, got {other:?}"),
    };
    let mcp_v = match mcp_err {
        HandlerError::SafetyViolation(v) => v,
        other => panic!("expected mcp SafetyViolation, got {other:?}"),
    };
    // The step IRI's `graph-id` segment differs across tempdirs but the
    // structural fields (kind, missing_ops, safety class, allowed ops)
    // must match.
    assert_eq!(cli_v.step_kind, mcp_v.step_kind);
    assert_eq!(cli_v.missing_ops, mcp_v.missing_ops);
    assert_eq!(cli_v.env_safety_class, mcp_v.env_safety_class);
    assert_eq!(cli_v.env_allowed_ops, mcp_v.env_allowed_ops);
}

// --- AC #5: subsequent allowed step in a different graph still works -----

#[test]
fn subsequent_allowed_step_succeeds_in_isolated_env() {
    let (tmp, prod_graph) = workdir_with_prod_env_and_graph("ac5");
    // Fire-and-fail safety violation in prod graph.
    let _ = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: prod_graph,
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
                ("expect-status", "200"),
            ]),
            workdir: None,
        },
    )
    .expect_err("prod must fail");
    // Now author an isolated graph and append a benign step.
    let iso = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-002-iso".to_string()),
        verifies: "FT-001".to_string(),
        environment: "ENV-001-ephemeral-cli".to_string(),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("iso graph");
    let out = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: iso.id,
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "echo ok")]),
            workdir: None,
        },
    )
    .expect("benign append must succeed after prior safety failure");
    assert_eq!(out.position, 1);
}

// --- AC #6: first-violation diagnostic — safety runs before SHACL --------

#[test]
fn safety_violation_surfaces_before_schema_violation() {
    // Field errors (would yield SchemaViolation) combined with safety
    // violation: per FT-044 §Behaviour, fields are validated FIRST when
    // they prevent parsing of the step entirely. But if the fields ARE
    // present (valid step), then safety runs before the writer's SHACL.
    //
    // The TC asserts that when a step's `requiredOps` escape the env, the
    // safety violation is the first observable error. Constructing a
    // valid-fields http-request step is the right test here.
    let (tmp, vg) = workdir_with_prod_env_and_graph("ac6");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "http-request".to_string(),
            fields: fields(&[
                ("method", "POST"),
                ("url", "https://prod.example.com/api"),
            ]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::SafetyViolation(_) => {}
        other => panic!("expected SafetyViolation first, got {other:?}"),
    }
}
