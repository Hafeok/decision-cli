//! TC-184 — matcher short-circuit still fires when no defect feedback
//! exists for the (feature, env) pair (FT-107 AC #2).
//!
//! Validates: FT-107 · ADR-030.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::{
    self,
    proposal::ProposalKind,
    worker::{install_mock, reset_subprocess_invocation_count, subprocess_invocation_count},
    GenerateMode, GenerateRequest,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};

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
            "decision-cli-tc184-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-184 fixture\n");
    body.push_str("phase: 3\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-184.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

#[test]
fn tc_184_matcher_short_circuit_still_fires() {
    let wd = WorkdirGuard::new("nofeedback");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-T2";
    let tcs = ["TC-T2a"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    // Seed a graph that completely covers FT-T2's TCs in the ephemeral env.
    let _ = verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-184".to_string()),
        verifies: feature_id.to_string(),
        environment: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    })
    .expect("seed graph");
    for tc in &tcs {
        let mut fields = BTreeMap::new();
        fields.insert("command".to_string(), format!("echo {tc}"));
        fields.insert("expect-exit-code".to_string(), "0".to_string());
        let _ = verify_step_add::run(&StepAddRequest {
            graph_id: "VG-184".to_string(),
            step_type: "shell-command".to_string(),
            fields,
            provides_evidence_for: vec![(*tc).to_string()],
            workdir: Some(wd.path().to_path_buf()),
        })
        .expect("seed step");
    }

    // Panicking mock: if the worker were dispatched, this fires.
    reset_subprocess_invocation_count();
    let _guard = install_mock(|_bundle| {
        panic!("TC-184: worker MUST NOT be invoked when complete match + no defect feedback")
    });

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "BNCH-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Interactive,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let outcome = verify_graph_generate::run_generate(&req).expect("generate ok");

    // AC #1: the proposal is a `Match` referencing the existing graph.
    assert_eq!(outcome.proposal.kind, ProposalKind::Match);
    let m = outcome
        .proposal
        .match_payload
        .as_ref()
        .expect("match payload");
    assert_eq!(m.graph_id, "VG-184");

    // AC #2: worker subprocess NOT spawned.
    assert_eq!(
        subprocess_invocation_count(),
        0,
        "TC-184: worker subprocess must NOT be spawned on a clean match path"
    );

    // No persistence (matches TC-080 expectation).
    assert!(outcome.persisted.is_none(), "no persistence on match");
}
