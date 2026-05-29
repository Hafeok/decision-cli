//! `dec drive <goal> <artifact>` — pluggable artifact+goal orchestrator
//! (FT-110, FT-111).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use decision_cli::core::drive::{ArtifactRef, Goal, PlanContext};
use decision_cli::drive::{run as drive_run, DriveError, RunArgs, DEFAULT_MAX_ITER};
use decision_cli::ft_111_drive_ship_all::{
    apply_filter, render, resolve_features, run_sweep, Format, SweepInput,
};

#[derive(Debug, clap::Args)]
pub struct DriveArgs {
    /// Goal: `ship`, `verify`, `accept`, `cover`, or `approve`.
    pub goal: String,
    /// Artifact short id (e.g. `FT-019`, `TC-027`). Mutually exclusive with --all.
    #[arg(conflicts_with = "all")]
    pub artifact: Option<String>,
    /// Sweep all features (only supported with `ship` goal).
    #[arg(long, conflicts_with = "artifact")]
    pub all: bool,
    /// Bail out after N planner iterations. Default 5.
    #[arg(long)]
    pub max_iter: Option<usize>,
    /// Bench override passed to dispatch actions that need one (renamed from --env by FT-112).
    #[arg(long, value_name = "BNCH-NNN")]
    pub bench: Option<String>,
    /// RESERVED: --env is reserved for future deployment-target use; use --bench instead (FT-112).
    #[arg(long, value_name = "ENV-NNN", hide = true)]
    pub env: Option<String>,
    /// Override the product-cli root (default: same as `--workdir`).
    #[arg(long, value_name = "PATH")]
    pub product_root: Option<PathBuf>,
    /// Comma-separated feature filter for --all (e.g. FT-1,FT-2,FT-10).
    #[arg(long, value_delimiter = ',', requires = "all")]
    pub filter: Option<Vec<String>>,
    /// Per-feature timeout in seconds for --all. Default 600.
    #[arg(long, requires = "all", default_value = "600")]
    pub per_feature_timeout: u64,
    /// Output format for --all: text, tsv, or json. Default text.
    #[arg(long, requires = "all", default_value = "text")]
    pub format: String,
    /// Retire non-approved graphs before sweep (--all only).
    #[arg(long, requires = "all")]
    pub retire_failing_graphs: bool,
}

pub fn run(workdir: &Path, args: DriveArgs) -> ExitCode {
    // FT-112: --env is reserved for future deployment-target use
    if args.env.is_some() {
        eprintln!("dec drive: --env is reserved for future deployment-target use; use --bench BNCH-NNN instead");
        return ExitCode::from(2);
    }

    // Route to sweep if --all is set
    if args.all {
        return run_sweep_all(workdir, args);
    }

    // Single-artifact drive
    let Some(artifact_str) = args.artifact else {
        eprintln!("dec drive: artifact required (or use --all for sweep)");
        return ExitCode::from(2);
    };

    let goal = match Goal::parse(&args.goal) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("dec drive: {e}");
            return ExitCode::from(2);
        }
    };
    let artifact = match ArtifactRef::parse(&artifact_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("dec drive: {e}");
            return ExitCode::from(2);
        }
    };
    let ctx = match PlanContext::open(workdir.to_path_buf(), args.product_root, args.bench) {
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

fn run_sweep_all(workdir: &Path, args: DriveArgs) -> ExitCode {
    // Validate goal is ship
    if args.goal != "ship" {
        eprintln!("dec drive --all: only 'ship' goal is currently supported");
        return ExitCode::from(2);
    }

    // Parse format
    let format = match Format::parse(&args.format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive --all: {e}");
            return ExitCode::from(2);
        }
    };

    // Resolve product root
    let product_root = args
        .product_root
        .unwrap_or_else(|| workdir.join(".product"));

    // Resolve features
    let resolved = match resolve_features(&product_root) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive --all: {e}");
            return ExitCode::from(2);
        }
    };

    // Apply filter
    let features = match apply_filter(resolved, args.filter) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive --all: {e}");
            return ExitCode::from(2);
        }
    };

    if features.is_empty() {
        eprintln!("dec drive --all: no features match filter");
        return ExitCode::from(2);
    }

    // TODO: implement retire-failing-graphs pre-pass when args.retire_failing_graphs is true
    if args.retire_failing_graphs {
        eprintln!("dec drive --all: --retire-failing-graphs not yet implemented");
    }

    // Run sweep
    let sweep_input = SweepInput {
        features,
        env_id: args.bench,
        max_iter: args.max_iter.unwrap_or(6), // FT-111 default is 6, not 5
        per_item_timeout: Duration::from_secs(args.per_feature_timeout),
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("dec drive --all: failed to create async runtime: {e}");
            return ExitCode::from(1);
        }
    };

    let (rows, tally) = match runtime.block_on(run_sweep(workdir.to_path_buf(), sweep_input)) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("dec drive --all: {e}");
            return ExitCode::from(1);
        }
    };

    // Render output
    let output = render(&rows, &tally, format);
    print!("{output}");

    // Exit code: 0 if all done, 1 otherwise
    if tally.done == rows.len() && rows.len() > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
