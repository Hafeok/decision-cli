//! TC-065 — `dec verify graph show` renders header and ordered step list
//!          (FT-043 / ADR-029 / ADR-028).
//!
//! Spec: `.product/tests/TC-065-dec-verify-graph-show-renders-header-and-ordered-s.md`
//! Validates: FT-043 — CLI surface, MCP twin, single handler.
//!
//! The eight acceptance criteria map onto `#[test]` functions that share
//! a tempdir / `dec init` fixture and a single multi-kind graph
//! authored directly through `StreamWriter` (slice 2.5 has no
//! `dec verify step add` yet — FT-044 ships that).

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::core::ontology::verification_graph::{
    to_canonical_turtle, ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::core::StreamWriter;
use decision_cli::verify_graph_show::{
    self, document_to_graph, GraphDocument, GraphShowRequest, GraphShowResponse, OutputFormat,
};
use decision_cli::vocab::verify_graph_named_graph;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
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
        p.push(format!("dec-tc065-{tag}-{pid}-{nonce}"));
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

/// Author the canonical TC-065 fixture: a single graph carrying four
/// step kinds in this order:
///
///   1. `shell-command` whose `dec:command` contains a `${name}` placeholder
///      (covers AC #8 placeholder preservation in shell-command).
///   2. `sparql-assertion`
///   3. `file-assertion`
///   4. `capture`
///
/// Steps 1..=3 also satisfy AC #2's ordering assertion.
fn author_fixture_graph(workdir: &Path, id: &str) {
    let env_iri = "https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli";
    let verifies_iri = "https://decision-cli.dev/ns/feature/FT-001";
    let steps = vec![
        VerificationStep::new(
            id,
            0,
            StepFields::ShellCommand {
                command: "echo ${earlier_capture} && true".to_string(),
                expect_exit_code: Some(0),
                capture_output: Some(true),
            },
        ),
        VerificationStep::new(
            id,
            1,
            StepFields::SparqlAssertion {
                target: ".dec/store".to_string(),
                query: "SELECT ?s WHERE { ?s a <urn:ex:T> }".to_string(),
                expect_rows: Some(1),
            },
        ),
        VerificationStep::new(
            id,
            2,
            StepFields::FileAssertion {
                path: ".dec/store/orchestration.nq".to_string(),
                expect_hash: None,
                expect_content: None,
            },
        ),
        VerificationStep::new(
            id,
            3,
            StepFields::Capture {
                from_step: None,
                bind_as: "manifest_sha".to_string(),
            },
        ),
    ];
    let graph = VerificationGraph::new(
        id,
        ArtifactRef(NamedNode::new_unchecked(verifies_iri)),
        NamedNode::new_unchecked(env_iri),
        steps,
    );
    write_graph_through_writer(workdir, &graph);
    // Mirror to disk — `dec verify graph new` does this for empty graphs;
    // we author manually for the multi-step fixture.
    let dir = workdir.join(".dec/verify/graph");
    std::fs::create_dir_all(&dir).expect("graph dir");
    let ttl = to_canonical_turtle(&graph);
    std::fs::write(dir.join(format!("{id}.ttl")), ttl).expect("write ttl");
}

fn write_graph_through_writer(workdir: &Path, graph: &VerificationGraph) {
    let scope = ActiveScope::load(workdir).expect("active scope");
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path).expect("load store");
    let store = Arc::new(store);
    let stream_iri = NamedNode::new(&scope.stream_iri).expect("stream iri");
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).expect("open writer");
    let quads = graph.to_quads(verify_graph_named_graph());
    writer
        .commit(Mutation::insert(quads))
        .unwrap_or_else(|e| panic!("commit graph: {e:#}"));
    persist_store(&store, &dump_path).expect("persist store");
}

fn run_show(workdir: &Path, req: GraphShowRequest) -> Result<GraphShowResponse, HandlerError> {
    let mut req = req;
    if req.workdir.is_none() {
        req.workdir = Some(workdir.to_path_buf());
    }
    verify_graph_show::run(&req)
}

fn mcp_invoke(workdir: &Path, args: Value) -> Result<GraphShowResponse, HandlerError> {
    let mut obj = args;
    obj.as_object_mut()
        .expect("args is object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_graph_show::TOOL_NAME, obj);
    let parsed = verify_graph_show::parse_request(&req)?;
    verify_graph_show::run(&parsed)
}

// --- AC #1: header text contains id, verifies, env-with-safety, Steps ---

#[test]
fn ac1_text_header_renders_in_documented_order() {
    let tmp = init_workdir("ac1");
    author_fixture_graph(tmp.path(), "VG-001");
    let req = GraphShowRequest {
        id: "VG-001".to_string(),
        format: Some(OutputFormat::Text),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show ok");
    let text = verify_graph_show::render_text(&resp);
    let id_at = text.find("VG-001").expect("id present");
    let verifies_at = text.find("Verifies:").expect("verifies present");
    let env_at = text.find("Environment:").expect("env present");
    let steps_at = text.find("Steps:").expect("steps header present");
    assert!(id_at < verifies_at, "id before Verifies");
    assert!(verifies_at < env_at, "Verifies before Environment");
    assert!(env_at < steps_at, "Environment before Steps:");
    // Environment line carries the safety class.
    assert!(
        text.contains("safety: isolated"),
        "expected safety class: {text}"
    );
    // Path footer at the end.
    assert!(text.contains("Path:"), "expected Path footer: {text}");
}

// --- AC #2: step order matches storage (byte-stable) ----------------------

#[test]
fn ac2_step_order_matches_storage_and_is_byte_stable() {
    let tmp = init_workdir("ac2");
    author_fixture_graph(tmp.path(), "VG-001");
    let req = GraphShowRequest {
        id: "VG-001".to_string(),
        format: Some(OutputFormat::Text),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp_a = run_show(tmp.path(), req.clone()).expect("show A");
    let resp_b = run_show(tmp.path(), req).expect("show B");
    let text_a = verify_graph_show::render_text(&resp_a);
    let text_b = verify_graph_show::render_text(&resp_b);
    assert_eq!(text_a, text_b, "successive renders must be byte-equal");
    // The three positions appear in order: shell, sparql, file.
    let p1 = text_a.find("1. shell-command").expect("pos 1 missing");
    let p2 = text_a.find("2. sparql-assertion").expect("pos 2 missing");
    let p3 = text_a.find("3. file-assertion").expect("pos 3 missing");
    let p4 = text_a.find("4. capture").expect("pos 4 missing");
    assert!(p1 < p2, "1 before 2: {text_a}");
    assert!(p2 < p3, "2 before 3: {text_a}");
    assert!(p3 < p4, "3 before 4: {text_a}");
}

// --- AC #3: step row shows position, kind, and key-field summary ----------

#[test]
fn ac3_step_rows_show_position_kind_and_summary() {
    let tmp = init_workdir("ac3");
    author_fixture_graph(tmp.path(), "VG-001");
    let req = GraphShowRequest {
        id: "VG-001".to_string(),
        format: Some(OutputFormat::Text),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show ok");
    let text = verify_graph_show::render_text(&resp);
    // shell-command exposes command="..." in the summary line.
    assert!(
        text.lines()
            .any(|l| l.contains("1. shell-command") && l.contains("command=")),
        "shell-command summary missing: {text}"
    );
    // sparql-assertion exposes query="..." in the summary.
    assert!(
        text.lines()
            .any(|l| l.contains("2. sparql-assertion") && l.contains("query=")),
        "sparql-assertion summary missing: {text}"
    );
    // file-assertion exposes path="..." in the summary.
    assert!(
        text.lines()
            .any(|l| l.contains("3. file-assertion") && l.contains("path=")),
        "file-assertion summary missing: {text}"
    );
}

// --- AC #4: JSON format emits full graph document with ordered steps ------

#[test]
fn ac4_json_format_emits_graph_document_with_ordered_steps() {
    let tmp = init_workdir("ac4");
    author_fixture_graph(tmp.path(), "VG-001");
    let req = GraphShowRequest {
        id: "VG-001".to_string(),
        format: Some(OutputFormat::Json),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show ok");
    let s = verify_graph_show::render_json(&resp);
    let v: Value = serde_json::from_str(&s).expect("json");
    assert!(v.is_object(), "expected object, got {s}");
    assert_eq!(v["id"], "VG-001");
    assert_eq!(v["verifies"], "FT-001");
    assert_eq!(v["environment"], "BNCH-001-ephemeral-cli");
    let steps = v["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 4, "expected four steps, got {steps:?}");
    assert_eq!(steps[0]["kind"], "shell-command");
    assert_eq!(steps[1]["kind"], "sparql-assertion");
    assert_eq!(steps[2]["kind"], "file-assertion");
    assert_eq!(steps[3]["kind"], "capture");
    // Full step documents include every field — shell-command shows
    // expect_exit_code and capture_output, not just `command`.
    assert!(steps[0].get("expect_exit_code").is_some());
    assert!(steps[0].get("capture_output").is_some());
}

// --- AC #5: round-trip JSON → Turtle equals on-disk file ------------------

#[test]
fn ac5_json_round_trips_to_canonical_turtle() {
    let tmp = init_workdir("ac5");
    author_fixture_graph(tmp.path(), "VG-001");
    let req = GraphShowRequest {
        id: "VG-001".to_string(),
        format: Some(OutputFormat::Json),
        workdir: Some(tmp.path().to_path_buf()),
    };
    let resp = run_show(tmp.path(), req).expect("show ok");
    let s = verify_graph_show::render_json(&resp);
    let doc: GraphDocument = serde_json::from_str(&s).expect("parse json");
    let graph = document_to_graph(&doc).expect("reconstruct");
    let rendered = to_canonical_turtle(&graph);
    let on_disk = std::fs::read_to_string(&resp.path).expect("read on-disk file");
    assert_eq!(rendered, on_disk, "round-trip Turtle must equal on-disk");
}

// --- AC #6: MCP parity with CLI JSON output -------------------------------

#[test]
fn ac6_mcp_parity_with_cli_json_output() {
    let tmp = init_workdir("ac6");
    author_fixture_graph(tmp.path(), "VG-001");
    let cli = run_show(
        tmp.path(),
        GraphShowRequest {
            id: "VG-001".to_string(),
            format: Some(OutputFormat::Json),
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect("cli ok");
    let cli_json: Value =
        serde_json::from_str(&verify_graph_show::render_json(&cli)).expect("cli json");
    let mcp = mcp_invoke(
        tmp.path(),
        json!({
            "id": "VG-001",
            "format": "json",
        }),
    )
    .expect("mcp ok");
    let mcp_json = serde_json::to_value(&mcp.graph).expect("ser mcp graph");
    assert_eq!(cli_json, mcp_json, "CLI JSON must equal MCP graph object");
    assert_eq!(cli.graph, mcp.graph);
}

// --- AC #7: unknown id surfaces ArtifactNotFound, exit 1 ------------------

#[test]
fn ac7_unknown_id_returns_artifact_not_found() {
    let tmp = init_workdir("ac7");
    author_fixture_graph(tmp.path(), "VG-001");
    let err = run_show(
        tmp.path(),
        GraphShowRequest {
            id: "VG-999".to_string(),
            format: None,
            workdir: Some(tmp.path().to_path_buf()),
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
    // MCP surface returns the same structured error.
    let mcp_err = mcp_invoke(tmp.path(), json!({"id": "VG-999"})).expect_err("mcp must also fail");
    match mcp_err {
        HandlerError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "VerificationGraph");
            assert_eq!(id, "VG-999");
        }
        other => panic!("expected ArtifactNotFound, got {other:?}"),
    }
    // Binary surface: unknown id exits 1, stderr names kind and id.
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("show")
        .arg("VG-999")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify graph show VG-999");
    assert_eq!(
        output.status.code(),
        Some(1),
        "unknown id must exit 1, got {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VerificationGraph"),
        "stderr must name kind: {stderr}"
    );
    assert!(stderr.contains("VG-999"), "stderr must name id: {stderr}");
}

// --- AC #8: ${name} preserved verbatim in both formats --------------------

#[test]
fn ac8_dollar_placeholder_preserved_in_text_and_json() {
    let tmp = init_workdir("ac8");
    author_fixture_graph(tmp.path(), "VG-001");
    // Text: the `${earlier_capture}` substring survives in the rendered output.
    let resp_text = run_show(
        tmp.path(),
        GraphShowRequest {
            id: "VG-001".to_string(),
            format: Some(OutputFormat::Text),
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect("text show ok");
    let text = verify_graph_show::render_text(&resp_text);
    assert!(
        text.contains("${earlier_capture}"),
        "text must preserve placeholder: {text}"
    );
    // JSON: same substring survives, indicating no resolution happened.
    let resp_json = run_show(
        tmp.path(),
        GraphShowRequest {
            id: "VG-001".to_string(),
            format: Some(OutputFormat::Json),
            workdir: Some(tmp.path().to_path_buf()),
        },
    )
    .expect("json show ok");
    let json = verify_graph_show::render_json(&resp_json);
    let v: Value = serde_json::from_str(&json).expect("json");
    let cmd = v["steps"][0]["command"].as_str().expect("command string");
    assert!(
        cmd.contains("${earlier_capture}"),
        "json must preserve placeholder: {cmd}"
    );
}

// --- Extras: malformed id exits 2 -----------------------------------------

#[test]
fn malformed_id_exits_with_invalid_argument() {
    let tmp = init_workdir("fmt-bad-id");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("show")
        .arg("not-an-id")
        .current_dir(tmp.path())
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed id must exit 2, got {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("id"), "stderr must name id: {stderr}");
}

#[test]
fn malformed_format_exits_with_invalid_argument() {
    let tmp = init_workdir("fmt-bad-format");
    author_fixture_graph(tmp.path(), "VG-001");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("show")
        .arg("VG-001")
        .arg("--format")
        .arg("yaml")
        .current_dir(tmp.path())
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed --format must exit 2, got {output:?}"
    );
}

// --- Binary CLI smoke ------------------------------------------------------

#[test]
fn binary_cli_text_format_smoke() {
    let tmp = init_workdir("binary-text");
    author_fixture_graph(tmp.path(), "VG-001");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("show")
        .arg("VG-001")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify graph show");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("VG-001"));
    assert!(stdout.contains("Steps:"));
    assert!(stdout.contains("Path:"));
}

#[test]
fn binary_cli_json_format_outputs_object() {
    let tmp = init_workdir("binary-json");
    author_fixture_graph(tmp.path(), "VG-001");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("show")
        .arg("VG-001")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify graph show --format json");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert!(v.is_object());
    assert_eq!(v["id"], "VG-001");
    assert!(v["steps"].is_array());
}
