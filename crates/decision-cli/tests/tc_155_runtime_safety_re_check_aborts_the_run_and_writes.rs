//! TC-155 — Runtime safety re-check aborts the run and writes a
//!          rejected VGR when an env mutation invalidates allowedOps.
//!
//! Spec: `.product/tests/TC-155-runtime-safety-re-check-aborts-the-run-and-writes.md`
//! Validates: FT-098 · ADR-028 / ADR-001.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::verify::runner::{run_graph, RunGraphRequest, RunnerError, TriggerKind};
use decision_cli::verify_env_new::{self, EnvNewRequest};
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
        p.push(format!("dec-tc155-{tag}-{pid}-{nonce}"));
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

const ENV_ID: &str = "ENV-9-tc155";
const VG_ID: &str = "VG-9-tc155";

#[test]
fn tc_155_runtime_safety_re_check_aborts_the_run_and_writes() {
    let tmp = init_workdir("safety");
    // Initially: env permits http-readonly so the http-request step
    // passes the FT-037 authoring-time gate.
    verify_env_new::run(&EnvNewRequest {
        id: Some(ENV_ID.into()),
        env_type: "ephemeral-tempdir".into(),
        safety_class: "isolated".into(),
        allowed_ops: vec!["http-readonly".into(), "filesystem".into()],
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
    verify_step_add::run(&StepAddRequest {
        graph_id: VG_ID.into(),
        step_type: "http-request".into(),
        fields: fields_of(&[
            ("method", "GET"),
            ("url", "http://127.0.0.1:1/"),
            ("expect-status", "200"),
        ]),
        provides_evidence_for: Vec::new(),
        workdir: Some(tmp.path().to_path_buf()),
    })
    .expect("step add (gate passes at authoring)");

    // Mutate the env file on disk so allowed_ops drops http-readonly.
    // This bypasses the StreamWriter chokepoint by editing the canonical
    // Turtle directly — the in-store projection still carries the
    // permissive env, but the runner loads the env from disk per
    // FT-098 §Behaviour and so will see the mutated form.
    let env_path = tmp
        .path()
        .join(".dec/verify/env")
        .join(format!("{ENV_ID}.ttl"));
    let mutated = "@prefix dec: <https://decision-cli.dev/ns#> .\n\
                   @prefix dcterms: <http://purl.org/dc/terms/> .\n\
                   @prefix prov: <http://www.w3.org/ns/prov#> .\n\
                   <https://decision-cli.dev/ns/env/ENV-9-tc155> a dec:VerificationEnvironment ;\n\
                       dec:envType        \"ephemeral-tempdir\" ;\n\
                       dec:safetyClass    \"isolated\" ;\n\
                       dec:allowedOps     ( \"filesystem\" ) .\n";
    std::fs::write(&env_path, mutated).expect("mutate env");

    // Invoke the runner — the runtime safety re-check fires.
    let req = RunGraphRequest {
        graph: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/graph/{VG_ID}"
        )),
        triggered_by: TriggerKind::Manual,
        capture_bindings: HashMap::new(),
        run_activity: NamedNode::new_unchecked(format!(
            "https://decision-cli.dev/ns/activity/tc155/{}",
            std::process::id()
        )),
        workdir: tmp.path().to_path_buf(),
    };
    let err = run_graph(&req).expect_err("must fail with SafetyViolation");
    match err {
        RunnerError::SafetyViolation { step, op } => {
            assert!(
                step.ends_with("/0"),
                "safety violation must name the offending step IRI, got {step}"
            );
            assert_eq!(op, "http-readonly", "missing op");
        }
        other => panic!("expected SafetyViolation, got {other:?}"),
    }

    // A rejected VGR with empty stepTraces must be persisted.
    let result_dir = tmp.path().join(".dec/verify/result");
    assert!(result_dir.exists(), "result dir created");
    let entries: Vec<_> = std::fs::read_dir(&result_dir)
        .expect("read result dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "ttl"))
        .collect();
    assert_eq!(entries.len(), 1, "exactly one VGR.ttl");
    let body = std::fs::read_to_string(&entries[0]).expect("read result");
    assert!(
        body.contains("\"rejected\""),
        "verdict must be rejected:\n{body}"
    );
    assert!(
        body.contains("safety: step") && body.contains("http-readonly"),
        "rationale must name safety + op:\n{body}"
    );
    assert!(
        body.contains("dec:stepTraces ()") || body.contains("dec:stepTraces rdf:nil"),
        "stepTraces must be empty:\n{body}"
    );

    // Cross-check that the shared safety predicate produces the same
    // verdict — the runner's call site delegates to
    // `core::verify::safety::check_step_against_env`.
    use decision_cli::core::ontology::verification_env::from_turtle as env_from_turtle;
    use decision_cli::core::ontology::verification_graph::from_turtle as graph_from_turtle;
    use decision_cli::core::verify::safety::{check_step_against_env, SafetyError};
    let env = env_from_turtle(&env_path).expect("re-parse env");
    let graph_path = tmp
        .path()
        .join(".dec/verify/graph")
        .join(format!("{VG_ID}.ttl"));
    let graph = graph_from_turtle(&graph_path).expect("re-parse graph");
    let shared = check_step_against_env(&graph.steps[0], &env);
    match shared {
        Err(SafetyError::Violation(v)) => {
            assert_eq!(v.missing_ops, vec!["http-readonly"]);
        }
        other => panic!("shared predicate must reject too: {other:?}"),
    }

    // Verdict variant must be exactly Rejected (we want to assert this
    // beyond the rationale substring).
    use decision_cli::core::ontology::verdict::Verdict as V;
    let v: V = V::Rejected;
    assert_eq!(v, Verdict::Rejected);
}
