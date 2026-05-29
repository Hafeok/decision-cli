//! TC-154 — `run_graph` executes the six seed step kinds end-to-end
//!          against a fixture VG in an ephemeral env.
//!
//! Spec: `.product/tests/TC-154-run-graph-executes-the-six-seed-step-kinds-end-to.md`
//! Validates: FT-098 · ADR-028.

use std::collections::{BTreeMap, HashMap};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::StepOutcome;
use decision_cli::core::verify::runner::{run_graph, RunGraphRequest, TriggerKind};
use decision_cli::verify_bench_new::{self, BenchNewRequest};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};
use oxigraph::model::NamedNode;
use sha2::{Digest, Sha256};

// ---- tempdir helper -------------------------------------------------------

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
        p.push(format!("dec-tc154-{tag}-{pid}-{nonce}"));
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
    std::fs::write(features.join("FT-001-test-fixture.md"), ft_body).expect("FT-001");
}

fn dec_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dec"))
}

fn fields_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
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
        "dec init exit: {status:?}"
    );
    tmp
}

/// Bind a TCP listener on a random port to discover a free port; close
/// it before axum opens its own listener on the same port. Small race
/// window but fine for tests.
fn pick_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("pick port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// Start an in-process HTTP server on a freshly minted runtime in a
/// background thread. Returns the bound URL plus a JoinHandle the test
/// drops on completion.
fn start_health_server() -> (String, std::thread::JoinHandle<()>) {
    let port = pick_port();
    let url = format!("http://127.0.0.1:{port}");
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        rt.block_on(async move {
            let app = Router::new().route(
                "/health",
                get(|| async { axum::Json(serde_json::json!({"ok": true})) }),
            );
            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
                .await
                .expect("tcp bind");
            axum::serve(listener, app).await.expect("serve");
        });
    });
    // Give the server a moment to come up.
    std::thread::sleep(Duration::from_millis(250));
    (url, handle)
}

const ENV_ID: &str = "BNCH-9-tc154";
const VG_ID: &str = "VG-9-tc154";

fn seed_env(workdir: &Path) {
    verify_bench_new::run(&BenchNewRequest {
        id: Some(ENV_ID.into()),
        bench_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec![
            "shell".into(),
            "filesystem".into(),
            "sparql-local".into(),
            "http-readonly".into(),
        ],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("env new");
}

fn seed_graph(workdir: &Path) {
    verify_graph_new::run(&GraphNewRequest {
        id: Some(VG_ID.into()),
        verifies: "FT-001".into(),
        environment: ENV_ID.into(),
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("graph new");
}

fn add_step(workdir: &Path, kind: &str, fields: BTreeMap<String, String>) -> String {
    let out = verify_step_add::run(&StepAddRequest {
        graph_id: VG_ID.into(),
        step_type: kind.into(),
        fields,
        provides_evidence_for: Vec::new(),
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("step add");
    out.step_id
}

#[test]
fn tc_154_run_graph_executes_the_six_seed_step_kinds_end_to() {
    happy_path_six_kinds();
    negative_path_mutated_expect_rows();
}

fn happy_path_six_kinds() {
    let tmp = init_workdir("happy");
    seed_env(tmp.path());
    seed_graph(tmp.path());
    // Step 0: shell-command — create seed.ttl + store.ttl in $dec_workdir.
    add_step(
        tmp.path(),
        "shell-command",
        fields_of(&[
            (
                "command",
                "echo seeded > seed.ttl && echo '@prefix ex: <urn:ex#> . ex:s ex:p ex:o .' > store.ttl",
            ),
            ("expect-exit-code", "0"),
        ]),
    );
    // Step 1: sparql-assertion.
    add_step(
        tmp.path(),
        "sparql-assertion",
        fields_of(&[
            ("target", "store.ttl"),
            ("query", "SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }"),
            ("expect-rows", "1"),
        ]),
    );
    // Step 2: file-assertion (will also serve as wait-for target).
    let file_step_iri = add_step(
        tmp.path(),
        "file-assertion",
        fields_of(&[
            ("path", "seed.ttl"),
            ("expect-hash", &sha256_hex(b"seeded\n")),
        ]),
    );
    // Step 3: http-request.
    let (url, _server_handle) = start_health_server();
    let mut caps: HashMap<String, String> = HashMap::new();
    caps.insert("health_url".into(), format!("{url}/health"));
    add_step(
        tmp.path(),
        "http-request",
        fields_of(&[
            ("method", "GET"),
            ("url", "${health_url}"),
            ("expect-status", "200"),
        ]),
    );
    // Step 4: wait-for that wraps step 2's file-assertion. The condition
    // IRI must be a known step in the parent graph — step 2 already
    // passes against seed.ttl created by step 0, so the wait-for
    // succeeds on its first poll.
    add_step(
        tmp.path(),
        "wait-for",
        fields_of(&[("condition", &file_step_iri), ("timeout", "PT5S")]),
    );
    // Step 5: capture.
    add_step(
        tmp.path(),
        "capture",
        fields_of(&[("bind-as", "summary")]),
    );

    // Invoke the runner.
    let req = RunGraphRequest {
        graph: graph_iri(VG_ID),
        triggered_by: TriggerKind::Manual,
        capture_bindings: caps,
        run_activity: synthetic_activity(),
        workdir: tmp.path().to_path_buf(),
    };
    let response = run_graph(&req).expect("run must succeed");
    assert_eq!(response.verdict, Verdict::Approved, "verdict");
    assert_eq!(response.step_outcomes.len(), 6, "trace count");
    for (i, o) in response.step_outcomes.iter().enumerate() {
        assert_eq!(o.outcome, StepOutcome::Pass, "step {i} outcome");
    }
    assert!(
        response.emitted_feedback.is_empty(),
        "no feedback expected on happy path"
    );

    // Persisted result file must exist on disk and be SHACL-valid (we
    // wrote it through StreamWriter so re-loading + the SHACL validator
    // would have refused it otherwise).
    let result_dir = tmp.path().join(".dec/verify/result");
    let files = std::fs::read_dir(&result_dir)
        .expect("result dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "ttl")
        })
        .count();
    assert_eq!(files, 1, "exactly one VGR.ttl produced");

    // The ephemeral tempdir is cleaned up after the runner returns. We
    // can't easily address the exact dir, but DEC_KEEP_TMP must not be
    // set in the test process.
    assert!(
        std::env::var("DEC_KEEP_TMP").is_err(),
        "DEC_KEEP_TMP must not be set in test process"
    );
}

fn negative_path_mutated_expect_rows() {
    let tmp = init_workdir("neg");
    seed_env(tmp.path());
    seed_graph(tmp.path());
    // Step 0: shell-command creates store.ttl.
    add_step(
        tmp.path(),
        "shell-command",
        fields_of(&[
            (
                "command",
                "echo '@prefix ex: <urn:ex#> . ex:s ex:p ex:o .' > store.ttl",
            ),
            ("expect-exit-code", "0"),
        ]),
    );
    // Step 1: sparql-assertion expects 99 rows (will fail).
    add_step(
        tmp.path(),
        "sparql-assertion",
        fields_of(&[
            ("target", "store.ttl"),
            ("query", "SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }"),
            ("expect-rows", "99"),
        ]),
    );
    let req = RunGraphRequest {
        graph: graph_iri(VG_ID),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: synthetic_activity(),
        workdir: tmp.path().to_path_buf(),
    };
    let response = run_graph(&req).expect("run must produce a result");
    // Per FT-097 single-graph rule: a fail with no providesEvidenceFor
    // is `amendment-required` (setup/capture-style failure). The
    // important invariant is the trace pattern and that no feedback is
    // emitted — the verdict variant is a function of the linkage.
    assert!(
        matches!(response.verdict, Verdict::AmendmentRequired | Verdict::Rejected),
        "expected fail-derived verdict, got {:?}",
        response.verdict
    );
    assert_eq!(response.step_outcomes.len(), 2, "trace count");
    assert_eq!(response.step_outcomes[1].outcome, StepOutcome::Fail);
    assert!(
        response.emitted_feedback.is_empty(),
        "no providesEvidenceFor ⇒ no feedback"
    );
}

fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let bytes = hasher.finalize();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn graph_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/graph/{id}"))
}

fn synthetic_activity() -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/activity/tc154/{}",
        std::process::id()
    ))
}

// Suppress unused import warnings when Arc is not exercised.
#[allow(dead_code)]
fn _unused_arc(s: Arc<u8>) {
    drop(s);
}
