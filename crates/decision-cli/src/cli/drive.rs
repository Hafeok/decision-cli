//! `dec drive <goal> <artifact>` — pluggable artifact+goal orchestrator
//! (FT-110).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_cli::core::drive::{ArtifactRef, Goal, PlanContext};
use decision_cli::drive::{run as drive_run, DriveError, RunArgs, DEFAULT_MAX_ITER};

#[derive(Debug, clap::Args)]
pub struct DriveArgs {
    /// Goal: `ship`, `verify`, `accept`, `cover`, or `approve`.
    pub goal: String,
    /// Artifact short id (e.g. `FT-019`, `TC-027`).
    pub artifact: String,
    /// Bail out after N planner iterations. Default 5.
    #[arg(long)]
    pub max_iter: Option<usize>,
    /// Env override passed to dispatch actions that need one.
    #[arg(long, value_name = "ENV-NNN")]
    pub env: Option<String>,
    /// Override the product-cli root (default: same as `--workdir`).
    #[arg(long, value_name = "PATH")]
    pub product_root: Option<PathBuf>,
}

pub fn run(workdir: &Path, args: DriveArgs) -> ExitCode {
    // Use 0 as the "iterations" value before the planner reports it.
    let goal = match Goal::parse(&args.goal) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("dec drive: {e}");
            return ExitCode::from(2);
        }
    };
    let artifact = match ArtifactRef::parse(&args.artifact) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("dec drive: {e}");
            return ExitCode::from(2);
        }
    };
    let ctx = match PlanContext::open(workdir.to_path_buf(), args.product_root, args.env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dec drive: {e}");
            return ExitCode::from(2);
        }
    };
    let run_args = RunArgs {
        goal,
        artifact,
        max_iter: args.max_iter.unwrap_or(DEFAULT_MAX_ITER),
    };

    match drive_run(&ctx, &run_args) {
        Ok(outcome) => {
            println!(
                "drive: reached goal in {n} iteration(s)",
                n = outcome.iterations
            );
            for entry in &outcome.history {
                println!("  [{i}] {tag}", i = entry.iteration, tag = entry.action.tag());
            }
            ExitCode::SUCCESS
        }
        Err(DriveError::Stuck { reason, history }) => {
            eprintln!("drive: stuck — {reason}");
            for entry in &history {
                eprintln!("  [{i}] {tag}", i = entry.iteration, tag = entry.action.tag());
            }
            ExitCode::from(3)
        }
        Err(DriveError::MaxIterations { max, history }) => {
            eprintln!("drive: hit iteration cap ({max}); not converging");
            for entry in &history {
                eprintln!("  [{i}] {tag}", i = entry.iteration, tag = entry.action.tag());
            }
            ExitCode::from(3)
        }
        Err(other) => {
            eprintln!("dec drive: {other}");
            ExitCode::from(1)
        }
    }
}
