//! TC-157 — stopOnFail truncates execution but every step still has a
//!          trace entry equal to graph.steps length.
//!
//! Spec: `.product/tests/TC-157-stoponfail-truncates-execution-but-every-step-stil.md`
//! Validates: FT-098 · ADR-028.
//!
//! Slice-3 lacks a typed `dec:stopOnFail` predicate, so this slice's
//! runner sniffs a `#dec:stopOnFail` sentinel at the head of a
//! shell-command's body. The TC's semantic — fail-and-skip-remaining
//! while preserving positional trace alignment — is what we validate.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::StepOutcome;
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
        p.push(format!("dec-tc157-{tag}-{pid}-{nonce}"));
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

const ENV_ID: &str = "BNCH-9-tc157";

fn seed_env(workdir: &Path) {
    verify_bench_new::run(&BenchNewRequest {
        id: Some(ENV_ID.into()),
        bench_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec!["shell".into(), "filesystem".into(), "sparql-local".into()],
        setup: None,
        teardown: None,
        endpoint: None,
        fixture_source: None,
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("env new");
}

fn seed_graph(workdir: &Path, vg_id: &str) {
    verify_graph_new::run(&GraphNewRequest {
        id: Some(vg_id.into()),
        verifies: "FT-001".into(),
        environment: ENV_ID.into(),
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("graph new");
}

fn add_step(workdir: &Path, vg_id: &str, kind: &str, fields: BTreeMap<String, String>) {
    verify_step_add::run(&StepAddRequest {
        graph_id: vg_id.into(),
        step_type: kind.into(),
        fields,
        provides_evidence_for: Vec::new(),
        workdir: Some(workdir.to_path_buf()),
    })
    .expect("step add");
}

#[test]
fn tc_157_stoponfail_truncates_execution_but_every_step_stil() {
    stop_on_fail_path();
    no_stop_on_fail_path();
}

fn stop_on_fail_path() {
    let tmp = init_workdir("stop");
    seed_env(tmp.path());
    let vg = "VG-1-stop";
    seed_graph(tmp.path(), vg);
    // Step 0: `echo ok` (pass).
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[("command", "echo ok"), ("expect-exit-code", "0")]),
    );
    // Step 1: stop-on-fail sentinel followed by `exit 1`. The sentinel
    // is a comment Bash ignores; the runner picks it up before spawning.
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[
            ("command", "#dec:stopOnFail\nexit 1"),
            ("expect-exit-code", "0"),
        ]),
    );
    // Step 2: `echo never` (would pass, but is skipped).
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[("command", "echo never"), ("expect-exit-code", "0")]),
    );
    // Step 3: sparql over store.ttl (would be unrunnable since step 0
    // didn't create it, but is also skipped).
    add_step(
        tmp.path(),
        vg,
        "sparql-assertion",
        fields_of(&[
            ("target", "store.ttl"),
            ("query", "SELECT ?s WHERE { ?s ?p ?o }"),
            ("expect-rows", "1"),
        ]),
    );

    let response = run_graph(&RunGraphRequest {
        graph: NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/graph/{vg}")),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/activity/tc157-stop/{}",
            std::process::id()
        )),
        workdir: tmp.path().to_path_buf(),
    })
    .expect("run");
    assert!(
        matches!(
            response.verdict,
            Verdict::Rejected | Verdict::AmendmentRequired
        ),
        "verdict must reflect the failure on step 1, got {:?}",
        response.verdict
    );
    assert_eq!(response.step_outcomes.len(), 4, "trace count");
    assert_eq!(response.step_outcomes[0].outcome, StepOutcome::Pass);
    assert_eq!(response.step_outcomes[1].outcome, StepOutcome::Fail);
    assert_eq!(
        response.step_outcomes[2].outcome,
        StepOutcome::Unrunnable,
        "step 2 skipped"
    );
    assert_eq!(
        response.step_outcomes[3].outcome,
        StepOutcome::Unrunnable,
        "step 3 skipped"
    );

    // Read the result Turtle to confirm error messages name the
    // halting step and `never` does not appear in any captured stdout.
    let result_dir = tmp.path().join(".dec/verify/result");
    let files: Vec<_> = std::fs::read_dir(&result_dir)
        .expect("result dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "ttl"))
        .collect();
    assert_eq!(files.len(), 1, "exactly one VGR.ttl");
    let body = std::fs::read_to_string(&files[0]).expect("read");
    assert!(
        body.contains("skipped: prior step 1 halted the run"),
        "skipped diagnostic must name halting step:\n{body}"
    );
    assert!(
        !body.contains("\"never\""),
        "step 2 must not have produced 'never' output:\n{body}"
    );
}

fn no_stop_on_fail_path() {
    let tmp = init_workdir("nostop");
    seed_env(tmp.path());
    let vg = "VG-1-nostop";
    seed_graph(tmp.path(), vg);
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[("command", "echo ok"), ("expect-exit-code", "0")]),
    );
    // No stop-on-fail sentinel — failure but loop continues.
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[("command", "exit 1"), ("expect-exit-code", "0")]),
    );
    add_step(
        tmp.path(),
        vg,
        "shell-command",
        fields_of(&[("command", "echo continued"), ("expect-exit-code", "0")]),
    );
    add_step(
        tmp.path(),
        vg,
        "sparql-assertion",
        fields_of(&[
            ("target", "store.ttl"),
            ("query", "SELECT ?s WHERE { ?s ?p ?o }"),
            ("expect-rows", "1"),
        ]),
    );

    let response = run_graph(&RunGraphRequest {
        graph: NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/graph/{vg}")),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/activity/tc157-nostop/{}",
            std::process::id()
        )),
        workdir: tmp.path().to_path_buf(),
    })
    .expect("run");
    assert_eq!(response.step_outcomes.len(), 4);
    assert_eq!(response.step_outcomes[0].outcome, StepOutcome::Pass);
    assert_eq!(response.step_outcomes[1].outcome, StepOutcome::Fail);
    assert_eq!(
        response.step_outcomes[2].outcome,
        StepOutcome::Pass,
        "step 2 must run when stop-on-fail is absent"
    );
    assert_eq!(
        response.step_outcomes[3].outcome,
        StepOutcome::Unrunnable,
        "step 3 is unrunnable because store.ttl was never created"
    );
}
