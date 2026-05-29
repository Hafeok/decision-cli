//! TC-082 — MCP `dec_verify_graph_accept` refuses stale proposals after
//! the candidate set changes between generate and accept.
//!
//! Validates: FT-049 · ADR-030.
//! Spec: `.product/tests/TC-082-mcp-generate-then-accept-refuses-stale-proposals-a.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::{
    self,
    proposal::{GraphProposal, NewProposal, ProposalKind, ProposedStep},
    worker::install_mock,
    AcceptRequest, GenerateMode, GenerateRequest,
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
            "decision-cli-tc082-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-082 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-082.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write feature fixture");
}

fn build_new_proposal(bundle_hash: &str, tcs: &[&str]) -> GraphProposal {
    let steps: Vec<ProposedStep> = tcs
        .iter()
        .map(|t| ProposedStep {
            step_type: "shell-command".to_string(),
            fields: {
                let mut m = serde_json::Map::new();
                m.insert(
                    "command".to_string(),
                    serde_json::Value::String(format!("echo {t}")),
                );
                m
            },
            provides_evidence_for: vec![(*t).to_string()],
        })
        .collect();
    GraphProposal::new_new(
        bundle_hash,
        NewProposal {
            environment: "BNCH-001-ephemeral-cli".to_string(),
            steps,
            rationale: "TC-082 mock proposal".to_string(),
            addressed_feedback_iris: Vec::new(),
        },
    )
}

#[test]
fn tc_082_mcp_generate_then_accept_refuses_stale_proposals_a() {
    let wd = WorkdirGuard::new("stale");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let feature_id = "FT-Mmock";
    let tcs = ["TC-Ma", "TC-Mb"];
    write_feature_fixture(wd.path(), feature_id, &tcs);

    // 1. Client A calls generate → receives proposal_token T1.
    let tcs_owned: Vec<String> = tcs.iter().map(|s| (*s).to_string()).collect();
    let guard_gen = install_mock(move |bundle| {
        Ok(build_new_proposal(
            &bundle.bundle_hash,
            &tcs_owned.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
    });
    let gen_req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: "BNCH-001-ephemeral-cli".to_string(),
        mode: GenerateMode::PrintOnly, // do not auto-persist
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let outcome = verify_graph_generate::run_generate(&gen_req).expect("generate ok");
    assert_eq!(outcome.proposal.kind, ProposalKind::New);
    let proposal_t1 = outcome.proposal.clone();
    let token_t1 = outcome.proposal_token.clone();
    drop(guard_gen);

    // 2. Client B writes a covering graph for the same (feature, env)
    //    between generate and accept.
    let other_graph = GraphNewRequest {
        id: Some("VG-150-concurrent".to_string()),
        verifies: feature_id.to_string(),
        environment: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
    };
    let _ = verify_graph_new::run(&other_graph).expect("seed concurrent graph");
    for tc in &tcs {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("command".to_string(), format!("echo {tc}"));
        let step_req = StepAddRequest {
            graph_id: "VG-150-concurrent".to_string(),
            step_type: "shell-command".to_string(),
            fields,
            provides_evidence_for: vec![(*tc).to_string()],
            workdir: Some(wd.path().to_path_buf()),
        };
        let _ = verify_step_add::run(&step_req).expect("seed step");
    }

    // 3. Client A calls accept with the now-stale proposal + token.
    let accept_req = AcceptRequest {
        proposal: proposal_t1,
        proposal_token: token_t1,
        feature_id: feature_id.to_string(),
        environment_id: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };

    let err = verify_graph_generate::run_accept(&accept_req).expect_err("accept must refuse");

    // AC: Error::ProposalStale.
    match err {
        HandlerError::ProposalStale { detail } => {
            // AC: message suggests re-running generate.
            assert!(
                detail.contains("dec_verify_graph_generate") || detail.contains("re-run"),
                "ProposalStale detail should suggest re-running generate; got {detail}"
            );
        }
        other => panic!("expected ProposalStale, got {other:?}"),
    }

    // AC: no new graph was written.
    let graph_dir = wd.path().join(".dec/verify/graph");
    let entries: Vec<_> = fs::read_dir(&graph_dir)
        .expect("read graph dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().into_string().unwrap_or_default())
        .filter(|n| n.ends_with(".ttl"))
        .collect();
    assert_eq!(
        entries,
        vec!["VG-150-concurrent.ttl".to_string()],
        "only the concurrent graph should exist; got {entries:?}"
    );
}

#[test]
fn tc_082_accept_refuses_token_mismatch() {
    let wd = WorkdirGuard::new("token-mismatch");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");
    write_feature_fixture(wd.path(), "FT-MTok", &["TC-Ma"]);

    let proposal = build_new_proposal("aabbccddeeff0011", &["TC-Ma"]);
    let accept_req = AcceptRequest {
        proposal,
        proposal_token: "DEADBEEFDEADBEEF".to_string(),
        feature_id: "FT-MTok".to_string(),
        environment_id: "BNCH-001-ephemeral-cli".to_string(),
        workdir: Some(wd.path().to_path_buf()),
        product_root: Some(wd.path().to_path_buf()),
    };
    let err = verify_graph_generate::run_accept(&accept_req).expect_err("token mismatch");
    assert!(
        matches!(err, HandlerError::ProposalStale { .. }),
        "expected ProposalStale on token mismatch; got {err:?}"
    );
}
