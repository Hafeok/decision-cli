//! Docker invocation seam for `dec workers run` (FT-095 / ADR-062).
//!
//! Captures the two CLI invocations the feature needs — `docker pull
//! <ref>` and `docker run --rm --env-file <path> <ref>` — behind a
//! trait so the integration test can verify the planned argv (and
//! injected env-file path) without requiring docker on the host.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::WorkersRunError;

/// The argv pair `dec workers run` constructs from a resolved
/// `WorkerImage` and an env-file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlan {
    /// `docker` (or `podman`) binary.
    pub binary: String,
    /// `docker pull <registry_ref>` argv (without the binary).
    pub pull_args: Vec<String>,
    /// `docker run --rm --env-file <path> <registry_ref>` argv
    /// (without the binary).
    pub run_args: Vec<String>,
    /// Absolute path of the env file the run will use; mirrored on the
    /// outcome so tests can assert it survived intact.
    pub env_file_path: PathBuf,
    /// Registry reference (OCI URI with `@sha256:<digest>`); mirrored
    /// so callers don't have to re-parse the run argv.
    pub registry_ref: String,
}

impl RunPlan {
    /// Construct the canonical pull + run argv for a given image.
    #[must_use]
    pub fn for_image(binary: &str, registry_ref: &str, env_file: &Path) -> Self {
        Self {
            binary: binary.to_string(),
            pull_args: vec!["pull".to_string(), registry_ref.to_string()],
            run_args: vec![
                "run".to_string(),
                "--rm".to_string(),
                "--env-file".to_string(),
                env_file.display().to_string(),
                registry_ref.to_string(),
            ],
            env_file_path: env_file.to_path_buf(),
            registry_ref: registry_ref.to_string(),
        }
    }
}

/// What the runner reports after executing a plan. Distinct fields for
/// pull and run so tests can assert the order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// Exit code reported by `docker pull`.
    pub pull_exit_code: i32,
    /// Exit code reported by `docker run`.
    pub run_exit_code: i32,
}

/// Seam between the feature and the docker binary. Production code
/// uses [`SystemDockerRunner`]; tests use a hand-rolled mock that
/// records the plan it was asked to execute.
pub trait DockerRunner {
    /// Execute pull, then (on success) run. Returns a `WorkersRunError`
    /// if either step fails.
    fn execute(&self, plan: &RunPlan) -> Result<RunOutcome, WorkersRunError>;
}

/// `DockerRunner` impl that shells out to a real `docker` binary on
/// `PATH`. stdout / stderr are inherited so the operator sees pull
/// progress and worker logs live.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDockerRunner;

impl DockerRunner for SystemDockerRunner {
    fn execute(&self, plan: &RunPlan) -> Result<RunOutcome, WorkersRunError> {
        spawn_pull(plan)?;
        spawn_run(plan)
    }
}

fn spawn_pull(plan: &RunPlan) -> Result<(), WorkersRunError> {
    let status = Command::new(&plan.binary)
        .args(&plan.pull_args)
        .status()
        .map_err(|e| WorkersRunError::Spawn {
            binary: plan.binary.clone(),
            message: e.to_string(),
        })?;
    if status.success() {
        return Ok(());
    }
    let code = status.code().unwrap_or(1);
    Err(WorkersRunError::PullFailed {
        registry_ref: plan.registry_ref.clone(),
        exit_code: code,
        message: format!(
            "`{} pull {}` exited with status {}",
            plan.binary,
            plan.registry_ref,
            status.code().unwrap_or(-1)
        ),
    })
}

fn spawn_run(plan: &RunPlan) -> Result<RunOutcome, WorkersRunError> {
    let status = Command::new(&plan.binary)
        .args(&plan.run_args)
        .status()
        .map_err(|e| WorkersRunError::Spawn {
            binary: plan.binary.clone(),
            message: e.to_string(),
        })?;
    if status.success() {
        Ok(RunOutcome {
            pull_exit_code: 0,
            run_exit_code: 0,
        })
    } else {
        Err(WorkersRunError::RunFailed {
            exit_code: status.code().unwrap_or(1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_plan_for_image_emits_canonical_argv() {
        let plan = RunPlan::for_image(
            "docker",
            "ghcr.io/example/img@sha256:cafeb0de",
            Path::new("/tmp/env"),
        );
        assert_eq!(plan.binary, "docker");
        assert_eq!(
            plan.pull_args,
            vec!["pull".to_string(), "ghcr.io/example/img@sha256:cafeb0de".to_string()]
        );
        assert_eq!(
            plan.run_args,
            vec![
                "run".to_string(),
                "--rm".to_string(),
                "--env-file".to_string(),
                "/tmp/env".to_string(),
                "ghcr.io/example/img@sha256:cafeb0de".to_string(),
            ]
        );
        assert_eq!(plan.env_file_path, PathBuf::from("/tmp/env"));
        assert_eq!(plan.registry_ref, "ghcr.io/example/img@sha256:cafeb0de");
    }
}
