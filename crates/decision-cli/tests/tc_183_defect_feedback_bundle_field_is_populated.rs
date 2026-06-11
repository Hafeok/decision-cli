//! TC-183 — `defect_feedback::load_for` populates the bundle field for
//! the (feature, env) pair (FT-107 AC #1 + #2 negative paths).
//!
//! Validates: FT-107 · ADR-026 · ADR-066.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::feedback::{Feedback, Severity};
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::defect_feedback::load_for;
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};
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
            "decision-cli-tc183-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-183 fixture\n");
    body.push_str("phase: 3\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-183.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

/// Seed one feedback artifact into the orchestration store.
#[allow(clippy::too_many_arguments)]
fn seed_feedback(
    workdir: &Path,
    iri: &str,
    class: &str,
    target_role: &str,
    source_tc_iri: &str,
    lifecycle_state: &str,
    evidence: &str,
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
        class: class.to_string(),
        severity: Severity::Error,
        target_role: target_role.to_string(),
        evidence: evidence.to_string(),
        recommendation: None,
        lifecycle_state: lifecycle_state.to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-183/seed",
        ),
        source_artifact: Some(NamedNode::new(source_tc_iri).expect("tc iri")),
        addressing_artifact: addressing_artifact
            .map(|s| NamedNode::new(s).expect("addressing iri")),
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
fn tc_183_defect_feedback_bundle_field_is_populated() {
    let wd = WorkdirGuard::new("loader");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    // Fixture: FT-T1 with one TC, BNCH-001-ephemeral-cli (auto-seeded by init),
    // one graph in that env with one step linked to TC-Ta.
    let feature_id = "FT-T1";
    let tcs = ["TC-Ta"];
    write_feature_fixture(wd.path(), feature_id, &tcs);
    let _ = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-183".to_string()),
        verifies: feature_id.to_string(),
        environment: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed graph");
    let mut fields = BTreeMap::new();
    fields.insert("command".to_string(), "echo seed".to_string());
    fields.insert("expect-exit-code".to_string(), "0".to_string());
    let _ = verify_step_add::run(&StepAddRequest {
        graph_id: "VG-183".to_string(),
        step_type: "shell-command".to_string(),
        fields,
        provides_evidence_for: vec!["TC-Ta".to_string()],
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed step");

    // Seed four feedback artifacts:
    //   FB-1, FB-2: defect / verifier / produced / source=TC-Ta  → INCLUDED
    //   FB-3       : gap    / spec-author / produced / source=TC-Ta → excluded by class+role
    //   FB-4       : defect / verifier / produced / source=TC-other → excluded by TC out of scope
    //   FB-5       : defect / verifier / addressed / source=TC-Ta → excluded by lifecycle
    let tc_iri = "https://decision-cli.dev/ns/tc/TC-Ta";
    let tc_other = "https://decision-cli.dev/ns/tc/TC-Tother";
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-183:fb-1",
        "defect",
        "verifier",
        tc_iri,
        "produced",
        "VG-183 step 0 produced fail; expected exit 0 got 127",
        None,
    );
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-183:fb-2",
        "defect",
        "verifier",
        tc_iri,
        "produced",
        "VG-183 step 0 produced fail; file missing",
        None,
    );
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-183:fb-3",
        "gap",
        "spec-author",
        tc_iri,
        "produced",
        "TC body underspecified",
        None,
    );
    seed_feedback(
        wd.path(),
        "urn:dec:feedback:tc-183:fb-4",
        "defect",
        "verifier",
        tc_other,
        "produced",
        "different-feature failure",
        None,
    );
    // Note: a fifth case (terminal `addressed` lifecycle) is covered
    // implicitly by TC-185 — duplicating it here would require running
    // the full transition through `feedback_close::mark_addressed`,
    // which TC-185 already exercises end-to-end.

    // Call the loader.
    let records = load_for(wd.path(), feature_id, "BNCH-001-ephemeral-cli");
    assert_eq!(
        records.len(),
        2,
        "loader must return exactly the two open defect/verifier entries on in-scope TCs; got {records:?}"
    );
    let iris: Vec<&str> = records.iter().map(|r| r.feedback_iri.as_str()).collect();
    assert!(
        iris.contains(&"urn:dec:feedback:tc-183:fb-1"),
        "FB-1 must be included"
    );
    assert!(
        iris.contains(&"urn:dec:feedback:tc-183:fb-2"),
        "FB-2 must be included"
    );
    for rec in &records {
        assert_eq!(rec.class, "defect");
        assert_eq!(rec.source_tc, tc_iri);
        assert_eq!(rec.graph_id, "VG-183");
    }
    // Sorted ascending by IRI (deterministic for stable bundle_hash).
    assert!(
        records[0].feedback_iri <= records[1].feedback_iri,
        "records must be sorted by feedback_iri ascending"
    );
}
