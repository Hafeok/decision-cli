//! TC-373 / FT-139 — Integration test for cluster_dispatch::run.
//!
//! Verifies the substrate slice's end-to-end behaviour: a tempdir
//! workdir containing the add-judge-worker audit script + a positive
//! fixture drives through cluster_dispatch::run cleanly. Mirrors the
//! TC-374 positive fixture inline so the test is self-contained.

use std::fs;
use std::path::Path;

use decision_cli::core::drive::PlanContext;
use decision_cli::features::drive::cluster_dispatch;

fn write_fixture(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("capability_binding.nq"),
        r#"<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#endpoint> "scaleway" <https://decision-cli.dev/ns/graph/capability> .
<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#model_identifier> "qwen3-coder-30b-a3b-instruct" <https://decision-cli.dev/ns/graph/capability> .
"#,
    )
    .unwrap();
    fs::write(
        dir.join("pydantic_io_models.py"),
        r#"from pydantic import BaseModel

class JudgeInput(BaseModel):
    feature_id: str
    proposed_artifact: str

class JudgeOutput(BaseModel):
    verdict: str
    reasoning: str
"#,
    )
    .unwrap();
    fs::write(
        dir.join("system_prompt.md"),
        "You are the example judge. Evaluate {{feature_id}} against {{proposed_artifact}}.\n",
    )
    .unwrap();
    fs::write(
        dir.join("agent_loop.py"),
        r#"import litellm

def loop(payload, system_prompt, litellm_key, litellm_base_url):
    return litellm.completion(
        model=payload.model_id,
        messages=[
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"{payload.feature_id} {payload.proposed_artifact}"},
        ],
        api_key=litellm_key,
        base_url=litellm_base_url,
    )
"#,
    )
    .unwrap();
    fs::write(
        dir.join("unit_tests.py"),
        r#"from .models import JudgeInput, JudgeOutput

def test_fixture_input():
    payload = JudgeInput(feature_id="FT-T373", proposed_artifact="example")
    assert payload.feature_id == "FT-T373"
"#,
    )
    .unwrap();
}

/// TC-373 — cluster_dispatch::run completes cleanly against a fixture
/// where the audit script is present and the cells satisfy every
/// coherence check.
#[test]
fn cluster_dispatch_add_judge_worker_end_to_end() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workdir = temp.path().to_path_buf();

    // Mirror the audit script into the tempdir at the relative path
    // declared by the TaskType's CoherenceAuditSpec.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let audit_src = repo_root.join("scripts/checks/cluster-audit-add-judge-worker.py");
    let audit_dst = workdir.join("scripts/checks/cluster-audit-add-judge-worker.py");
    fs::create_dir_all(audit_dst.parent().unwrap()).unwrap();
    fs::copy(&audit_src, &audit_dst).expect("copy audit script");
    // Preserve executable bit.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&audit_dst).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&audit_dst, perm).unwrap();
    }

    // The script accepts <fixture_dir> as $1. The cluster_dispatch
    // executor passes the feature_id; we treat the feature_id as a
    // path to the fixture for this end-to-end smoke. Write the fixture
    // under that subdir.
    let fixture_dir = workdir.join("FT-T373");
    write_fixture(&fixture_dir);

    let ctx = PlanContext {
        workdir: workdir.clone(),
        product_root: workdir.clone(),
        env_override: None,
    };

    cluster_dispatch::run(&ctx, "FT-T373", "add-judge-worker")
        .expect("cluster dispatch must complete cleanly against a positive fixture");
}

/// Companion assertion: planned_cell_order returns the same order
/// topo_order computes — confirms the dispatcher's planning surface
/// is consistent with the substrate types' contract.
#[test]
fn planned_cell_order_matches_topo_order() {
    let order = cluster_dispatch::planned_cell_order("add-judge-worker")
        .expect("registered TaskType returns Some");
    assert_eq!(order.len(), 5, "add-judge-worker has 5 cells");
    // capability_binding + pydantic_io_models have no derived_from, so
    // they should come first.
    assert!(
        order[0] == "capability_binding" || order[0] == "pydantic_io_models",
        "first cell must be a root (no derived_from); got {:?}",
        order[0]
    );
    // agent_loop derives from three others; must NOT be first.
    let agent_loop_pos = order
        .iter()
        .position(|s| s == "agent_loop")
        .expect("agent_loop in order");
    assert!(agent_loop_pos >= 3, "agent_loop must come after its 3 deps");
}
