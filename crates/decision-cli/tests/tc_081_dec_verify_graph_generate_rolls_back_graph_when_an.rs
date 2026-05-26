//! TC-081 — `dec verify graph generate --accept` rolls back the graph
//! when any step-add fails.
//!
//! Validates: FT-049 · ADR-030.
//! Spec: `.product/tests/TC-081-dec-verify-graph-generate-rolls-back-graph-when-an.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::{
    self,
    persist::{reset_writer_call_log, writer_call_log},
    proposal::{GraphProposal, NewProposal, ProposedStep},
    worker::install_mock,
    GenerateMode, GenerateRequest,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc081-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create workdir");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_seed_definition(dir: &Path) -> PathBuf {
    let p = dir.join("stream.ttl");
    fs::write(&p, STREAM_TTL).expect("write seed");
    p
}

fn write_feature_fixture(workdir: &Path, feature_id: &str, tcs: &[&str]) {
    let dir = workdir.join(".product/features");
    fs::create_dir_all(&dir).expect("create features");
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("id: {feature_id}\n"));
    body.push_str("title: TC-081 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-081.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

/// Build a two-step proposal where:
///   * step 1 is a valid `shell-command`,
///   * step 2 is a `sparql-assertion` MISSING `query` — SHACL/field
///     validation rejects it (TC-081 simulated by injecting a
///     malformed ProposedStep).
fn build_malformed_two_step_proposal(bundle_hash: &str) -> GraphProposal {
    let mut good = serde_json::Map::new();
    good.insert(
        "command".to_string(),
        serde_json::Value::String("echo hi".to_string()),
    );
    good.insert(
        "expect-exit-code".to_string(),
        serde_json::Value::String("0".to_string()),
    );
    let mut bad = serde_json::Map::new();
    // SparqlAssertion requires both `target` AND `query`; we omit `query`
    // so verify_step_add::run returns SchemaViolation per FT-044.
    bad.insert(
        "target".to_string(),
        serde_json::Value::String(".dec/store".to_string()),
    );
    GraphProposal::new_new(
        bundle_hash,
        NewProposal {
            environment: "ENV-001-ephemeral-cli".to_string(),
            steps: vec![
                ProposedStep {
                    step_type: "shell-command".to_string(),
                    fields: good,
                    provides_evidence_for: vec!["TC-Na".to_string()],
                },
                ProposedStep {
                    step_type: "sparql-assertion".to_string(),
                    fields: bad,
                    provides_evidence_for: vec!["TC-Nb".to_string()],
                },
            ],
            rationale: "TC-081 mock: second step is malformed (missing query)".to_string(),
            addressed_feedback_iris: Vec::new(),
        },
    )
}

#[test]
fn tc_081_dec_verify_graph_generate_rolls_back_graph_when_an() {
    let wd = WorkdirGuard::new("rollback");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-Nmock";
    let tcs = ["TC-Na", "TC-Nb"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    reset_writer_call_log();
    let _guard = install_mock(|bundle| Ok(build_malformed_two_step_proposal(&bundle.bundle_hash)));

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "ENV-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Accept,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };

    // AC: returns Error::SchemaViolation.
    let err = verify_graph_generate::run_generate(&req).expect_err("must fail");
    match &err {
        HandlerError::SchemaViolation { detail } => {
            assert!(
                detail.to_lowercase().contains("query") || detail.to_lowercase().contains("sparql"),
                "SchemaViolation diagnostic should mention the failing field; \
                 got {detail}"
            );
        }
        other => panic!("expected SchemaViolation, got {other:?}"),
    }

    // AC: no graph file remains on disk after the rollback.
    let graph_dir = wd.path().join(".dec/verify/graph");
    if graph_dir.exists() {
        let stragglers: Vec<_> = fs::read_dir(&graph_dir)
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().into_string().unwrap_or_default())
            .filter(|n| n.ends_with(".ttl"))
            .collect();
        assert!(
            stragglers.is_empty(),
            "TC-081: no .ttl should survive rollback; got {stragglers:?}"
        );
    }

    // Writer log proves: graph_new ran (the empty graph was created)
    // and step_add was attempted; the failure rolled both back.
    let log = writer_call_log();
    assert_eq!(log.graph_new_calls, 1, "graph_new should have run once");
    assert!(
        log.step_add_calls >= 1,
        "at least the first step-add should have been attempted"
    );
}
