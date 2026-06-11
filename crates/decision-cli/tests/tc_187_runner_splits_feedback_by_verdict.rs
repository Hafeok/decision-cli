//! TC-187 — runner splits defect feedback by verdict: `rejected` →
//! `targetRole = "implementer"`, `amendment-required` → `"verifier"`.
//!
//! Validates: FT-108 · ADR-026.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::core::verify::runner::{run_graph, RunGraphRequest, TriggerKind};
use decision_cli::verify_bench_new::{self, BenchNewRequest};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_step_add::{self, StepAddRequest};
use oxigraph::model::NamedNode;

struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new(tag: &str) -> Self {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("dec-tc187-{tag}-{pid}-{nonce}"));
        std::fs::create_dir_all(&p).expect("create tmp");
        Self { path: p }
    }
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn dec_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dec"))
}

fn write_tc_fixtures(dir: &Path, ids: &[&str]) {
    let tests = dir.join(".product/tests");
    std::fs::create_dir_all(&tests).expect("tests");
    for id in ids {
        let body = format!(
            "---\nid: {id}\ntitle: TC-187 fixture\ntype: scenario\nstatus: unimplemented\nvalidates:\n  features: []\n  adrs: []\nphase: 1\nrunner: bash\nrunner-args: 'true'\n---\n\nFixture {id}.\n"
        );
        let fname = format!("{}-tc187-fixture.md", id.to_lowercase());
        std::fs::write(tests.join(&fname), body).expect("tc fixture");
    }
}

fn init_workdir(tag: &str) -> TmpDir {
    let tmp = TmpDir::new(tag);
    let streams = tmp.path().join("streams");
    std::fs::create_dir_all(&streams).expect("streams");
    std::fs::write(
        streams.join("decision-cli-development.ttl"),
        "@prefix dec: <https://decision-cli.dev/ns#> .\n\
         @prefix va:  <https://decision-cli.dev/ns/value-actions/> .\n\
         <stream:decision-cli-development> a dec:ValueStream ;\n\
             dec:name                \"decision-cli-development\" ;\n\
             dec:title               \"decision-cli Development\" ;\n\
             dec:description         \"Value stream for shipping decision-cli features.\" ;\n\
             dec:terminalValueAction va:shipped-feature ;\n\
             dec:authorizedGoals     \"ship\" , \"land\" .\n",
    )
    .expect("seed");
    let features = tmp.path().join(".product/features");
    std::fs::create_dir_all(&features).expect("features");
    std::fs::write(
        features.join("FT-001-test-fixture.md"),
        "---\nid: FT-001\ntitle: test fixture feature\nphase: 1\nstatus: planned\n---\n\nFixture.\n",
    )
    .expect("ft");
    let status = Command::new(dec_binary())
        .arg("init")
        .arg("--from")
        .arg(streams.join("decision-cli-development.ttl"))
        .current_dir(tmp.path())
        .status()
        .expect("dec init");
    assert!(
        status.code() == Some(0) || status.code() == Some(2),
        "dec init: {status:?}"
    );
    tmp
}

fn fields_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// Pull the `targetRole` literals for the persisted feedback IRIs out
/// of the orchestration dump on disk.
fn target_roles_for(workdir: &Path, feedback_iris: &[NamedNode]) -> Vec<String> {
    let dump_body =
        std::fs::read_to_string(workdir.join(".dec/store/orchestration.nq")).expect("read dump");
    let mut out: Vec<String> = Vec::new();
    for iri in feedback_iris {
        let needle = format!(
            "<{}> <https://decision-cli.dev/ns#targetRole>",
            iri.as_str()
        );
        for line in dump_body.lines() {
            if line.starts_with(&needle) {
                if let Some(start) = line.find('"') {
                    let rest = &line[start + 1..];
                    if let Some(end) = rest.find('"') {
                        out.push(rest[..end].to_string());
                    }
                }
                break;
            }
        }
    }
    out
}

#[test]
fn tc_187_runner_splits_feedback_by_verdict() {
    // ----- Scenario A: rejected verdict → defects target implementer -----
    let tmp_a = init_workdir("rejected");
    write_tc_fixtures(tmp_a.path(), &["TC-R1"]);
    verify_bench_new::run(&BenchNewRequest {
        id: Some("ENV-T187a".into()),
        bench_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec!["shell".into(), "filesystem".into()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(tmp_a.path().to_path_buf()),
    })
    .expect("env new (A)");
    verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-187a".into()),
        verifies: "FT-001".into(),
        environment: "ENV-T187a".into(),
        workdir: Some(tmp_a.path().to_path_buf()),
    })
    .expect("graph new (A)");
    // Single evidence-bearing step that fails → evidence regression →
    // verdict = rejected.
    verify_step_add::run(&StepAddRequest {
        graph_id: "VG-187a".into(),
        step_type: "shell-command".into(),
        fields: fields_of(&[("command", "exit 1"), ("expect-exit-code", "0")]),
        provides_evidence_for: vec!["TC-R1".into()],
        workdir: Some(tmp_a.path().to_path_buf()),
    })
    .expect("step add (A)");

    let activity_a = NamedNode::new_unchecked("https://decision-cli.dev/ns/activity/tc187/a");
    let resp_a = run_graph(&RunGraphRequest {
        graph: NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/VG-187a"),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: activity_a,
        workdir: tmp_a.path().to_path_buf(),
    })
    .expect("run (A)");
    assert_eq!(
        resp_a.emitted_feedback.len(),
        1,
        "scenario A: exactly one defect feedback expected"
    );
    let roles_a = target_roles_for(tmp_a.path(), &resp_a.emitted_feedback);
    assert_eq!(
        roles_a,
        vec!["implementer".to_string()],
        "scenario A: rejected verdict ⇒ targetRole = implementer; got {roles_a:?}"
    );

    // ----- Scenario B: amendment-required → defects target verifier -----
    let tmp_b = init_workdir("amendment");
    write_tc_fixtures(tmp_b.path(), &["TC-AR1"]);
    verify_bench_new::run(&BenchNewRequest {
        id: Some("ENV-T187b".into()),
        bench_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec!["shell".into(), "filesystem".into()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(tmp_b.path().to_path_buf()),
    })
    .expect("env new (B)");
    verify_graph_new::run(&GraphNewRequest {
        id: Some("VG-187b".into()),
        verifies: "FT-001".into(),
        environment: "ENV-T187b".into(),
        workdir: Some(tmp_b.path().to_path_buf()),
    })
    .expect("graph new (B)");
    // Step 0: non-evidence-bearing shell that fails — counts as a
    // setup-style failure, NOT an evidence regression.
    verify_step_add::run(&StepAddRequest {
        graph_id: "VG-187b".into(),
        step_type: "shell-command".into(),
        fields: fields_of(&[("command", "exit 1"), ("expect-exit-code", "0")]),
        provides_evidence_for: Vec::new(),
        workdir: Some(tmp_b.path().to_path_buf()),
    })
    .expect("step 0 (B)");
    // Step 1: evidence-bearing step that ALSO fails. Verdict aggregator
    // sees a non-evidence fail preceding an evidence fail and classifies
    // the run as `amendment-required` (setup-style).
    verify_step_add::run(&StepAddRequest {
        graph_id: "VG-187b".into(),
        step_type: "shell-command".into(),
        fields: fields_of(&[("command", "exit 1"), ("expect-exit-code", "0")]),
        provides_evidence_for: vec!["TC-AR1".into()],
        workdir: Some(tmp_b.path().to_path_buf()),
    })
    .expect("step 1 (B)");

    let activity_b = NamedNode::new_unchecked("https://decision-cli.dev/ns/activity/tc187/b");
    let resp_b = run_graph(&RunGraphRequest {
        graph: NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/VG-187b"),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: activity_b,
        workdir: tmp_b.path().to_path_buf(),
    })
    .expect("run (B)");
    let roles_b = target_roles_for(tmp_b.path(), &resp_b.emitted_feedback);
    assert!(
        !roles_b.is_empty(),
        "scenario B: expected at least one emitted feedback"
    );
    for r in &roles_b {
        assert_eq!(
            r, "verifier",
            "scenario B: amendment-required verdict ⇒ targetRole = verifier; got {roles_b:?}"
        );
    }
}
