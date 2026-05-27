//! TC-189 — when the code-writer cites a feedback IRI in its
//! `addressed_feedback_iris`, the implement-side accept path
//! transitions that feedback from `produced` to `addressed` with the
//! new CodeChange IRI as `dec:addressingArtifact`.
//!
//! Covers the lifecycle transition via the shared `address_walk` helper
//! (the `transition_addressed_feedback` function in
//! `features::implement::mod` is a thin wrapper around it).
//!
//! Validates: FT-108 · ADR-024 · ADR-026.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::feedback::address_walk::mark_batch_addressed;
use decision_cli::core::feedback::{Feedback, Severity};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

const FB_IRI: &str = "urn:dec:feedback:tc-189:fb-1";

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc189-{tag}-{}-{}-{}",
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

fn seed_produced_defect_for_implementer(workdir: &Path, iri: &str, source_tc_iri: &str) {
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
        evidence: "VG step 0 failed; cargo test panicked".to_string(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-189/seed",
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
    writer.commit(Mutation::insert(quads)).expect("commit feedback");
    persist_store(&store, &dump).expect("persist store");
}

fn read_feedback_lifecycle(workdir: &Path, iri: &str) -> (String, Option<String>) {
    let dump_body = fs::read_to_string(orchestration_dump_path(workdir)).expect("read dump");
    let mut state = String::from("(unknown)");
    let mut addr: Option<String> = None;
    for line in dump_body.lines() {
        if !line.starts_with(&format!("<{iri}>")) {
            continue;
        }
        if line.contains("#lifecycleState") {
            if let Some(start) = line.find('"') {
                let rest = &line[start + 1..];
                if let Some(end) = rest.find('"') {
                    state = rest[..end].to_string();
                }
            }
        }
        if line.contains("#addressingArtifact") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                addr = Some(parts[2].trim_matches(|c| c == '<' || c == '>').to_string());
            }
        }
    }
    (state, addr)
}

#[test]
fn tc_189_dispatch_transitions_consumed_feedback() {
    let wd = WorkdirGuard::new("transition");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let tc_iri = "https://decision-cli.dev/ns/tc/TC-T189a";
    seed_produced_defect_for_implementer(wd.path(), FB_IRI, tc_iri);

    // Sanity: feedback starts in `produced`.
    let (state_before, addr_before) = read_feedback_lifecycle(wd.path(), FB_IRI);
    assert_eq!(state_before, "produced");
    assert!(addr_before.is_none());

    // Simulate the implementer's accept path: a fresh CodeChange just
    // landed and the worker cited this feedback. Walk it through.
    let code_change_iri = NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/code-change/test-tc189-codechange-1",
    );
    let session_iri = NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/activity/implement/test-tc189-session",
    );
    let now = chrono::Utc::now().to_rfc3339();
    let n = mark_batch_addressed(
        wd.path(),
        &[FB_IRI.to_string()],
        &code_change_iri,
        &session_iri,
        &now,
    )
    .expect("mark_batch_addressed");
    assert_eq!(n, 1, "exactly one feedback transitioned");

    // AC: feedback is now `addressed`, with the new CodeChange IRI as
    // `dec:addressingArtifact`.
    let (state_after, addr_after) = read_feedback_lifecycle(wd.path(), FB_IRI);
    assert_eq!(
        state_after, "addressed",
        "feedback must transition to addressed; got {state_after}"
    );
    assert_eq!(
        addr_after.as_deref(),
        Some(code_change_iri.as_str()),
        "addressing_artifact must point at the new CodeChange; got {addr_after:?}"
    );
}
