//! `dec workers run <worker-image-id>` — operator-facing worker spawn (FT-095).
//!
//! Pulls a qualified `dec:WorkerImage` from the orchestration catalog,
//! reads the four required env vars from a local config file (default
//! `~/.dec/workers.env`, overridable by `--env-file`), and shells out to
//! `docker run --rm --env-file <path> <registry_ref>` with stdout/stderr
//! attached to the calling terminal.
//!
//! Per ADR-062, slice 1 ships no `WorkerSupervisor` — the operator IS the
//! supervisor. Per ADR-063, secrets live in env vars sourced from a
//! local file; provider keys never reach worker env (ADR-064 routes
//! provider calls through `LiteLLM`).

pub mod env_file;
pub mod runner;

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::core::ontology::worker_image::{
    query_by_id, EligibilityStatus, WorkerImage, WorkerImageReadError,
};
use crate::core::store::open_orchestration_store;

pub use env_file::{load_env_file, EnvFileError, ResolvedEnvFile, REQUIRED_ENV_VARS};
pub use runner::{DockerRunner, RunOutcome, RunPlan, SystemDockerRunner};

/// Input arguments for [`run`]. The CLI layer builds this and hands it
/// in alongside the docker runner; tests pass a mock runner.
#[derive(Debug, Clone)]
pub struct WorkersRunArgs {
    /// Stable id of the `dec:WorkerImage` to spawn.
    pub worker_image_id: String,
    /// Optional override of the env file path. Defaults to
    /// `~/.dec/workers.env` (or the legacy `~/.pipeline-cli/workers.env`
    /// path the `feature_spec` calls out).
    pub env_file: Option<PathBuf>,
    /// Docker CLI binary name (`docker` or `podman`).
    pub docker_binary: String,
}

impl WorkersRunArgs {
    /// Default args for a given worker image id — convenient for the
    /// CLI binding where `docker` is the default runtime.
    #[must_use]
    pub fn for_image<S: Into<String>>(id: S) -> Self {
        Self {
            worker_image_id: id.into(),
            env_file: None,
            docker_binary: "docker".to_string(),
        }
    }
}

/// Errors `workers run` can surface to the operator. The CLI layer
/// maps these to fixed exit codes (see [`WorkersRunError::exit_code`]).
#[derive(Debug, Error)]
pub enum WorkersRunError {
    /// `.dec/store/orchestration.nq` is missing or unreadable.
    #[error("opening orchestration store: {0}")]
    Store(String),
    /// SPARQL read against the catalog failed.
    #[error("reading worker image catalog: {0}")]
    Catalog(#[from] WorkerImageReadError),
    /// No `dec:WorkerImage` artifact has the requested id.
    #[error("no WorkerImage with id `{id}` is registered in the catalog")]
    NotFound {
        /// Worker image id that was looked up.
        id: String,
    },
    /// Catalog has a matching id but its eligibility is not `qualified`.
    #[error(
        "WorkerImage `{id}` (v{version}) has eligibility_status = `{status}`; \
         only `qualified` images may be dispatched"
    )]
    NotQualified {
        /// Worker image id.
        id: String,
        /// Worker image version (so the operator knows which row mismatched).
        version: String,
        /// Wire form of the eligibility status (`candidate` / `deprecated` / `pulled`).
        status: String,
    },
    /// The env-file read or parse failed; the variant carries the
    /// underlying reason (missing file, missing required key, etc.).
    #[error(transparent)]
    EnvFile(#[from] EnvFileError),
    /// `docker pull` exited with a non-zero status.
    #[error("docker pull failed for `{registry_ref}` (exit code {exit_code}): {message}")]
    PullFailed {
        /// OCI reference that the pull attempted.
        registry_ref: String,
        /// Exit code reported by docker.
        exit_code: i32,
        /// Stderr-derived message for the operator.
        message: String,
    },
    /// `docker run` itself returned a non-zero exit. The container ran
    /// (or attempted to); the exit code is propagated as-is.
    #[error("docker run exited with status {exit_code}")]
    RunFailed {
        /// Exit code reported by the container.
        exit_code: i32,
    },
    /// Spawning the docker binary failed (binary missing, no PATH, etc.).
    #[error("spawning {binary}: {message}")]
    Spawn {
        /// Docker / podman binary name that failed to spawn.
        binary: String,
        /// Underlying OS error message.
        message: String,
    },
}

impl WorkersRunError {
    /// Stable exit code the binary surfaces for this failure class. The
    /// distinct values mean operators (and CI) can tell whether the
    /// failure was a config problem (2) or an external substrate
    /// problem (3) without parsing the error string.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            // Catalog / config failures — operator needs to fix data.
            Self::Store(_)
            | Self::Catalog(_)
            | Self::NotFound { .. }
            | Self::NotQualified { .. }
            | Self::EnvFile(_) => 2,
            // Docker substrate failures — image pull or run errored.
            Self::PullFailed { .. } | Self::Spawn { .. } => 3,
            // The container ran but exited non-zero — propagate as-is
            // (clamped to u8 range) so wrappers see the worker's status.
            Self::RunFailed { exit_code } => clamp_exit_code(*exit_code),
        }
    }
}

/// What [`run`] produced on success. Useful for tests to inspect the
/// commands that would have been executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkersRunOutcome {
    /// The resolved worker image (post `qualified` check).
    pub image: WorkerImage,
    /// Path of the env file that was read.
    pub env_file_path: PathBuf,
    /// The plan that was executed (pull + run argv).
    pub plan: RunPlan,
    /// The outcome the runner reported (pull/run exit codes).
    pub run_outcome: RunOutcome,
}

/// Execute the `dec workers run` flow against the orchestration store
/// at `workdir` and the supplied [`DockerRunner`]. Returns the resolved
/// image + executed plan on success, or a structured error.
///
/// Order of operations (per FT-095 success criteria — *non-existent id,
/// non-qualified image, or missing env var produces a clean exit before
/// any container starts*):
///
/// 1. Open orchestration store.
/// 2. Look up `WorkerImage` by id.
/// 3. Pick the highest-version `qualified` row; refuse otherwise.
/// 4. Load + validate env file (all four required vars must be set).
/// 5. `docker pull <registry_ref>` — propagate failure verbatim.
/// 6. `docker run --rm --env-file <env_file_path> <registry_ref>`.
pub fn run<R: DockerRunner>(
    workdir: &Path,
    args: &WorkersRunArgs,
    runner: &R,
) -> Result<WorkersRunOutcome, WorkersRunError> {
    let image = resolve_qualified_image(workdir, &args.worker_image_id)?;
    let env = load_env_file(args.env_file.as_deref())?;
    let plan = RunPlan::for_image(&args.docker_binary, &image.registry_ref, &env.path);
    let run_outcome = runner.execute(&plan)?;
    Ok(WorkersRunOutcome {
        image,
        env_file_path: env.path,
        plan,
        run_outcome,
    })
}

/// Resolve a worker image id to a `qualified` row, or return a
/// structured error. Public so the CLI layer can pre-flight catalog
/// lookups without going through the full `run` pipeline.
pub fn resolve_qualified_image(
    workdir: &Path,
    worker_image_id: &str,
) -> Result<WorkerImage, WorkersRunError> {
    let store =
        open_orchestration_store(workdir).map_err(|e| WorkersRunError::Store(format!("{e:#}")))?;
    let mut hits = query_by_id(&store, worker_image_id)?;
    if hits.is_empty() {
        return Err(WorkersRunError::NotFound {
            id: worker_image_id.to_string(),
        });
    }
    // Records are sorted by version asc; the newest entry is the last.
    // Pick the highest-version `qualified` row if any; otherwise refuse.
    if let Some(qualified) = hits
        .iter()
        .rev()
        .find(|img| img.eligibility_status == EligibilityStatus::Qualified)
        .cloned()
    {
        return Ok(qualified);
    }
    // No qualified row — surface the newest non-qualified status so
    // the operator sees the actionable detail. `pop` cannot fail
    // because the empty-hits branch above returned early.
    let newest = hits.pop().ok_or_else(|| WorkersRunError::NotFound {
        id: worker_image_id.to_string(),
    })?;
    Err(WorkersRunError::NotQualified {
        id: newest.id,
        version: newest.version,
        status: newest.eligibility_status.as_str().to_string(),
    })
}

fn clamp_exit_code(raw: i32) -> u8 {
    if !(1..=255).contains(&raw) {
        1
    } else {
        u8::try_from(raw).unwrap_or(1)
    }
}
