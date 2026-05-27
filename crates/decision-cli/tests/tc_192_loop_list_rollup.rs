//! TC-192 — `dec loop list` rolls up open/closed defect feedback by
//! feature, with three state filters (open/closed/all).
//!
//! Validates: FT-109 · ADR-024.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::feedback::{Feedback, Severity};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::loop_inspect::{
    list::{run as run_list, LoopListRequest, StateFilter},
    OutputFormat,
};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
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
            "decision-cli-tc192-{tag}-{}-{}-{}",
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

fn write_tc_fixture(workdir: &Path, tc_id: &str, owning_feature: &str) {
    let dir = workdir.join(".product/tests");
    fs::create_dir_all(&dir).expect("create tests");
    let body = format!(
        "---\nid: {tc_id}\ntitle: TC-192 fixture\ntype: exit-criteria\nstatus: unimplemented\nvalidates:\n  features:\n  - {owning_feature}\n  adrs: []\nphase: 1\n---\n\nFixture.\n"
    );
    let fname = format!("{tc_id}-tc192-fixture.md");
    fs::write(dir.join(&fname), body).expect("write tc");
}

#[allow(clippy::too_many_arguments)]
fn seed_defect(
    workdir: &Path,
    iri: &str,
    source_tc_iri: &str,
    lifecycle_state: &str,
    routed_at: Option<&str>,
    addressing_artifact: Option<&str>,
) {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump).expect("load store");
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).expect("active scope");
    let stream_iri = NamedNode::new(&scope.stream_iri).expect("stream iri");
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone()).expect("writer");
    let fb = Feedback {
        iri: NamedNode::new(iri).expect("feedback iri"),
        class: "defect".to_string(),
        severity: Severity::Error,
        target_role: "implementer".to_string(),
        evidence: format!("seeded for {tc}", tc = source_tc_iri),
        recommendation: None,
        lifecycle_state: lifecycle_state.to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/verify-graph-run/VG-T192/ts-1",
        ),
        source_artifact: Some(NamedNode::new(source_tc_iri).expect("tc iri")),
        addressing_artifact: addressing_artifact
            .map(|s| NamedNode::new(s).expect("addressing iri")),
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: routed_at.map(String::from),
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: stream_iri,
    };
    let quads = fb.to_quads(orchestration_graph());
    writer.commit(Mutation::insert(quads)).expect("commit feedback");
    persist_store(&store, &dump).expect("persist store");
}

#[test]
fn tc_192_loop_list_rollup() {
    let wd = WorkdirGuard::new("list");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    // Three TCs, each pinned to a distinct feature.
    write_tc_fixture(wd.path(), "TC-T192A", "FT-T192a");
    write_tc_fixture(wd.path(), "TC-T192B", "FT-T192b");
    write_tc_fixture(wd.path(), "TC-T192C", "FT-T192c");

    let tc_a = "https://decision-cli.dev/ns/tc/TC-T192A";
    let tc_b = "https://decision-cli.dev/ns/tc/TC-T192B";
    let tc_c = "https://decision-cli.dev/ns/tc/TC-T192C";

    // FT-T192a: 3 open + 1 addressed.
    seed_defect(wd.path(), "urn:dec:feedback:tc-192:a-open-1", tc_a, "produced", None, None);
    seed_defect(wd.path(), "urn:dec:feedback:tc-192:a-open-2", tc_a, "produced", None, None);
    seed_defect(wd.path(), "urn:dec:feedback:tc-192:a-open-3", tc_a, "routed", Some("2026-05-27T01:00:00Z"), None);
    seed_defect(
        wd.path(),
        "urn:dec:feedback:tc-192:a-closed-1",
        tc_a,
        "addressed",
        Some("2026-05-27T02:00:00Z"),
        Some("https://decision-cli.dev/ns/graph/VG-T192-a-fix"),
    );

    // FT-T192b: 1 open.
    seed_defect(wd.path(), "urn:dec:feedback:tc-192:b-open-1", tc_b, "produced", None, None);

    // FT-T192c: 2 addressed (all closed).
    seed_defect(
        wd.path(),
        "urn:dec:feedback:tc-192:c-closed-1",
        tc_c,
        "addressed",
        Some("2026-05-27T03:00:00Z"),
        Some("https://decision-cli.dev/ns/graph/VG-T192-c-fix"),
    );
    seed_defect(
        wd.path(),
        "urn:dec:feedback:tc-192:c-closed-2",
        tc_c,
        "addressed",
        Some("2026-05-27T04:00:00Z"),
        Some("https://decision-cli.dev/ns/graph/VG-T192-c-fix"),
    );

    let req = LoopListRequest {
        workdir: wd.path().to_path_buf(),
        product_root: Some(wd.path().to_path_buf()),
        state: StateFilter::Open,
        format: OutputFormat::Json,
    };

    // --state=open (default) — only FT-T192a and FT-T192b appear.
    let open = run_list(&req).expect("loop list open");
    let open_ids: Vec<&str> = open.rows.iter().map(|r| r.feature_id.as_str()).collect();
    assert_eq!(
        open_ids,
        vec!["FT-T192a", "FT-T192b"],
        "open sort by open_count DESC then by IRI: got {open_ids:?}"
    );
    assert_eq!(open.rows[0].open_count, 3);
    assert_eq!(open.rows[0].closed_count, 1);
    assert_eq!(open.rows[1].open_count, 1);

    // --state=all — all three features.
    let all = run_list(&LoopListRequest {
        state: StateFilter::All,
        ..req.clone()
    })
    .expect("loop list all");
    let all_ids: Vec<&str> = all.rows.iter().map(|r| r.feature_id.as_str()).collect();
    assert_eq!(
        all_ids,
        vec!["FT-T192a", "FT-T192b", "FT-T192c"],
        "all-mode shows every feature; got {all_ids:?}"
    );

    // --state=closed — only FT-T192c.
    let closed = run_list(&LoopListRequest {
        state: StateFilter::Closed,
        ..req.clone()
    })
    .expect("loop list closed");
    let closed_ids: Vec<&str> = closed.rows.iter().map(|r| r.feature_id.as_str()).collect();
    assert_eq!(
        closed_ids,
        vec!["FT-T192c"],
        "closed-mode only includes fully-closed features; got {closed_ids:?}"
    );
}

