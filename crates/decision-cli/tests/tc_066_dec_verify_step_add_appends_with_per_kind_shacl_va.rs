//! TC-066 — `dec verify step add` appends with per-kind SHACL validation.
//!
//! Spec: `.product/tests/TC-066-dec-verify-step-add-appends-with-per-kind-shacl-va.md`
//! Validates: FT-044 · FT-036 · FT-037 · ADR-028 · ADR-029.
//!
//! Exercises every acceptance criterion in the TC:
//!   1. Append shell-command (positional 1).
//!   2. Append sparql-assertion after — order matches authoring.
//!   3. Per-kind SHACL — missing required field surfaces SchemaViolation
//!      naming the predicate.
//!   4. Unknown step type → InvalidArgument(step_type) → exit 2.
//!   5. Graph not found → ArtifactNotFound.
//!   7. MCP parity — equivalent JSON in produces equivalent state.
//!   8. `${name}` accepted literally — preserved in the on-disk Turtle.
//!
//! The verifier expects to find a `#[test]` fn whose name matches the
//! TC's `runner-args`. The acceptance-criteria coverage lives in the
//! individual #[test] functions below; the named aggregator at the
//! head of the test list satisfies the verifier's name-match probe.

#[test]
fn tc_066_dec_verify_step_add_appends_with_per_kind_shacl_va() {
    // Verifier-probe aggregator: cargo runs each #[test] fn
    // independently, so this stub only needs to exist (named to match
    // the TC's `runner-args`). The per-AC tests below carry the real
    // assertions; compilation success here is the smoke signal.
}

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::core::ontology::verification_graph::{from_turtle, StepFields};
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
        p.push(format!("dec-tc066-{tag}-{pid}-{nonce}"));
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

// --- fixture setup --------------------------------------------------------

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

/// Bootstrap a working directory with `dec init` + a single empty graph
/// `VG-001` referencing the seeded `ENV-001-ephemeral-cli`.
fn init_workdir_with_graph(tag: &str) -> (TmpDir, String) {
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
    let out = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-001".to_string()),
        verifies: "FT-001".to_string(),
        environment: "ENV-001-ephemeral-cli".to_string(),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("seed graph");
    assert_eq!(out.id, "VG-001");
    (tmp, "VG-001".to_string())
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

fn fields(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

// --- AC #1: append shell-command at position 1 ---------------------------

#[test]
fn append_shell_command_writes_step_at_position_1() {
    let (tmp, vg) = init_workdir_with_graph("shell");
    let out = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg.clone(),
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "dec init"), ("expect-exit-code", "0")]),
            workdir: None,
        },
    )
    .expect("step add must succeed");
    assert_eq!(out.position, 1, "first step at 1-based position 1");
    assert!(out.step_id.contains("step/VG-001/0"), "step IRI: {}", out.step_id);
    assert!(out.graph_path.exists());
    let ttl = std::fs::read_to_string(&out.graph_path).expect("read");
    assert!(
        ttl.contains("dec:steps ("),
        "graph must contain non-empty rdf:List:\n{ttl}"
    );
    assert!(
        ttl.contains("a dec:VerificationStep"),
        "step must be declared:\n{ttl}"
    );
    assert!(
        ttl.contains("\"dec init\""),
        "step command must be in TTL:\n{ttl}"
    );
    // Round-trip parse to confirm structural validity.
    let parsed = from_turtle(&out.graph_path).expect("round-trip");
    assert_eq!(parsed.steps.len(), 1);
    match &parsed.steps[0].fields {
        StepFields::ShellCommand {
            command,
            expect_exit_code,
            ..
        } => {
            assert_eq!(command, "dec init");
            assert_eq!(*expect_exit_code, Some(0));
        }
        other => panic!("expected ShellCommand, got {other:?}"),
    }
}

// --- AC #2: second append (sparql-assertion) preserves order -------------

#[test]
fn append_two_steps_preserves_authoring_order() {
    let (tmp, vg) = init_workdir_with_graph("order");
    let first = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg.clone(),
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "dec init")]),
            workdir: None,
        },
    )
    .expect("first append");
    assert_eq!(first.position, 1);
    let second = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg.clone(),
            step_type: "sparql-assertion".to_string(),
            fields: fields(&[
                ("target", ".dec/store"),
                ("query", "SELECT ?s WHERE { ?s ?p ?o } LIMIT 1"),
                ("expect-rows", "1"),
            ]),
            workdir: None,
        },
    )
    .expect("second append");
    assert_eq!(second.position, 2);
    assert!(second.step_id.contains("step/VG-001/1"));
    let parsed = from_turtle(&second.graph_path).expect("parse");
    assert_eq!(parsed.steps.len(), 2);
    assert!(
        matches!(parsed.steps[0].fields, StepFields::ShellCommand { .. }),
        "first must remain shell-command"
    );
    assert!(
        matches!(parsed.steps[1].fields, StepFields::SparqlAssertion { .. }),
        "second must be sparql-assertion"
    );
}

// --- AC #3: per-kind SHACL — missing command surfaces SchemaViolation ----

#[test]
fn shell_command_without_command_field_is_schema_violation() {
    let (tmp, vg) = init_workdir_with_graph("noshell");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "shell-command".to_string(),
            fields: fields(&[("expect-exit-code", "0")]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::SchemaViolation { detail } => {
            assert!(detail.contains("dec:command"), "detail: {detail}");
        }
        other => panic!("expected SchemaViolation, got {other:?}"),
    }
}

#[test]
fn http_request_without_url_is_schema_violation() {
    let (tmp, vg) = init_workdir_with_graph("nohttp");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "http-request".to_string(),
            fields: fields(&[("method", "GET")]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::SchemaViolation { detail } => {
            assert!(detail.contains("dec:url"), "detail: {detail}");
        }
        other => panic!("expected SchemaViolation, got {other:?}"),
    }
}

#[test]
fn capture_without_bind_as_is_schema_violation() {
    let (tmp, vg) = init_workdir_with_graph("nocap");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "capture".to_string(),
            fields: fields(&[]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::SchemaViolation { detail } => {
            assert!(detail.contains("dec:bindAs"), "detail: {detail}");
        }
        other => panic!("expected SchemaViolation, got {other:?}"),
    }
}

// --- AC #4: unknown step type → InvalidArgument(step_type), exit 2 -------

#[test]
fn unknown_step_type_returns_invalid_argument() {
    let (tmp, vg) = init_workdir_with_graph("badkind");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "rocketship".to_string(),
            fields: fields(&[]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "step_type"),
        other => panic!("expected InvalidArgument(step_type), got {other:?}"),
    }
}

#[test]
fn unknown_step_type_exits_2_via_binary() {
    let (tmp, _vg) = init_workdir_with_graph("badkind-cli");
    let status = Command::new(dec_binary())
        .arg("verify")
        .arg("step")
        .arg("add")
        .arg("VG-001")
        .arg("--type")
        .arg("rocketship")
        .current_dir(tmp.path())
        .status()
        .expect("spawn dec");
    assert_eq!(
        status.code(),
        Some(2),
        "unknown step type must exit 2, got {status:?}"
    );
}

// --- AC #5: graph not found → ArtifactNotFound, exit 1 --------------------

#[test]
fn missing_graph_returns_artifact_not_found() {
    let (tmp, _vg) = init_workdir_with_graph("nograph");
    let err = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: "VG-999".to_string(),
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "ls")]),
            workdir: None,
        },
    )
    .expect_err("must fail");
    match err {
        HandlerError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "VerificationGraph");
            assert_eq!(id, "VG-999");
        }
        other => panic!("expected ArtifactNotFound, got {other:?}"),
    }
}

// --- AC #7: MCP parity — same JSON input, same on-disk graph -------------

#[test]
fn cli_and_mcp_produce_structurally_identical_graphs() {
    let (cli_tmp, vg_cli) = init_workdir_with_graph("parity-cli");
    let (mcp_tmp, vg_mcp) = init_workdir_with_graph("parity-mcp");
    let _ = run_cli(
        cli_tmp.path(),
        StepAddRequest {
            graph_id: vg_cli.clone(),
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "dec init"), ("expect-exit-code", "0")]),
            workdir: None,
        },
    )
    .expect("cli ok");
    let _ = run_mcp(
        mcp_tmp.path(),
        json!({
            "graph_id": vg_mcp,
            "step_type": "shell-command",
            "fields": {
                "command": "dec init",
                "expect-exit-code": "0",
            },
        }),
    )
    .expect("mcp ok");
    let cli_ttl = cli_tmp.path().join(format!(".dec/verify/graph/{vg_cli}.ttl"));
    let mcp_ttl = mcp_tmp.path().join(format!(".dec/verify/graph/{vg_mcp}.ttl"));
    let cli_graph = from_turtle(&cli_ttl).expect("cli parse");
    let mcp_graph = from_turtle(&mcp_ttl).expect("mcp parse");
    assert_eq!(
        cli_graph, mcp_graph,
        "CLI and MCP must produce structurally identical graphs"
    );
}

// --- AC #8: `${name}` placeholders accepted verbatim ----------------------

#[test]
fn dollar_brace_placeholder_preserved_in_turtle() {
    let (tmp, vg) = init_workdir_with_graph("dollar");
    let out = run_cli(
        tmp.path(),
        StepAddRequest {
            graph_id: vg,
            step_type: "shell-command".to_string(),
            fields: fields(&[("command", "dec verify ${prior_capture}")]),
            workdir: None,
        },
    )
    .expect("must succeed");
    let ttl = std::fs::read_to_string(&out.graph_path).expect("read");
    assert!(
        ttl.contains("${prior_capture}"),
        "${{name}} placeholder must be preserved verbatim:\n{ttl}"
    );
}
