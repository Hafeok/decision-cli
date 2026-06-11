//! FT-162 — `dec session list` surfaces cluster-dispatch activities and
//! renders cluster cell sessions with parent feature/status.
//!
//! Validates: FT-162 · FT-161 · FT-146 · ADR-081.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use decision_cli::core::cluster_session::{
    persist_cluster_run, CellSessionRecord, CellStatus, ClusterOutcome,
};
use decision_cli::features::implement::session_show;
use decision_cli::features::implement::WorkerResponseUsage;
use decision_cli::features::session_inspect::list;
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::model::NamedNode;

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
            "decision-cli-ft162-{tag}-{}-{}-{}",
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

fn make_cell(tt: &str, feat: &str, name: &str, status: CellStatus) -> CellSessionRecord {
    let now = Utc::now();
    CellSessionRecord {
        iri: NamedNode::new(format!("urn:dec:cluster-session:{tt}/{feat}/{name}")).unwrap(),
        capability: NamedNode::new("https://decision-cli.dev/ns/capability/qwen3-coder/v1")
            .unwrap(),
        usage: Some(WorkerResponseUsage {
            input_tokens_base: 100,
            input_tokens_cache_write: 0,
            input_tokens_cache_hit: 0,
            output_tokens: 50,
        }),
        status,
        started_at: now,
        ended_at: now + chrono::Duration::milliseconds(40),
    }
}

fn persist_one_run(
    wd: &Path,
    tt: &str,
    feat: &str,
    cells: Vec<CellSessionRecord>,
    outcome: ClusterOutcome,
) -> String {
    let cluster_iri_str = format!("urn:dec:cluster-dispatch:{tt}/{feat}");
    let cluster_iri = NamedNode::new(&cluster_iri_str).unwrap();
    let now = Utc::now();
    persist_cluster_run(
        wd,
        &cluster_iri,
        feat,
        tt,
        now,
        now + chrono::Duration::milliseconds(80),
        outcome,
        &cells,
    )
    .unwrap();
    cluster_iri_str
}

/// TC-391 — cluster dispatch activities (parent IRI) surface in list with
/// `featureId` + `clusterOutcome` projected onto the standard row shape.
#[test]
fn tc_391_session_list_surfaces_cluster_dispatch_activities() {
    let wd = WorkdirGuard::new("dispatch");
    bootstrap(&wd);
    let cluster = persist_one_run(
        wd.path(),
        "add-cli-subcommand",
        "FT-T391",
        vec![make_cell(
            "add-cli-subcommand",
            "FT-T391",
            "handler",
            CellStatus::Succeeded,
        )],
        ClusterOutcome::Succeeded,
    );
    let rows = list(wd.path(), 100, 0).expect("list");
    let dispatch = rows
        .iter()
        .find(|r| r.iri == cluster)
        .expect("expected cluster dispatch IRI in list output");
    assert_eq!(dispatch.feature_id, "FT-T391");
    assert_eq!(
        dispatch.status, "succeeded",
        "clusterOutcome should project onto ?status"
    );
}

/// TC-392 — cluster cell sessions render with their parent's feature
/// (lifted via prov:wasInformedBy) and their dec:cellStatus.
#[test]
fn tc_392_session_list_renders_cluster_cells_with_parent_feature_and_status() {
    let wd = WorkdirGuard::new("cell");
    bootstrap(&wd);
    let cells = vec![
        make_cell(
            "add-judge-worker",
            "FT-T392",
            "agent_loop",
            CellStatus::Succeeded,
        ),
        make_cell(
            "add-judge-worker",
            "FT-T392",
            "system_prompt",
            CellStatus::Mechanical,
        ),
    ];
    let _ = persist_one_run(
        wd.path(),
        "add-judge-worker",
        "FT-T392",
        cells,
        ClusterOutcome::Succeeded,
    );

    let rows = list(wd.path(), 100, 0).expect("list");
    let agent = rows
        .iter()
        .find(|r| r.iri.ends_with("/agent_loop"))
        .expect("expected agent_loop cell row");
    assert_eq!(agent.feature_id, "FT-T392", "feature lifted from parent");
    assert_eq!(
        agent.status, "succeeded",
        "cellStatus projected onto ?status"
    );

    let prompt = rows
        .iter()
        .find(|r| r.iri.ends_with("/system_prompt"))
        .expect("expected system_prompt cell row");
    assert_eq!(prompt.feature_id, "FT-T392");
    assert_eq!(prompt.status, "mechanical");
}

/// TC-393 — re-dispatching the same cluster IRI dedupes to a single row
/// per cell via GROUP BY ?session.
#[test]
fn tc_393_cluster_cell_with_multiple_dispatches_dedupes_to_single_row() {
    let wd = WorkdirGuard::new("dedup");
    bootstrap(&wd);
    persist_one_run(
        wd.path(),
        "add-cli-subcommand",
        "FT-T393",
        vec![make_cell(
            "add-cli-subcommand",
            "FT-T393",
            "handler",
            CellStatus::Succeeded,
        )],
        ClusterOutcome::Succeeded,
    );
    // Second iteration: same cluster IRI, same cell, different status.
    persist_one_run(
        wd.path(),
        "add-cli-subcommand",
        "FT-T393",
        vec![make_cell(
            "add-cli-subcommand",
            "FT-T393",
            "handler",
            CellStatus::Failed,
        )],
        ClusterOutcome::AuditFailed,
    );

    let rows = list(wd.path(), 100, 0).expect("list");
    let handler_rows: Vec<_> = rows
        .iter()
        .filter(|r| r.iri.ends_with("/handler"))
        .collect();
    assert_eq!(
        handler_rows.len(),
        1,
        "expected exactly 1 row for the handler cell across two runs, got {}: {:?}",
        handler_rows.len(),
        handler_rows.iter().map(|r| &r.iri).collect::<Vec<_>>()
    );
    let cluster_rows: Vec<_> = rows
        .iter()
        .filter(|r| r.iri == "urn:dec:cluster-dispatch:add-cli-subcommand/FT-T393")
        .collect();
    assert_eq!(
        cluster_rows.len(),
        1,
        "expected exactly 1 row for the cluster dispatch IRI"
    );
}

/// TC-394 — every IRI returned by `dec session list` resolves via
/// `dec session show` (ADR-081 totality). Pins the contract complement
/// FT-162 closes.
#[test]
fn tc_394_every_listed_iri_resolves_via_show() {
    let wd = WorkdirGuard::new("totality");
    bootstrap(&wd);
    persist_one_run(
        wd.path(),
        "add-cli-subcommand",
        "FT-T394",
        vec![
            make_cell(
                "add-cli-subcommand",
                "FT-T394",
                "clap_args",
                CellStatus::Succeeded,
            ),
            make_cell(
                "add-cli-subcommand",
                "FT-T394",
                "handler",
                CellStatus::Succeeded,
            ),
            make_cell(
                "add-cli-subcommand",
                "FT-T394",
                "wiring",
                CellStatus::Mechanical,
            ),
        ],
        ClusterOutcome::Succeeded,
    );

    let rows = list(wd.path(), 100, 0).expect("list");
    assert!(
        rows.len() >= 4,
        "expected ≥4 rows (3 cells + 1 cluster), got {}",
        rows.len()
    );
    let mut unresolved: Vec<(String, String)> = Vec::new();
    for row in &rows {
        match session_show(wd.path(), &row.iri) {
            Ok(_) => {}
            Err(e) => unresolved.push((row.iri.clone(), format!("{e}"))),
        }
    }
    assert!(
        unresolved.is_empty(),
        "ADR-081 totality violated — {} listed IRI(s) did not resolve via show:\n{}",
        unresolved.len(),
        unresolved
            .iter()
            .map(|(i, e)| format!("  {i}: {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
