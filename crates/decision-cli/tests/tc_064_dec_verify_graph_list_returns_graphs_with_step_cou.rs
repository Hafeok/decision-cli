//! TC-064 — `dec verify graph list` returns graphs with step counts and
//!          respects filters (FT-042 / ADR-029 / ADR-028).
//!
//! Spec: `.product/tests/TC-064-dec-verify-graph-list-returns-graphs-with-step-cou.md`
//! Validates: FT-042 — CLI surface, MCP twin, single handler.
//!
//! The seven acceptance criteria map onto seven `#[test]` functions,
//! each sharing the same tempdir / `dec init` fixture and the same set
//! of authored graphs.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use decision_cli::core::handler::{Error as HandlerError, Request};
use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::core::StreamWriter;
use decision_cli::verify_graph_list::{
    self, GraphListRequest, GraphListResponse, GraphSummary, OutputFormat,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
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
        p.push(format!("dec-tc064-{tag}-{pid}-{nonce}"));
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

/// Author placeholder feature/TC files so `dec verify graph new` can
/// resolve `FT-001` and `TC-013` `verifies` references.
fn write_product_fixtures(dir: &Path) {
    let features = dir.join(".product/features");
    let tests = dir.join(".product/tests");
    std::fs::create_dir_all(&features).expect("features");
    std::fs::create_dir_all(&tests).expect("tests");
    let ft1 = "---\nid: FT-001\ntitle: fixture\nphase: 1\nstatus: planned\n---\n\nFixture.\n";
    std::fs::write(features.join("FT-001-fixture.md"), ft1).expect("FT-001 fixture");
    let ft2 = "---\nid: FT-002\ntitle: fixture\nphase: 1\nstatus: planned\n---\n\nFixture.\n";
    std::fs::write(features.join("FT-002-fixture.md"), ft2).expect("FT-002 fixture");
    let tc = "---\nid: TC-013\ntitle: fixture\nphase: 1\nstatus: passing\n---\n\nFixture.\n";
    std::fs::write(tests.join("TC-013-fixture.md"), tc).expect("TC-013 fixture");
}

fn dec_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dec"))
}

fn init_workdir(tag: &str) -> TmpDir {
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
    tmp
}

/// Author a graph with the given verifies/env via the production handler.
/// All authored graphs are empty by default; use [`author_graph_with_steps`]
/// when steps are needed (AC #3).
fn author_empty_graph(workdir: &Path, id: &str, verifies: &str, environment: &str) {
    let req = GraphNewRequest {
        id: Some(id.to_string()),
        verifies: verifies.to_string(),
        environment: environment.to_string(),
        workdir: Some(workdir.to_path_buf()),
    };
    verify_graph_new::run(&req)
        .unwrap_or_else(|e| panic!("author empty graph {id}: {e}"));
}

/// Author a graph with `n` shell-command steps. Writes directly through
/// `StreamWriter` because slice 2.5 has no `dec verify step add` yet
/// (FT-043 ships that). The step list passes the FT-037 safety check
/// against the seed `ENV-001-ephemeral-cli` env (`shell` + `filesystem`).
fn author_graph_with_steps(workdir: &Path, id: &str, verifies_iri: &str, env_iri: &str, n: usize) {
    let steps: Vec<VerificationStep> = (0..n)
        .map(|i| {
            VerificationStep::new(
                id,
                i,
                StepFields::ShellCommand {
                    command: format!("true # step {i}"),
                    expect_exit_code: Some(0),
                    capture_output: None,
                },
            )
        })
        .collect();
    let graph = VerificationGraph::new(
        id,
        ArtifactRef(NamedNode::new_unchecked(verifies_iri)),
        NamedNode::new_unchecked(env_iri),
        steps,
    );
    write_graph_through_writer(workdir, &graph);
    // Also write the canonical Turtle so `.dec/verify/graph/<id>.ttl`
    // mirrors the projection (mint-via-FT-041 also does this).
    let dir = workdir.join(".dec/verify/graph");
    std::fs::create_dir_all(&dir).expect("graph dir");
    let ttl = decision_cli::core::ontology::verification_graph::to_canonical_turtle(&graph);
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

fn run_list(workdir: &Path, req: GraphListRequest) -> Vec<GraphSummary> {
    let mut req = req;
    if req.workdir.is_none() {
        req.workdir = Some(workdir.to_path_buf());
    }
    let resp = verify_graph_list::run(&req).expect("graph list ok");
    resp.graphs
}

fn mcp_invoke(workdir: &Path, args: Value) -> Result<GraphListResponse, HandlerError> {
    let mut obj = args;
    obj.as_object_mut()
        .expect("args is object")
        .insert("workdir".to_string(), json!(workdir.to_string_lossy()));
    let req = Request::new(verify_graph_list::TOOL_NAME, obj);
    let parsed = verify_graph_list::parse_request(&req)?;
    verify_graph_list::run(&parsed)
}

// --- AC #1: empty store -------------------------------------------------

#[test]
fn ac1_empty_store_prints_advisory_returns_empty_array_and_mcp_empty() {
    let tmp = init_workdir("ac1");
    // No graphs authored. Default list returns empty.
    let graphs = run_list(tmp.path(), GraphListRequest::default());
    assert!(graphs.is_empty(), "no graphs should be present");

    // CLI table render carries the advisory.
    let resp = GraphListResponse { graphs };
    let table = verify_graph_list::render_table(&resp);
    assert!(
        table.contains("no verification graphs yet"),
        "table must carry empty-state advisory; got: {table}"
    );

    // CLI JSON render returns an empty array.
    let json = verify_graph_list::render_json(&resp);
    let v: Value = serde_json::from_str(&json).expect("json");
    assert!(v.is_array());
    assert_eq!(v.as_array().expect("arr").len(), 0);

    // MCP returns `{ "graphs": [] }`.
    let mcp_resp = mcp_invoke(tmp.path(), json!({})).expect("mcp ok");
    assert!(mcp_resp.graphs.is_empty());
}

// --- AC #2: ascending VG-NNN order regardless of authoring order -------

#[test]
fn ac2_graphs_returned_in_ascending_numeric_order() {
    let tmp = init_workdir("ac2");
    // Author out of order: VG-003 first, then VG-001, then VG-002.
    author_empty_graph(tmp.path(), "VG-003", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-002", "TC-013", "ENV-001-ephemeral-cli");
    let graphs = run_list(tmp.path(), GraphListRequest::default());
    let ids: Vec<&str> = graphs.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, vec!["VG-001", "VG-002", "VG-003"]);
}

// --- AC #3: step count computed server-side ----------------------------

#[test]
fn ac3_step_count_three_and_zero() {
    let tmp = init_workdir("ac3");
    // Empty graph minted via the new handler.
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    // 3-step graph minted directly through StreamWriter.
    author_graph_with_steps(
        tmp.path(),
        "VG-002",
        "https://decision-cli.dev/ns/feature/FT-001",
        "https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli",
        3,
    );
    let graphs = run_list(tmp.path(), GraphListRequest::default());
    assert_eq!(graphs.len(), 2);
    let vg1 = graphs.iter().find(|g| g.id == "VG-001").expect("VG-001");
    assert_eq!(vg1.step_count, 0, "empty graph has zero steps");
    let vg2 = graphs.iter().find(|g| g.id == "VG-002").expect("VG-002");
    assert_eq!(vg2.step_count, 3, "3-step graph reports step_count: 3");
}

// --- AC #4: filter by verifies (FT and TC) ------------------------------

#[test]
fn ac4_filter_by_verifies_feature_then_tc() {
    let tmp = init_workdir("ac4");
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-002", "TC-013", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-003", "FT-002", "ENV-001-ephemeral-cli");

    // FT-001 filter returns only VG-001.
    let req = GraphListRequest {
        verifies: Some("FT-001".to_string()),
        ..Default::default()
    };
    let graphs = run_list(tmp.path(), req);
    let ids: Vec<&str> = graphs.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, vec!["VG-001"]);
    assert_eq!(graphs[0].verifies, "FT-001");

    // TC-013 filter returns only VG-002.
    let req = GraphListRequest {
        verifies: Some("TC-013".to_string()),
        ..Default::default()
    };
    let graphs = run_list(tmp.path(), req);
    let ids: Vec<&str> = graphs.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, vec!["VG-002"]);
    assert_eq!(graphs[0].verifies, "TC-013");
}

// --- AC #5: filter by environment --------------------------------------

#[test]
fn ac5_filter_by_environment() {
    let tmp = init_workdir("ac5");
    // Author a second env so we have two to filter between.
    let env2_req = decision_cli::verify_env_new::EnvNewRequest {
        id: Some("ENV-002".to_string()),
        env_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        workdir: Some(tmp.path().to_path_buf()),
    };
    decision_cli::verify_env_new::run(&env2_req).expect("env new");

    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-002", "FT-001", "ENV-002");

    let req = GraphListRequest {
        environment: Some("ENV-001-ephemeral-cli".to_string()),
        ..Default::default()
    };
    let graphs = run_list(tmp.path(), req);
    let ids: Vec<&str> = graphs.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, vec!["VG-001"]);
    assert_eq!(graphs[0].environment, "ENV-001-ephemeral-cli");
}

// --- AC #6: combined filters apply conjunctively -----------------------

#[test]
fn ac6_combined_filters_are_conjunctive() {
    let tmp = init_workdir("ac6");
    let env2_req = decision_cli::verify_env_new::EnvNewRequest {
        id: Some("ENV-002".to_string()),
        env_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        workdir: Some(tmp.path().to_path_buf()),
    };
    decision_cli::verify_env_new::run(&env2_req).expect("env new");
    // Four graphs across two features × two envs.
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-002", "FT-001", "ENV-002");
    author_empty_graph(tmp.path(), "VG-003", "FT-002", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-004", "FT-002", "ENV-002");

    let req = GraphListRequest {
        verifies: Some("FT-001".to_string()),
        environment: Some("ENV-001-ephemeral-cli".to_string()),
        ..Default::default()
    };
    let graphs = run_list(tmp.path(), req);
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].id, "VG-001");
    assert_eq!(graphs[0].verifies, "FT-001");
    assert_eq!(graphs[0].environment, "ENV-001-ephemeral-cli");
}

// --- AC #7: MCP parity (CLI JSON ≡ MCP `graphs` array) ----------------

#[test]
fn ac7_mcp_parity_with_cli_json_output() {
    let tmp = init_workdir("ac7");
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    author_empty_graph(tmp.path(), "VG-002", "TC-013", "ENV-001-ephemeral-cli");
    author_graph_with_steps(
        tmp.path(),
        "VG-003",
        "https://decision-cli.dev/ns/feature/FT-001",
        "https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli",
        2,
    );

    let cli_graphs = run_list(tmp.path(), GraphListRequest::default());
    let cli_resp = GraphListResponse {
        graphs: cli_graphs.clone(),
    };
    let cli_json: Value =
        serde_json::from_str(&verify_graph_list::render_json(&cli_resp)).expect("cli json");

    let mcp_resp = mcp_invoke(tmp.path(), json!({})).expect("mcp ok");
    assert_eq!(mcp_resp.graphs.len(), cli_graphs.len());
    let mcp_arr = serde_json::to_value(&mcp_resp.graphs).expect("ser");
    assert_eq!(
        cli_json, mcp_arr,
        "CLI JSON ≡ MCP `graphs` array element-for-element"
    );
}

// --- Extra: invalid filter rejected with InvalidArgument --------------

#[test]
fn invalid_verifies_filter_returns_invalid_argument() {
    let tmp = init_workdir("invalid");
    let req = GraphListRequest {
        verifies: Some("garbage".to_string()),
        workdir: Some(tmp.path().to_path_buf()),
        ..Default::default()
    };
    let err = verify_graph_list::run(&req).expect_err("garbage must fail");
    match err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "verifies"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn invalid_environment_filter_returns_invalid_argument() {
    let tmp = init_workdir("invalid-env");
    let req = GraphListRequest {
        environment: Some("nope".to_string()),
        workdir: Some(tmp.path().to_path_buf()),
        ..Default::default()
    };
    let err = verify_graph_list::run(&req).expect_err("nope must fail");
    match err {
        HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "environment"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

// --- Extra: invalid format on the CLI parse helper --------------------

#[test]
fn invalid_format_value_returns_none() {
    assert!(OutputFormat::parse("yaml").is_none());
    assert_eq!(OutputFormat::parse("table"), Some(OutputFormat::Table));
    assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
}

// --- Extra: round-trip CLI invocation via the binary -----------------

#[test]
fn binary_cli_table_format_smoke() {
    let tmp = init_workdir("binary-smoke");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("list")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify graph list");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains("no verification graphs yet"),
        "empty store should print advisory; got: {stdout}"
    );
}

#[test]
fn binary_cli_json_format_outputs_array() {
    let tmp = init_workdir("binary-json");
    author_empty_graph(tmp.path(), "VG-001", "FT-001", "ENV-001-ephemeral-cli");
    let output = Command::new(dec_binary())
        .arg("verify")
        .arg("graph")
        .arg("list")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .output()
        .expect("spawn dec verify graph list --format json");
    assert!(output.status.success(), "non-zero exit: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert!(v.is_array());
    let arr = v.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "VG-001");
    assert_eq!(arr[0]["verifies"], "FT-001");
    assert_eq!(arr[0]["step_count"], 0);
}
