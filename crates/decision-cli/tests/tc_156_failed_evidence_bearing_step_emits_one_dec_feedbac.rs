//! TC-156 — Failed evidence-bearing step emits one dec:Feedback per
//!          linked TC with the correct class.
//!
//! Spec: `.product/tests/TC-156-failed-evidence-bearing-step-emits-one-dec-feedbac.md`
//! Validates: FT-098 · ADR-022 · FT-031.
//!
//! Note: ADR-023 enumerates `dec:feedbackClass` as one of
//! `{gap, contradiction, unimplementable, scope-issue, defect,
//! capability-request}`. The TC-156 spec text uses "regression" as the
//! conceptual class for a failing evidence-bearing step; the closest
//! controlled-vocabulary entry is `defect` ("action self-discovered
//! error post-production"). The implementation maps the conceptual
//! "regression" → `defect`.

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
        p.push(format!("dec-tc156-{tag}-{pid}-{nonce}"));
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

fn write_product_tc_fixtures(dir: &Path) {
    let tests = dir.join(".product/tests");
    std::fs::create_dir_all(&tests).expect("tests");
    for id in ["TC-EVI-A", "TC-EVI-B", "TC-EVI-C"] {
        let body = format!(
            "---\nid: {id}\ntitle: evidence fixture\ntype: scenario\nstatus: unimplemented\nvalidates:\n  features: []\n  adrs: []\nphase: 1\nrunner: bash\nrunner-args: 'true'\n---\n\nFixture {id}.\n"
        );
        let fname = format!("{}-evidence-fixture.md", id.to_lowercase());
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
    write_product_tc_fixtures(tmp.path());
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

const ENV_ID: &str = "BNCH-9-tc156";
const VG_ID: &str = "VG-9-tc156";

#[test]
fn tc_156_failed_evidence_bearing_step_emits_one_dec_feedbac() {
    let tmp = init_workdir("fb");
    verify_bench_new::run(&BenchNewRequest {
        id: Some(ENV_ID.into()),
        bench_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec!["shell".into(), "filesystem".into(), "sparql-local".into()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("env new");
    verify_graph_new::run(&GraphNewRequest {
        id: Some(VG_ID.into()),
        verifies: "FT-001".into(),
        environment: ENV_ID.into(),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("graph new");

    // Step 0: shell-command `exit 1` with providesEvidenceFor [TC-EVI-A, TC-EVI-B].
    verify_step_add::run(&StepAddRequest {
        graph_id: VG_ID.into(),
        step_type: "shell-command".into(),
        fields: fields_of(&[("command", "exit 1"), ("expect-exit-code", "0")]),
        provides_evidence_for: vec!["TC-EVI-A".into(), "TC-EVI-B".into()],
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("step 0");
    // Step 1: sparql-assertion over missing target, providesEvidenceFor [TC-EVI-C].
    verify_step_add::run(&StepAddRequest {
        graph_id: VG_ID.into(),
        step_type: "sparql-assertion".into(),
        fields: fields_of(&[
            ("target", "missing.ttl"),
            ("query", "SELECT ?s WHERE { ?s ?p ?o }"),
            ("expect-rows", "1"),
        ]),
        provides_evidence_for: vec!["TC-EVI-C".into()],
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("step 1");

    let activity = NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/activity/tc156/{}",
        std::process::id()
    ));
    let response = run_graph(&RunGraphRequest {
        graph: NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/graph/{VG_ID}")),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: activity.clone(),
        workdir: tmp.path().to_path_buf(),
    })
    .expect("run");
    // Three feedbacks: 2 from step 0 (fail → defect for TC-A, TC-B),
    // 1 from step 1 (unrunnable → gap for TC-C).
    assert_eq!(
        response.emitted_feedback.len(),
        3,
        "expected 3 emitted feedbacks (2 defect + 1 gap), got {:?}",
        response.emitted_feedback
    );

    // Inspect the persisted store dump to check each feedback's class +
    // target.
    let dump = tmp.path().join(".dec/store/orchestration.nq");
    let body = std::fs::read_to_string(&dump).expect("read dump");
    let defect_count = body.matches("\"defect\"").count();
    let gap_count = body.matches("\"gap\"").count();
    assert!(
        defect_count >= 2,
        "at least 2 defect-class feedbacks in dump"
    );
    assert!(gap_count >= 1, "at least 1 gap-class feedback in dump");
    assert!(
        body.contains("TC-EVI-A") && body.contains("TC-EVI-B") && body.contains("TC-EVI-C"),
        "every linked TC must appear as source_artifact in dump"
    );
    // Bodies contain the runner's expected-vs-actual line for shell
    // failures and a target-missing line for sparql.
    assert!(
        body.contains("expected exit 0, got 1"),
        "shell failure body must contain expected-vs-actual"
    );
    assert!(
        body.contains("target missing") || body.contains("could not load target"),
        "sparql unrunnable body must explain the cause"
    );
}
