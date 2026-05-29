//! TC-186 — worker returning `kind = Match` despite non-empty
//! `defect_feedback` in its bundle is rejected with
//! `Error::WorkerIgnoredFeedback` (FT-107 AC #4).
//!
//! Validates: FT-107 · ADR-022.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::feedback::{Feedback, Severity};
use decision_cli::core::handler::Error as HandlerError;
use decision_cli::core::scope::ActiveScope;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::{
    self,
    proposal::{GraphProposal, MatchProposal},
    worker::install_mock,
    GenerateMode, GenerateRequest,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

const FB_IRI: &str = "urn:dec:feedback:tc-186:fb-1";

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc186-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-186 fixture\n");
    body.push_str("phase: 3\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-186.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
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
        target_role: "verifier".to_string(),
        evidence: "TC-186: existing graph produced a failure".to_string(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-186/seed",
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
fn tc_186_worker_ignoring_feedback_is_rejected() {
    let wd = WorkdirGuard::new("ignored");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-T4";
    let tcs = ["TC-T4a"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    // Seed an existing covering graph (so matcher would normally short-circuit).
    let _ = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-186-existing".to_string()),
        verifies: feature_id.to_string(),
        environment: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed graph");
    let mut fields = BTreeMap::new();
    fields.insert("command".to_string(), "echo seed".to_string());
    fields.insert("expect-exit-code".to_string(), "0".to_string());
    let _ = verify_step_add::run(&StepAddRequest {
        graph_id: "VG-186-existing".to_string(),
        step_type: "shell-command".to_string(),
        fields,
        provides_evidence_for: vec!["TC-T4a".to_string()],
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed step");

    // Pre-seed a produced defect so the matcher gate falls through.
    let tc_iri = "https://decision-cli.dev/ns/tc/TC-T4a";
    seed_produced_defect(wd.path(), FB_IRI, tc_iri);

    // Install a degenerate mock worker that returns `Match` against the
    // existing (broken) graph despite the bundle carrying defect feedback.
    let _guard = install_mock(move |bundle| {
        assert!(
            !bundle.defect_feedback.is_empty(),
            "TC-186 precondition: bundle must carry defect feedback"
        );
        Ok(GraphProposal::new_match(
            bundle.bundle_hash.clone(),
            MatchProposal {
                graph_id: "VG-186-existing".to_string(),
                rationale: "TC-186 degenerate worker: ignoring feedback".to_string(),
            },
        ))
    });

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "BNCH-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Interactive,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let err = verify_graph_generate::run_generate(&req)
        .expect_err("worker returning Match with non-empty defect_feedback must be rejected");
    match err {
        HandlerError::WorkerIgnoredFeedback {
            feedback_iris,
            detail: _,
        } => {
            assert_eq!(
                feedback_iris,
                vec![FB_IRI.to_string()],
                "the rejection must enumerate the feedback IRIs the worker ignored"
            );
        }
        other => panic!("expected WorkerIgnoredFeedback, got {other:?}"),
    }
}
