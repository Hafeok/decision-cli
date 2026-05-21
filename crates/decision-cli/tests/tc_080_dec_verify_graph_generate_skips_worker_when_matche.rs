//! TC-080 — `dec verify graph generate` skips worker when matcher reports complete match.
//!
//! Validates: FT-049 · ADR-030.
//! Spec: `.product/tests/TC-080-dec-verify-graph-generate-skips-worker-when-matche.md`

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

const STREAM_TTL: &str = include_str!(
    "../src/core/bundled/assets/streams/engineering-development.ttl"
);

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc080-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-080 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-080.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body)
        .expect("write feature fixture");
}

#[test]
fn tc_080_dec_verify_graph_generate_skips_worker_when_matche() {
    let wd = WorkdirGuard::new("matcher");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-Omock";
    let tcs = ["TC-Oa", "TC-Ob"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    // Seed an existing VG that covers all of FT-O's TCs in ENV-001-ephemeral-cli.
    let graph_req = GraphNewRequest {
        id: Some("VG-099-existing".to_string()),
        verifies: feature_id.to_string(),
        environment: "ENV-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    };
    let _ = verify_graph_new::run(&graph_req).expect("seed graph");

    for tc in &tcs {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("command".to_string(), format!("echo {tc}"));
        fields.insert("expect-exit-code".to_string(), "0".to_string());
        let step_req = StepAddRequest {
            graph_id: "VG-099-existing".to_string(),
            step_type: "shell-command".to_string(),
            fields,
            provides_evidence_for: vec![(*tc).to_string()],
            workdir: Some(wd.path().to_path_buf()),
        };
        let _ = verify_step_add::run(&step_req).expect("seed step");
    }

    // Reset the subprocess counter to 0 so we can assert the worker
    // is NOT spawned during the generate call.
    reset_subprocess_invocation_count();
    // Install a panicking mock — if the worker were invoked, this
    // mock would fire and the test would fail loudly.
    let _guard = install_mock(|_bundle| {
        panic!("TC-080: worker MUST NOT be invoked when a complete match exists")
    });

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "ENV-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Interactive,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let outcome = verify_graph_generate::run_generate(&req).expect("generate ok");

    // AC: handler returns GraphProposal::Match { graph_id: "VG-099-existing" }.
    assert_eq!(outcome.proposal.kind, ProposalKind::Match);
    let m = outcome
        .proposal
        .match_payload
        .as_ref()
        .expect("match payload");
    assert_eq!(m.graph_id, "VG-099-existing");

    // AC: worker subprocess NOT spawned (and the mock never panicked,
    // which is implicit in reaching this line).
    assert_eq!(
        subprocess_invocation_count(),
        0,
        "TC-080: worker subprocess must NOT be spawned on a complete match"
    );

    // AC: persisted None — nothing new was written.
    assert!(
        outcome.persisted.is_none(),
        "no persistence should happen on Match"
    );

    // AC: no new .ttl was written — only VG-EX exists.
    let graph_dir = wd.path().join(".dec/verify/graph");
    let entries: Vec<_> = fs::read_dir(&graph_dir)
        .expect("read graph dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().into_string().unwrap_or_default())
        .filter(|n| n.ends_with(".ttl"))
        .collect();
    assert_eq!(
        entries,
        vec!["VG-099-existing.ttl".to_string()],
        "only the seeded graph should exist; got {entries:?}"
    );
}
