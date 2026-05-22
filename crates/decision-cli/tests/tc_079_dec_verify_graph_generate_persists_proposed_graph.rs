//! TC-079 — `dec verify graph generate ... --accept` persists through
//! the slice-2.5 writers.
//!
//! Validates: FT-049 · ADR-030.
//! Spec: `.product/tests/TC-079-dec-verify-graph-generate-persists-proposed-graph.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::{
    self,
    persist::{reset_writer_call_log, writer_call_log},
    proposal::{GraphProposal, NewProposal, ProposedStep},
    worker::{install_mock, reset_subprocess_invocation_count, subprocess_invocation_count},
    GenerateMode, GenerateRequest,
};
use serde_json::json;

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
            "decision-cli-tc079-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-079 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-079.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

fn build_three_step_proposal(bundle_hash: &str, tcs: &[&str]) -> GraphProposal {
    let steps: Vec<ProposedStep> = tcs
        .iter()
        .enumerate()
        .map(|(i, t)| ProposedStep {
            step_type: "shell-command".to_string(),
            fields: {
                let mut m = serde_json::Map::new();
                m.insert(
                    "command".to_string(),
                    json!(format!("echo \"step {i} for {t}\"")),
                );
                m.insert("expect-exit-code".to_string(), json!("0"));
                m
            },
            provides_evidence_for: vec![(*t).to_string()],
        })
        .collect();
    GraphProposal::new_new(
        bundle_hash,
        NewProposal {
            environment: "ENV-001-ephemeral-cli".to_string(),
            steps,
            rationale: "TC-079 mock: each step covers one TC via providesEvidenceFor".to_string(),
        },
    )
}

#[test]
fn tc_079_dec_verify_graph_generate_persists_proposed_graph() {
    let wd = WorkdirGuard::new("persist");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-Pmock";
    let tcs = ["TC-Pa", "TC-Pb", "TC-Pc"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    reset_writer_call_log();
    reset_subprocess_invocation_count();

    // Install mock worker that returns a `New` proposal with 3 steps.
    let tcs_owned: Vec<String> = tcs.iter().map(|s| (*s).to_string()).collect();
    let _guard = install_mock(move |bundle| {
        Ok(build_three_step_proposal(
            &bundle.bundle_hash,
            &tcs_owned.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
    });

    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "ENV-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Accept,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };

    let outcome = verify_graph_generate::run_generate(&req).expect("generate ok");

    // AC: handler returns persisted graph_id, path, coverage_report.
    let persisted = outcome.persisted.as_ref().expect("persisted set");
    assert!(
        persisted.graph_id.starts_with("VG-"),
        "graph id should be VG-NNN, got {}",
        persisted.graph_id
    );

    // AC: VG-NNN.ttl exists at .dec/verify/graph/.
    assert!(
        persisted.graph_path.is_file(),
        "graph file must exist on disk: {}",
        persisted.graph_path.display()
    );

    // AC: each step has dec:providesEvidenceFor for the TC ids.
    let body = fs::read_to_string(&persisted.graph_path).expect("read ttl");
    for tc in &tcs {
        let tc_iri = format!("https://decision-cli.dev/ns/tc/{tc}");
        assert!(
            body.contains(&tc_iri),
            "graph .ttl missing providesEvidenceFor IRI for {tc}: body = {body}"
        );
    }

    // AC: writer instrumentation — graph_new called once, step_add called 3x.
    let log = writer_call_log();
    assert_eq!(
        log.graph_new_calls, 1,
        "graph_new should be called exactly once, got {}",
        log.graph_new_calls
    );
    assert_eq!(
        log.step_add_calls, 3,
        "step_add should be called 3 times (one per step), got {}",
        log.step_add_calls
    );

    // The subprocess hook MUST have been bypassed via the mock; the
    // real Python subprocess was not invoked.
    assert_eq!(
        subprocess_invocation_count(),
        0,
        "real worker subprocess should NOT have been spawned"
    );
}

#[test]
fn tc_079_handler_error_on_unknown_feature() {
    let wd = WorkdirGuard::new("unknown-feature");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let req = GenerateRequest {
        feature_id: "FT-DoesNotExist".to_string(),
        environment_id: "ENV-001-ephemeral-cli".to_string(),
        mode: GenerateMode::Accept,
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let err = verify_graph_generate::run_generate(&req).expect_err("must fail");
    assert!(
        matches!(err, HandlerError::ArtifactNotFound { .. }),
        "expected ArtifactNotFound, got {err:?}"
    );
}
