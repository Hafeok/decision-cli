//! TC-188 — implementer-side `load_for_implementer` populates the
//! defect_feedback array with `class=defect targetRole=implementer
//! lifecycleState=produced` feedback whose `sourceArtifact` is in the
//! supplied TC-IRI set, and excludes anything else.
//!
//! Validates: FT-108 · ADR-026.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
            "decision-cli-tc188-{tag}-{}-{}-{}",
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

#[allow(clippy::too_many_arguments)]
fn seed_feedback(
    workdir: &Path,
    iri: &str,
    class: &str,
    target_role: &str,
    source_tc_iri: &str,
    lifecycle_state: &str,
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
        class: class.to_string(),
        severity: Severity::Error,
        target_role: target_role.to_string(),
        evidence: evidence.to_string(),
        recommendation: None,
        lifecycle_state: lifecycle_state.to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-188/seed",
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

#[test]
fn tc_188_dispatch_payload_carries_defect_feedback() {
    let wd = WorkdirGuard::new("loader");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let tc_in_a = "https://decision-cli.dev/ns/tc/TC-T188a";
    let tc_in_b = "https://decision-cli.dev/ns/tc/TC-T188b";
    let tc_other = "https://decision-cli.dev/ns/tc/TC-Tother";

    // FB-1: in scope, right class+role+state → INCLUDED.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-188:fb-1",
        "defect",
        "implementer",
        tc_in_a,
        "produced",
        "VG step 0 produced fail; cargo test panicked",
    );
    // FB-2: in scope, different TC → INCLUDED.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-188:fb-2",
        "defect",
        "implementer",
        tc_in_b,
        "produced",
        "VG step 1 produced fail; assertion failed",
    );
    // FB-3: wrong role (verifier) → EXCLUDED.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-188:fb-3",
        "defect",
        "verifier",
        tc_in_a,
        "produced",
        "graph step setup failure",
    );
    // FB-4: implementer-targeted but for a TC outside the supplied set → EXCLUDED.
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-188:fb-4",
        "defect",
        "implementer",
        tc_other,
        "produced",
        "different-feature failure",
    );

    let tc_iris = vec![tc_in_a.to_string(), tc_in_b.to_string()];
    let records = load_for_implementer(wd.path(), &tc_iris);
    assert_eq!(
        records.len(),
        2,
        "loader must return exactly the two in-scope implementer/produced entries; got {records:?}"
    );
    let iris: Vec<&str> = records.iter().map(|r| r.feedback_iri.as_str()).collect();
    assert!(iris.contains(&"urn:dec:feedback:tc-188:fb-1"));
    assert!(iris.contains(&"urn:dec:feedback:tc-188:fb-2"));
    for rec in &records {
        assert_eq!(rec.class, "defect");
        // The implementer loader joins by TC, not by graph, so it
        // deliberately leaves graph_id empty.
        assert_eq!(rec.graph_id, "");
    }
}
