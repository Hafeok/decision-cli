//! `dec workers run <id>` — operator-facing worker spawn (FT-095).
//!
//! Thin CLI adapter; per ADR-013 §Rule 3 the binary entry point only
//! parses + dispatches. All business logic (catalog lookup, env-file
//! validation, docker invocation) lives in
//! `decision_cli::workers_run`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::workers_run::{self, SystemDockerRunner, WorkersRunArgs, WorkersRunError};

/// `dec workers <subcommand>` — slice 1 surface (only `run`).
#[derive(Debug, Subcommand)]
pub enum WorkersCmd {
    /// Pull a qualified `dec:WorkerImage` and start it locally (FT-095).
    Run(RunArgs),
}

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    /// Stable id of the `dec:WorkerImage` to spawn. Looked up in
    /// `.dec/store/orchestration.nq`; must have
    /// `eligibility_status = qualified`.
    pub worker_image_id: String,
    /// Override the env-file path (default `~/.dec/workers.env`,
    /// falling back to `~/.pipeline-cli/workers.env`). Must contain
    /// `PIPELINE_ENDPOINT`, `PIPELINE_TOKEN`, `LITELLM_BASE_URL`,
    /// `LITELLM_API_KEY`.
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
    /// Docker CLI binary to invoke. Defaults to `docker`; pass
    /// `--docker-binary podman` to use podman.
    #[arg(long, value_name = "BIN", default_value = "docker")]
    pub docker_binary: String,
}

pub fn run(workdir: &Path, cmd: WorkersCmd) -> ExitCode {
    match cmd {
        WorkersCmd::Run(args) => run_subcommand(workdir, args),
    }
}

fn run_subcommand(workdir: &Path, args: RunArgs) -> ExitCode {
    let inputs = WorkersRunArgs {
        worker_image_id: args.worker_image_id,
        env_file: args.env_file,
        docker_binary: args.docker_binary,
    };
    match workers_run::run(workdir, &inputs, &SystemDockerRunner) {
        Ok(outcome) => {
            println!(
                "dec workers run: container exited cleanly for {}@v{} ({})",
                outcome.image.id, outcome.image.version, outcome.plan.registry_ref,
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_error(&err);
            ExitCode::from(err.exit_code())
        }
    }
}

fn print_error(err: &WorkersRunError) {
    eprintln!("dec workers run: {err}");
}
