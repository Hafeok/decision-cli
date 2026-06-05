//! FT-161 — `dec session show` renders cluster IRIs.
//!
//! Validates the cluster-cell + cluster-dispatch branches of
//! `features::implement::session_show::session_show` against a
//! bootstrapped workdir + the FT-146 persistence path.
//!
//! Validates: FT-161 · FT-146 · ADR-081.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::Utc;
use decision_cli::core::graph::cluster_session::{
    persist_cluster_run, CellSessionRecord, CellStatus, ClusterOutcome,
};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{
    load_store_from_dump, orchestration_dump_path, persist_store,
};
use decision_cli::features::implement::session_show;
use decision_cli::features::implement::WorkerResponseUsage;
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

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
            "decision-cli-ft161-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter
        ));
        fs::create_dir_all(&base).unwrap();
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

fn bootstrap(wd: &WorkdirGuard) {
    let seed = wd.path().join("stream.ttl");
    fs::write(&seed, STREAM_TTL).unwrap();
    init_run(wd.path(), DefinitionSource::File(seed)).unwrap();
}

/// Seed a `dec:Capability` with the three cost-rate triples FT-161's
/// renderer reads (`cost_input_per_m`, `cost_output_per_m`,
/// `cost_currency`). Goes through StreamWriter so the orchestration
/// store dump is consistent after the call.
fn seed_capability_cost(wd: &Path, cap_iri: &str, input_per_m: &str, output_per_m: &str, currency: &str) {
    let dump = orchestration_dump_path(wd);
    let store = load_store_from_dump(&dump).unwrap();
    let store = Arc::new(store);
    let scope = ActiveScope::load(wd).unwrap();
    let stream_iri = NamedNode::new(&scope.stream_iri).unwrap();
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).unwrap();

    let g: GraphName = orchestration_graph().into_owned().into();
    let cap = NamedNode::new(cap_iri).unwrap();
    let xsd_decimal = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal");
    let lit_decimal = |v: &str| Literal::new_typed_literal(v, xsd_decimal.clone());
    let quads = vec![
        Quad::new(
            cap.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#cost_input_per_m").into_owned(),
            lit_decimal(input_per_m),
            g.clone(),
        ),
        Quad::new(
            cap.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#cost_output_per_m").into_owned(),
            lit_decimal(output_per_m),
            g.clone(),
        ),
        Quad::new(
            cap,
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#cost_currency").into_owned(),
            Literal::new_simple_literal(currency),
            g,
        ),
    ];
    writer.commit(Mutation::insert(quads)).unwrap();
    persist_store(&store, &dump).unwrap();
}

fn seed_cluster_run(
    wd: &Path,
    cluster_iri_str: &str,
    feature: &str,
    task_type: &str,
    cells: Vec<CellSessionRecord>,
    outcome: ClusterOutcome,
) {
    let cluster_iri = NamedNode::new(cluster_iri_str).unwrap();
    let now = Utc::now();
    persist_cluster_run(
        wd,
        &cluster_iri,
        feature,
        task_type,
        now,
        now + chrono::Duration::milliseconds(150),
        outcome,
        &cells,
    )
    .unwrap();
}

fn cell(
    cluster_tt: &str,
    feature: &str,
    name: &str,
    cap: &str,
    usage: Option<(u64, u64)>,
    status: CellStatus,
) -> CellSessionRecord {
    let now = Utc::now();
    CellSessionRecord {
        iri: NamedNode::new(format!(
            "urn:dec:cluster-session:{cluster_tt}/{feature}/{name}"
        ))
        .unwrap(),
        capability: NamedNode::new(cap).unwrap(),
        usage: usage.map(|(b, o)| WorkerResponseUsage {
            input_tokens_base: b,
            input_tokens_cache_write: 0,
            input_tokens_cache_hit: 0,
            output_tokens: o,
        }),
        status,
        started_at: now,
        ended_at: now + chrono::Duration::milliseconds(50),
    }
}

/// TC-387 — per-cell renderer surfaces token breakdown + parent link.
#[test]
fn tc_387_cluster_cell_iri_renders_breakdown_and_parent_link() {
    let wd = WorkdirGuard::new("cell");
    bootstrap(&wd);
    let cap = "https://decision-cli.dev/ns/capability/qwen3-coder/v1";
    let cell_iri = "urn:dec:cluster-session:add-judge-worker/FT-T387/agent_loop";
    seed_cluster_run(
        wd.path(),
        "urn:dec:cluster-dispatch:add-judge-worker/FT-T387",
        "FT-T387",
        "add-judge-worker",
        vec![cell(
            "add-judge-worker",
            "FT-T387",
            "agent_loop",
            cap,
            Some((4321, 765)),
            CellStatus::Succeeded,
        )],
        ClusterOutcome::Succeeded,
    );

    let out = session_show(wd.path(), cell_iri).expect("render cell");
    assert!(out.contains("Cell session"), "expected cell-session header: {out}");
    assert!(out.contains(cell_iri), "expected cell IRI in output");
    assert!(out.contains("urn:dec:cluster-dispatch:add-judge-worker/FT-T387"),
        "expected parent cluster IRI in output");
    assert!(out.contains("Capability     https://decision-cli.dev/ns/capability/qwen3-coder/v1"),
        "expected capability line: {out}");
    assert!(out.contains("succeeded"), "expected status line: {out}");
    assert!(out.contains("worker-reported"), "expected usage source: {out}");
    // Token values appear with right-aligned formatting.
    assert!(out.contains("4321"), "expected input_tokens_base value: {out}");
    assert!(out.contains("765"), "expected output_tokens value: {out}");
}

/// TC-388 — cluster IRI renders header + per-cell table + currency total.
#[test]
fn tc_388_cluster_dispatch_iri_renders_table_with_currency_total() {
    let wd = WorkdirGuard::new("dispatch");
    bootstrap(&wd);
    // Seed a capability with cost rates so the cost column resolves.
    let cap = "https://decision-cli.dev/ns/capability/ft-t388-coder/v1";
    seed_capability_cost(wd.path(), cap, "0.20", "0.80", "EUR");
    let mech = "urn:dec:capability:mechanical";
    let cluster = "urn:dec:cluster-dispatch:add-cli-subcommand/FT-T388";
    seed_cluster_run(
        wd.path(),
        cluster,
        "FT-T388",
        "add-cli-subcommand",
        vec![
            cell("add-cli-subcommand", "FT-T388", "clap_args_module", cap,
                 Some((1000, 200)), CellStatus::Succeeded),
            cell("add-cli-subcommand", "FT-T388", "handler_module", cap,
                 Some((3000, 600)), CellStatus::Succeeded),
            cell("add-cli-subcommand", "FT-T388", "registration_wiring", mech,
                 None, CellStatus::Mechanical),
        ],
        ClusterOutcome::Succeeded,
    );

    let out = session_show(wd.path(), cluster).expect("render cluster");
    assert!(out.contains("Cluster        urn:dec:cluster-dispatch:add-cli-subcommand/FT-T388"));
    assert!(out.contains("Feature        FT-T388"), "expected feature line: {out}");
    assert!(out.contains("Task type      add-cli-subcommand"));
    assert!(out.contains("Outcome        succeeded"));
    assert!(out.contains("Cells (3):"), "expected cell count header: {out}");
    // Per-cell rows render the short cell name.
    assert!(out.contains("clap_args_module"));
    assert!(out.contains("handler_module"));
    assert!(out.contains("registration_wiring"));
    // Mechanical cell carries em-dash for cost; non-mechanical carry €.
    assert!(out.contains("€"), "expected EUR currency symbol: {out}");
    assert!(out.contains("TOTAL EUR"), "expected currency-tagged TOTAL row: {out}");
    // Totals: 4000 base, 800 output across the two priced cells.
    assert!(out.contains("4000"), "expected total base = 4000: {out}");
    assert!(out.contains("800"), "expected total output = 800: {out}");
}

/// TC-389 — re-dispatching the same cluster IRI must dedupe to one row
/// per cell and annotate "(N runs aggregated)" when multiple
/// `dec:clusterOutcome` values land.
#[test]
fn tc_389_cluster_dispatch_dedupes_multi_run_and_annotates_outcomes() {
    let wd = WorkdirGuard::new("dedup");
    bootstrap(&wd);
    let cap = "https://decision-cli.dev/ns/capability/code-writer/v1";
    let cluster = "urn:dec:cluster-dispatch:add-cli-subcommand/FT-T389";

    // Run 1: cluster ran and succeeded.
    seed_cluster_run(
        wd.path(),
        cluster,
        "FT-T389",
        "add-cli-subcommand",
        vec![cell(
            "add-cli-subcommand",
            "FT-T389",
            "clap_args_module",
            cap,
            Some((1234, 100)),
            CellStatus::Succeeded,
        )],
        ClusterOutcome::Succeeded,
    );
    // Run 2: same cluster, audit failed this iteration.
    seed_cluster_run(
        wd.path(),
        cluster,
        "FT-T389",
        "add-cli-subcommand",
        vec![cell(
            "add-cli-subcommand",
            "FT-T389",
            "clap_args_module",
            cap,
            Some((1234, 100)),
            CellStatus::Failed,
        )],
        ClusterOutcome::AuditFailed,
    );

    let out = session_show(wd.path(), cluster).expect("render");
    // Dedupe: exactly one row for clap_args_module, not two.
    let n = out.matches("clap_args_module").count();
    assert_eq!(
        n, 1,
        "expected exactly 1 row for clap_args_module across two runs, got {n}: {out}"
    );
    // Annotation surfaces.
    assert!(
        out.contains("runs aggregated"),
        "expected multi-run annotation: {out}"
    );
}

/// TC-390 — non-cluster IRIs fall through unchanged. We use an IRI that
/// matches neither cluster prefix and assert the slice-1 "no Session
/// with IRI" error surfaces (the existing path), not a cluster-flavoured
/// error. This pins the routing contract.
#[test]
fn tc_390_non_cluster_iri_falls_through_to_existing_renderer() {
    let wd = WorkdirGuard::new("passthrough");
    bootstrap(&wd);
    let other = "https://decision-cli.dev/ns/activity/some-other-shape/X-T390";
    let err = session_show(wd.path(), other).expect_err("not a Session");
    let msg = format!("{err:#}");
    // The slice-1 path emits this exact prefix; cluster paths use different
    // verbiage ("no cluster cell session with IRI"). Catch routing drift.
    assert!(
        msg.contains("no Session with IRI"),
        "expected slice-1 error verb for non-cluster IRI, got: {msg}"
    );
    assert!(!msg.contains("cluster"),
        "non-cluster IRI must not produce a cluster-flavoured error: {msg}");

    // Verify the FT-146 store integrity check too — orchestration dump is
    // readable post-bootstrap (sanity).
    let dump = orchestration_dump_path(wd.path());
    let _ = load_store_from_dump(&dump).expect("dump loads");
}
