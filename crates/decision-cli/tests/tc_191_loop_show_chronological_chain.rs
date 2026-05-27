//! TC-191 — `dec loop show <FT-NNN>` produces one entry per defect
//! feedback for the feature's TCs, chronologically sorted, with state
//! transitions and addressing artifacts surfaced.
//!
//! Validates: FT-109 · ADR-004.

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
    show::{run as run_show, LoopShowRequest},
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
            "decision-cli-tc191-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-191 fixture\n");
    body.push_str("phase: 3\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

#[allow(clippy::too_many_arguments)]
fn seed_feedback(
    workdir: &Path,
    iri: &str,
    source_tc_iri: &str,
    lifecycle_state: &str,
    source_session: &str,
    routed_at: Option<&str>,
    addressing_artifact: Option<&str>,
    evidence: &str,
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
        evidence: evidence.to_string(),
        recommendation: None,
        lifecycle_state: lifecycle_state.to_string(),
        source_session: NamedNode::new(source_session).expect("session iri"),
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
fn tc_191_loop_show_chronological_chain() {
    let wd = WorkdirGuard::new("show");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    write_feature_fixture(wd.path(), "FT-T191", &["TC-T191a", "TC-T191b"]);

    let tc_a = "https://decision-cli.dev/ns/tc/TC-T191a";
    let tc_b = "https://decision-cli.dev/ns/tc/TC-T191b";
    let tc_other = "https://decision-cli.dev/ns/tc/TC-other";

    // FB-1: T1, produced, no routed_at, no addressing.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-191:fb-1",
        tc_a,
        "produced",
        "https://decision-cli.dev/ns/activity/verify-graph-run/VG-007/ts-001",
        None,
        None,
        "evidence A",
    );
    // FB-2: T3, addressed by VG-PATCH-1.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-191:fb-2",
        tc_a,
        "addressed",
        "https://decision-cli.dev/ns/activity/verify-graph-run/VG-007/ts-002",
        Some("2026-05-27T08:00:03Z"),
        Some("https://decision-cli.dev/ns/graph/VG-PATCH-1"),
        "evidence B",
    );
    // FB-3: T2, produced. (Routed-at-derived sort would put this after
    // FB-1 and before FB-2.)
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-191:fb-3",
        tc_b,
        "routed",
        "https://decision-cli.dev/ns/activity/verify-graph-run/VG-009/ts-003",
        Some("2026-05-27T08:00:02Z"),
        None,
        "evidence C",
    );
    // FB-other: outside the feature's TCs.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-191:fb-other",
        tc_other,
        "produced",
        "https://decision-cli.dev/ns/activity/verify-graph-run/VG-010/ts-004",
        None,
        None,
        "irrelevant",
    );

    let resp = run_show(&LoopShowRequest {
        feature_id: "FT-T191".to_string(),
        workdir: wd.path().to_path_buf(),
        product_root: Some(wd.path().to_path_buf()),
        format: OutputFormat::Json,
    })
    .expect("loop show");

    // AC #1: exactly three entries; FB-other excluded.
    assert_eq!(
        resp.entries.len(),
        3,
        "expected 3 in-scope entries; got {:?}",
        resp.entries
            .iter()
            .map(|e| e.feedback_iri.as_str())
            .collect::<Vec<_>>()
    );
    for e in &resp.entries {
        assert_ne!(e.feedback_iri, "urn:dec:feedback:tc-191:fb-other");
    }

    // AC #2: chronological sort by routed_at — entries with no
    // routed_at sort first (empty string < any timestamp), then
    // FB-3 (08:00:02), then FB-2 (08:00:03).
    let ids: Vec<&str> = resp.entries.iter().map(|e| e.feedback_iri.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "urn:dec:feedback:tc-191:fb-1",
            "urn:dec:feedback:tc-191:fb-3",
            "urn:dec:feedback:tc-191:fb-2",
        ],
        "chronological order off; got {ids:?}"
    );

    // AC #4: FB-2's addressing_artifact resolves to its short id.
    let fb_2 = resp
        .entries
        .iter()
        .find(|e| e.feedback_iri == "urn:dec:feedback:tc-191:fb-2")
        .expect("FB-2");
    assert_eq!(fb_2.addressing_artifact_short.as_deref(), Some("VG-PATCH-1"));
    assert_eq!(fb_2.source_session_short, "VG-007");
    assert_eq!(fb_2.source_tc_short, "TC-T191a");
}
