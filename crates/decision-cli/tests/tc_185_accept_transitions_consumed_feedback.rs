//! TC-185 — accepting a re-authored proposal transitions consumed
//! feedback from produced to addressed (FT-107 AC #3).
//!
//! Validates: FT-107 · ADR-024 · ADR-026.

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
use decision_cli::verify_graph_generate::{
    self,
    proposal::{GraphProposal, NewProposal, ProposedStep},
    worker::install_mock,
    GenerateMode, GenerateRequest,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use serde_json::json;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

const FB_IRI: &str = "urn:dec:feedback:tc-185:fb-1";

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc185-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-185 fixture\n");
    body.push_str("phase: 3\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-185.\n");
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
        evidence: "TC-185: existing graph produced a failure".to_string(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/activity/tc-185/seed",
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
    let dump = orchestration_dump_path(workdir);
    let body = fs::read_to_string(&dump).expect("read dump");
    let mut state = String::from("(unknown)");
    let mut addr: Option<String> = None;
    for line in body.lines() {
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
            // <s> <p> <o> <g> .
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                addr = Some(parts[2].trim_matches(|c| c == '<' || c == '>').to_string());
            }
        }
    }
    (state, addr)
}

#[test]
fn tc_185_accept_transitions_consumed_feedback() {
    let wd = WorkdirGuard::new("accept");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-T3";
    let tcs = ["TC-T3a"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    // Seed an existing graph that completely covers FT-T3.
    let _ = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-185-existing".to_string()),
        verifies: feature_id.to_string(),
        environment: "ENV-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed graph");
    let mut fields = BTreeMap::new();
    fields.insert("command".to_string(), "echo broken".to_string());
    fields.insert("expect-exit-code".to_string(), "0".to_string());
    let _ = verify_step_add::run(&StepAddRequest {
        graph_id: "VG-185-existing".to_string(),
        step_type: "shell-command".to_string(),
        fields,
        provides_evidence_for: vec!["TC-T3a".to_string()],
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed step");

    // Pre-seed a defect feedback so the matcher gate falls through to
    // the worker (the FT-107 bypass).
    let tc_iri = "https://decision-cli.dev/ns/tc/TC-T3a";
    seed_produced_defect(wd.path(), FB_IRI, tc_iri);

    // Sanity: feedback starts in `produced`.
    let (state_before, addr_before) = read_feedback_lifecycle(wd.path(), FB_IRI);
    assert_eq!(state_before, "produced");
    assert!(addr_before.is_none());

    // Install a mock worker that returns a `New` proposal citing FB.
    let fb_iri_owned = FB_IRI.to_string();
    let _guard = install_mock(move |bundle| {
        // The bundle MUST carry the defect feedback (otherwise the
        // matcher would have short-circuited).
        assert!(
            !bundle.defect_feedback.is_empty(),
            "TC-185 precondition: bundle must carry defect feedback"
        );
        let mut fields = serde_json::Map::new();
        fields.insert("command".to_string(), json!("echo fixed"));
        fields.insert("expect-exit-code".to_string(), json!("0"));
        Ok(GraphProposal::new_new(
            bundle.bundle_hash.clone(),
            NewProposal {
                environment: "ENV-001-ephemeral-cli".to_string(),
                steps: vec![ProposedStep {
                    step_type: "shell-command".to_string(),
                    fields,
                    provides_evidence_for: vec!["TC-T3a".to_string()],
                }],
                rationale: "TC-185 mock: re-author addressing FB-1".to_string(),
                addressed_feedback_iris: vec![fb_iri_owned.clone()],
            },
        ))
    });

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "ENV-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Accept,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let outcome = verify_graph_generate::run_generate(&req).expect("generate accept");
    let persisted = outcome.persisted.as_ref().expect("persisted on Accept");
    let new_graph_id = persisted.graph_id.clone();
    assert!(
        new_graph_id.starts_with("VG-") && new_graph_id != "VG-185-existing",
        "a fresh graph must be minted (got {new_graph_id})"
    );

    // AC: the cited feedback transitions to `addressed` and gains
    // `dec:addressingArtifact = <new graph iri>`.
    let (state_after, addr_after) = read_feedback_lifecycle(wd.path(), FB_IRI);
    assert_eq!(
        state_after, "addressed",
        "feedback must transition to addressed; got {state_after}"
    );
    let expected_addr = format!("https://decision-cli.dev/ns/graph/{new_graph_id}");
    assert_eq!(
        addr_after.as_deref(),
        Some(expected_addr.as_str()),
        "addressing_artifact must point at the new graph; got {addr_after:?}"
    );
}
