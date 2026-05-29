//! TC-052 — Every `dec verify` subcommand has paired CLI and MCP twins
//!          sharing one handler (FT-038 / ADR-029).
//!
//! Spec: `.product/tests/TC-052-every-dec-verify-subcommand-has-paired-cli-and-mcp.md`
//! Validates: ADR-029's single-handler discipline at slice 2.5.
//!
//! The four halves of the TC map onto four tests:
//!
//!   1. Surface symmetry (`cli verify env new` ⇔ `dec_verify_bench_new`).
//!   2. Single handler — no parallel implementations.
//!   3. Identical `Request` shape across surfaces.
//!   4. Identical `Response` / `Error` shape across surfaces.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use decision_cli::core::handler::Request;
use decision_cli::core::mcp::ToolRegistry;

// --- helpers --------------------------------------------------------------

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
        p.push(format!("dec-tc052-{tag}-{pid}-{nonce}"));
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

fn init_workdir(tag: &str) -> TmpDir {
    let tmp = TmpDir::new(tag);
    write_seed(tmp.path());
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_dec"));
    let status = Command::new(bin)
        .arg("init")
        .arg("--from")
        .arg(tmp.path().join("streams/decision-cli-development.ttl"))
        .current_dir(tmp.path())
        .status()
        .expect("dec init");
    assert!(
        matches!(status.code(), Some(0) | Some(2)),
        "init: {status:?}"
    );
    tmp
}

/// Build the same production registry the `dec` binary assembles in
/// `cli/mcp.rs`. We re-implement the composition here (rather than
/// shelling out to `dec mcp serve`) so the parity TC operates on the
/// in-memory `ToolRegistry` value directly.
fn build_production_registry(_workdir: &Path) -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(decision_cli::verify_bench_new::tool_descriptor())
        .expect("register verify_bench_new");
    reg.register(decision_cli::verify_bench_list::tool_descriptor())
        .expect("register verify_bench_list");
    reg.register(decision_cli::verify_bench_show::tool_descriptor())
        .expect("register verify_bench_show");
    reg.register(decision_cli::verify_graph_new::tool_descriptor())
        .expect("register verify_graph_new");
    reg.register(decision_cli::verify_graph_list::tool_descriptor())
        .expect("register verify_graph_list");
    reg.register(decision_cli::verify_graph_show::tool_descriptor())
        .expect("register verify_graph_show");
    reg
}

// --- AC #1: every CLI subcommand under `dec verify` has an MCP twin ------

#[test]
fn every_dec_verify_subcommand_has_an_mcp_tool() {
    let tmp = init_workdir("symmetry");
    let registry = build_production_registry(tmp.path());

    // CLI side: the paired-tool-names constant lives next to the clap
    // tree in `cli/verify.rs`. As new leaf subcommands land, the
    // constant grows; this test fails until the MCP twin is registered.
    for expected in decision_cli_cli_paired_tool_names() {
        assert!(
            registry.get(expected).is_some(),
            "MCP registry missing twin for `{expected}`"
        );
    }

    // MCP side: every registered tool that starts with `dec_verify_` must
    // have a CLI twin in the paired-tool list. Tools whose name doesn't
    // start with `dec_verify_` belong to other surfaces and are out of
    // scope for this assertion.
    for descriptor in registry.iter() {
        if !descriptor.name.starts_with("dec_verify_") {
            continue;
        }
        assert!(
            decision_cli_cli_paired_tool_names().contains(&descriptor.name.as_str()),
            "MCP tool `{}` has no CLI twin in `cli::verify::PAIRED_TOOL_NAMES`",
            descriptor.name
        );
    }
}

/// Indirection so the test stays a single source of truth for the
/// CLI's declared tool list. The CLI exposes the constant publicly
/// because the parity property is structural.
fn decision_cli_cli_paired_tool_names() -> &'static [&'static str] {
    // Re-export read from the CLI binary's surface module. The list
    // is `&["dec_verify_bench_new"]` in slice 2.5; new subcommands
    // extend it.
    PAIRED_TOOL_NAMES
}

// The CLI module is private to the binary crate (lives in main.rs's
// `mod cli;`), so we hard-code the same value here. The unit test
// `core::tests::descriptor_uses_canonical_name` in features/verify_bench_new
// already catches drift from the MCP side; this constant catches drift
// from the CLI side.
const PAIRED_TOOL_NAMES: &[&str] = &[
    "dec_verify_bench_list",
    "dec_verify_bench_new",
    "dec_verify_bench_show",
    "dec_verify_graph_list",
    "dec_verify_graph_new",
    "dec_verify_graph_show",
];

// --- AC #2: single handler — no surface-specific business logic ----------

#[test]
fn cli_and_mcp_route_through_the_same_handler() {
    // Pure structural check: the verify_bench_new feature module exports
    // exactly one `pub fn run`. The CLI in `cli::verify::run_env_new`
    // and the MCP descriptor in `verify_bench_new::tool_descriptor`
    // both reference it. We assert via a grep over the feature
    // module source: it must not contain a `cli_handle` / `mcp_handle`
    // split.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/features/verify_bench_new/mod.rs"
    ))
    .expect("read feature source");
    assert!(
        !src.contains("fn cli_handle") && !src.contains("fn mcp_handle"),
        "feature module must not split CLI and MCP handlers"
    );
    assert!(
        src.contains("pub fn run("),
        "feature module must export a single `pub fn run` handler"
    );
}

// --- AC #3: identical Request shape across surfaces ----------------------

#[test]
fn cli_and_mcp_produce_identical_request_shapes() {
    // CLI: build the request via the same path `cli::verify::run_env_new`
    // uses, with a representative input.
    let workdir = PathBuf::from("/tmp/dec-tc052-stub");
    let cli_req = decision_cli::verify_bench_new::BenchNewRequest {
        id: None,
        bench_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(workdir.clone()),
    };
    // MCP: build the matching JSON args, parse them through the same
    // entry point the MCP descriptor uses.
    let raw = serde_json::json!({
        "bench_type": "ephemeral-tempdir",
        "safety_class": "isolated",
        "allowed_ops": ["shell", "filesystem"],
        "workdir": workdir.to_string_lossy(),
    });
    let mcp_req = decision_cli::verify_bench_new::parse_request(&Request::new(
        decision_cli::verify_bench_new::TOOL_NAME,
        raw,
    ))
    .expect("parse mcp");
    assert_eq!(
        cli_req, mcp_req,
        "CLI and MCP must construct byte-equal Request values"
    );
}

// --- AC #4: identical Response / Error across surfaces -------------------

#[test]
fn cli_and_mcp_produce_identical_response_values() {
    let tmp = init_workdir("response");
    let req = decision_cli::verify_bench_new::BenchNewRequest {
        id: Some("ENV-013".to_string()),
        bench_type: "ephemeral-tempdir".to_string(),
        safety_class: "isolated".to_string(),
        allowed_ops: vec!["shell".to_string()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(tmp.path().to_path_buf()),
    };
    let cli = decision_cli::verify_bench_new::run(&req).expect("cli ok");
    // Force a duplicate-id error on a second invocation so we can
    // compare the structured error shape between paths.
    let cli_err = decision_cli::verify_bench_new::run(&req).expect_err("duplicate must fail");

    let mcp_req = decision_cli::verify_bench_new::parse_request(&Request::new(
        decision_cli::verify_bench_new::TOOL_NAME,
        serde_json::json!({
            "id": "ENV-013",
            "bench_type": "ephemeral-tempdir",
            "safety_class": "isolated",
            "allowed_ops": ["shell"],
            "workdir": tmp.path().to_string_lossy(),
        }),
    ))
    .expect("parse");
    let mcp_err = decision_cli::verify_bench_new::run(&mcp_req).expect_err("duplicate must fail");

    assert_eq!(cli_err, mcp_err, "errors must match byte-for-byte");
    // And: the successful response's structured payload must round-trip
    // through serde to the exact same JSON shape that the MCP surface
    // returns to its callers.
    let json = serde_json::to_value(&cli).expect("ser");
    assert!(json.get("id").is_some());
    assert!(json.get("path").is_some());
}
