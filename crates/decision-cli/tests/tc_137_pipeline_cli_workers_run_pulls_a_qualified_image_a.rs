//! TC-137 — `pipeline-cli workers run` (a.k.a. `dec workers run`)
//! pulls a qualified image and starts it with the four required env vars.
//!
//! Validates: FT-095 · ADR-062 · ADR-063 · ADR-055.
//! Spec: `.product/tests/TC-137-pipeline-cli-workers-run-pulls-a-qualified-image-a.md`
//!
//! What this test pins down end-to-end:
//!
//! 1. A `qualified` `dec:WorkerImage` is resolved from the orchestration
//!    catalog by its stable id, and the resolved `registry_ref` flows
//!    into `docker pull` and `docker run` argv verbatim.
//! 2. The env file passed via `--env-file <path>` is read, validated
//!    for the four required keys (`PIPELINE_ENDPOINT`, `PIPELINE_TOKEN`,
//!    `LITELLM_BASE_URL`, `LITELLM_API_KEY`), and the same path is
//!    plumbed through to `docker run --env-file <path>`.
//! 3. A missing env var, a non-qualified image, and an unknown image id
//!    each error out *before any container starts* — verified by
//!    asserting the mock docker runner was never asked to execute.
//!
//! The docker invocation seam is mocked: a real docker binary is not
//! required to run the test.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use oxigraph::store::Store;

use decision_cli::core::ontology::worker_image::{EligibilityStatus, WorkerImage};
use decision_cli::core::store::{orchestration_dump_path, persist_store};
use decision_cli::vocab::worker_image_graph;
use decision_cli::workers_run::{
    self, DockerRunner, RunOutcome, RunPlan, WorkersRunArgs, WorkersRunError,
};

// ---------------------------------------------------------------------------
// Mock DockerRunner — records every plan it was asked to execute so the
// test can assert pull/run argv and the env-file path that was wired in.
// ---------------------------------------------------------------------------

struct MockDocker {
    invocations: RefCell<Vec<RunPlan>>,
    outcome: RunOutcome,
}

impl MockDocker {
    fn success() -> Self {
        Self {
            invocations: RefCell::new(Vec::new()),
            outcome: RunOutcome {
                pull_exit_code: 0,
                run_exit_code: 0,
            },
        }
    }
}

impl DockerRunner for MockDocker {
    fn execute(&self, plan: &RunPlan) -> Result<RunOutcome, WorkersRunError> {
        self.invocations.borrow_mut().push(plan.clone());
        Ok(self.outcome.clone())
    }
}

// ---------------------------------------------------------------------------
// Fixture helpers.
// ---------------------------------------------------------------------------

fn tempdir(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    p.push(format!("{label}-{pid}-{nonce}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn image(id: &str, version: &str, status: EligibilityStatus) -> WorkerImage {
    WorkerImage {
        id: id.to_string(),
        name: format!("Image {id}"),
        version: version.to_string(),
        registry_ref: format!(
            "ghcr.io/example/{id}@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        ),
        capability_tags: vec!["code-writer".to_string()],
        compatible_roles: Vec::new(),
        signed_by_subject: format!(
            "https://github.com/example/{id}/.github/workflows/build.yml@refs/heads/main"
        ),
        signed_by_issuer: "https://token.actions.githubusercontent.com".to_string(),
        sbom_ref: format!(
            "ghcr.io/example/{id}@sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"
        ),
        conformance_audits: Vec::new(),
        eligibility_status: status,
        source_repo_uri: format!("https://github.com/example/{id}"),
        source_commit_hash: "abc123def456".to_string(),
        build_run_url: format!("https://github.com/example/{id}/actions/runs/1"),
    }
}

/// Materialise a `.dec/store/orchestration.nq` containing the supplied
/// images under the catalog graph so the resolver can read them.
fn seed_store(workdir: &Path, images: &[&WorkerImage]) {
    let store = Store::new().expect("memory store");
    for img in images {
        for q in img.to_quads(worker_image_graph()) {
            store.insert(&q).expect("insert quad");
        }
    }
    let dump = orchestration_dump_path(workdir);
    persist_store(&store, &dump).expect("persist store");
}

fn write_env_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, content).expect("write env file");
    p
}

const COMPLETE_ENV: &str = "PIPELINE_ENDPOINT=https://pipeline.example/sse\n\
                            PIPELINE_TOKEN=secret-token\n\
                            LITELLM_BASE_URL=http://localhost:4000\n\
                            LITELLM_API_KEY=sk-litellm-virtual-key\n";

// ---------------------------------------------------------------------------
// 1. Happy path: pull qualified image, run with the four required env vars.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_pulls_qualified_image_and_starts_with_required_env_vars() {
    let workdir = tempdir("tc137-happy");
    let img = image("code-writer-impl", "1.0.0", EligibilityStatus::Qualified);
    seed_store(&workdir, &[&img]);
    let env_path = write_env_file(&workdir, "workers.env", COMPLETE_ENV);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "code-writer-impl".to_string(),
        env_file: Some(env_path.clone()),
        docker_binary: "docker".to_string(),
    };

    let outcome = workers_run::run(&workdir, &args, &runner)
        .expect("qualified image + complete env file -> success");

    // (a) The resolved image is the one we seeded.
    assert_eq!(outcome.image.id, "code-writer-impl");
    assert_eq!(outcome.image.version, "1.0.0");
    assert_eq!(
        outcome.image.eligibility_status,
        EligibilityStatus::Qualified
    );

    // (b) The pull argv targets the resolved `registry_ref` verbatim.
    assert_eq!(
        outcome.plan.pull_args,
        vec!["pull".to_string(), outcome.image.registry_ref.clone()]
    );
    assert_eq!(outcome.plan.binary, "docker");

    // (c) The run argv carries `--rm`, `--env-file <path>`, and the
    //     same registry_ref — proving the four required env vars
    //     reach the container via the env-file plumbing.
    assert_eq!(
        outcome.plan.run_args,
        vec![
            "run".to_string(),
            "--rm".to_string(),
            "--env-file".to_string(),
            env_path.display().to_string(),
            outcome.image.registry_ref.clone(),
        ]
    );
    assert_eq!(outcome.env_file_path, env_path);
    assert_eq!(outcome.plan.env_file_path, env_path);

    // (d) The runner was invoked exactly once with that plan; both
    //     pull and run reported success.
    let calls = runner.invocations.borrow();
    assert_eq!(calls.len(), 1, "expected one execute() call, got {calls:?}");
    assert_eq!(calls[0], outcome.plan);
    assert_eq!(outcome.run_outcome.pull_exit_code, 0);
    assert_eq!(outcome.run_outcome.run_exit_code, 0);
}

// ---------------------------------------------------------------------------
// 2. Required-env-var coverage — the run argv refers to the env-file
//    that contains the four required keys. Reading the file back from
//    disk demonstrates the keys actually reach the container.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_env_file_contains_all_four_required_keys() {
    let workdir = tempdir("tc137-env");
    let img = image("code-writer-impl", "1.0.0", EligibilityStatus::Qualified);
    seed_store(&workdir, &[&img]);
    let env_path = write_env_file(&workdir, "workers.env", COMPLETE_ENV);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "code-writer-impl".to_string(),
        env_file: Some(env_path.clone()),
        docker_binary: "docker".to_string(),
    };
    let outcome = workers_run::run(&workdir, &args, &runner).expect("happy path must succeed");

    let raw = fs::read_to_string(&outcome.env_file_path).expect("read env file");
    for required in [
        "PIPELINE_ENDPOINT",
        "PIPELINE_TOKEN",
        "LITELLM_BASE_URL",
        "LITELLM_API_KEY",
    ] {
        assert!(
            raw.contains(&format!("{required}=")),
            "env file must contain `{required}=…`, contents were:\n{raw}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Refusal: image id not present in catalog. Errors before any
//    container starts.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_refuses_unknown_image_id_before_any_container_starts() {
    let workdir = tempdir("tc137-unknown");
    let img = image("code-writer-impl", "1.0.0", EligibilityStatus::Qualified);
    seed_store(&workdir, &[&img]);
    let env_path = write_env_file(&workdir, "workers.env", COMPLETE_ENV);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "does-not-exist".to_string(),
        env_file: Some(env_path),
        docker_binary: "docker".to_string(),
    };
    let err =
        workers_run::run(&workdir, &args, &runner).expect_err("unknown id must produce an error");
    assert!(
        matches!(err, WorkersRunError::NotFound { ref id } if id == "does-not-exist"),
        "expected NotFound, got {err:?}"
    );
    assert!(
        runner.invocations.borrow().is_empty(),
        "no docker invocation must occur for unknown ids"
    );
    assert_ne!(err.exit_code(), 0, "non-zero exit on refusal");
}

// ---------------------------------------------------------------------------
// 4. Refusal: image present but not qualified (e.g. `pulled`). Errors
//    before any container starts.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_refuses_non_qualified_image_before_any_container_starts() {
    let workdir = tempdir("tc137-pulled");
    let pulled = image("pulled-impl", "1.0.0", EligibilityStatus::Pulled);
    seed_store(&workdir, &[&pulled]);
    let env_path = write_env_file(&workdir, "workers.env", COMPLETE_ENV);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "pulled-impl".to_string(),
        env_file: Some(env_path),
        docker_binary: "docker".to_string(),
    };
    let err = workers_run::run(&workdir, &args, &runner)
        .expect_err("non-qualified must produce an error");
    match err {
        WorkersRunError::NotQualified {
            ref id,
            ref status,
            ref version,
        } => {
            assert_eq!(id, "pulled-impl");
            assert_eq!(version, "1.0.0");
            assert_eq!(status, "pulled");
        }
        other => panic!("expected NotQualified, got {other:?}"),
    }
    assert!(
        runner.invocations.borrow().is_empty(),
        "non-qualified must short-circuit before docker is invoked"
    );
}

// ---------------------------------------------------------------------------
// 5. Refusal: env file missing one required key. Errors before any
//    container starts.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_refuses_when_a_required_env_var_is_missing() {
    let workdir = tempdir("tc137-missing-env");
    let img = image("code-writer-impl", "1.0.0", EligibilityStatus::Qualified);
    seed_store(&workdir, &[&img]);
    // Drop LITELLM_API_KEY.
    let partial = "PIPELINE_ENDPOINT=https://e/\n\
                   PIPELINE_TOKEN=t\n\
                   LITELLM_BASE_URL=http://localhost:4000\n";
    let env_path = write_env_file(&workdir, "partial.env", partial);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "code-writer-impl".to_string(),
        env_file: Some(env_path),
        docker_binary: "docker".to_string(),
    };
    let err = workers_run::run(&workdir, &args, &runner)
        .expect_err("missing env var must produce an error");
    let msg = format!("{err}");
    assert!(msg.contains("LITELLM_API_KEY"), "{msg}");
    assert!(
        runner.invocations.borrow().is_empty(),
        "missing env var must short-circuit before docker is invoked"
    );
}

// ---------------------------------------------------------------------------
// 6. When the catalog has both a qualified and a pulled row for the
//    same id, the qualified one is picked.
// ---------------------------------------------------------------------------

#[test]
fn workers_run_prefers_qualified_row_over_other_statuses() {
    let workdir = tempdir("tc137-mixed");
    let qualified = image("code-writer-impl", "2.0.0", EligibilityStatus::Qualified);
    let pulled = image("code-writer-impl", "1.0.0", EligibilityStatus::Pulled);
    // Re-tag pulled so it has a different registry_ref / sbom_ref to
    // avoid colliding with the qualified row in the dump.
    let mut pulled = pulled;
    pulled.registry_ref =
        "ghcr.io/example/code-writer-impl@sha256:0123456789012345678901234567890123456789012345678901234567890123"
            .to_string();
    pulled.sbom_ref =
        "ghcr.io/example/code-writer-impl@sha256:4567456745674567456745674567456745674567456745674567456745674567"
            .to_string();
    seed_store(&workdir, &[&qualified, &pulled]);
    let env_path = write_env_file(&workdir, "workers.env", COMPLETE_ENV);

    let runner = MockDocker::success();
    let args = WorkersRunArgs {
        worker_image_id: "code-writer-impl".to_string(),
        env_file: Some(env_path),
        docker_binary: "docker".to_string(),
    };
    let outcome = workers_run::run(&workdir, &args, &runner)
        .expect("mixed catalog must pick the qualified row");
    assert_eq!(outcome.image.version, "2.0.0");
    assert_eq!(
        outcome.image.eligibility_status,
        EligibilityStatus::Qualified
    );
}
