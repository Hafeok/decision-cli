//! TC-190 — the implementer dispatch gate. Two scenarios:
//!
//!   A) When a feature has NO outstanding implementer-targeted defect
//!      feedback, `load_for_implementer` returns an empty list — the
//!      payload's `defect_feedback` arrives empty and `dec implement`
//!      operates as a normal first-time dispatch.
//!   B) When outstanding `produced`-state implementer-targeted defect
//!      feedback exists for the feature's TCs, the loader returns
//!      non-empty — the payload carries the feedback and the worker
//!      sees runtime evidence. An `addressed` feedback (terminal
//!      lifecycle) does NOT count as outstanding.
//!
//! This is the loader-level half of the spec's acceptance criterion 4.
//! The full `dec implement` end-to-end exercising the dispatch
//! subprocess is covered by the existing `tc_018_finalize_*` test
//! family; this test focuses on the new FT-108 filter.
//!
//! Validates: FT-108 · ADR-031.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::feedback::address_walk::mark_batch_addressed;
use decision_cli::core::feedback::{Feedback, Severity};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::features::implement::defect_feedback::load_for_implementer;
use decision_cli::init::{run as init_run, DefinitionSource};
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
            "decision-cli-tc190-{tag}-{}-{}-{}",
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

fn seed_produced_defect(workdir: &Path, iri: &str, source_tc_iri: &str) {
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
        evidence: "TC-190: prior verify run reported regression".to_string(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-190/seed",
        ),
        source_artifact: Some(NamedNode::new(source_tc_iri).expect("tc iri")),
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: stream_iri,
    };
    let quads = fb.to_quads(orchestration_graph());
    writer
        .commit(Mutation::insert(quads))
        .expect("commit feedback");
    persist_store(&store, &dump).expect("persist store");
}

#[test]
fn tc_190_implement_gate_falls_through_on_outstanding_feedback() {
    let wd = WorkdirGuard::new("gate");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let tc_iri = "https://decision-cli.dev/ns/tc/TC-T190a";
    let tc_iris = vec![tc_iri.to_string()];

    // ----- Scenario A: clean state — loader returns empty. -----
    let empty = load_for_implementer(wd.path(), &tc_iris);
    assert!(
        empty.is_empty(),
        "scenario A: no defect feedback ⇒ loader returns empty; got {empty:?}"
    );

    // ----- Scenario B: seed one outstanding defect, expect it surfaced. -----
    let fb_iri = "urn:dec:feedback:tc-190:fb-1";
    seed_produced_defect(wd.path(), fb_iri, tc_iri);
    let surfaced = load_for_implementer(wd.path(), &tc_iris);
    assert_eq!(
        surfaced.len(),
        1,
        "scenario B: one produced defect ⇒ loader returns one entry; got {surfaced:?}"
    );
    assert_eq!(surfaced[0].feedback_iri, fb_iri);

    // ----- Boundary: an `addressed` feedback must NOT keep the gate open. -----
    // Walk the seeded feedback through to `addressed` (simulating that a
    // prior implementer cycle resolved it), then re-call the loader.
    let code_change_iri =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/code-change/tc-190-cc-1");
    let session_iri =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/activity/tc-190/closing");
    let now = chrono::Utc::now().to_rfc3339();
    mark_batch_addressed(
        wd.path(),
        &[fb_iri.to_string()],
        &code_change_iri,
        &session_iri,
        &now,
    )
    .expect("transition to addressed");
    let after_close = load_for_implementer(wd.path(), &tc_iris);
    assert!(
        after_close.is_empty(),
        "boundary: addressed feedback must NOT count as outstanding; got {after_close:?}"
    );
}
