//! `dec drive {ship,show}` — pluggable artifact+goal orchestrator
//! and drive history viewer (FT-110, FT-111, FT-113).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::Subcommand;
use decision_cli::core::drive::{ArtifactRef, Goal, PlanContext};
use decision_cli::drive::{run as drive_run, DriveError, RunArgs, DEFAULT_MAX_ITER};
use decision_cli::ft_111_drive_ship_all::{
    apply_filter, render, resolve_features, run_sweep, Format, SweepInput,
};
use decision_cli::ft_113_drive_show;

#[derive(Debug, Subcommand)]
pub enum DriveCmd {
    /// Run a goal-driven dispatch loop (FT-110, FT-111).
    Ship(ShipArgs),
    /// Drive a feature through the Definition-of-Ready gate (FT-119).
    /// Dispatches `verify-graph-author` when a covering graph is
    /// missing; reports `Stuck` for every gap requiring human
    /// authoring (spec body, preflight ack, missing TCs, etc.).
    DefReady(DefReadyArgs),
    /// Show per-round narrative for a feature drive (FT-113).
    Show(ShowArgs),
}

#[derive(Debug, clap::Args)]
pub struct ShipArgs {
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

#[derive(Debug, clap::Args)]
pub struct DefReadyArgs {
    /// Feature short id (e.g. `FT-019`). Mutually exclusive with --all.
    #[arg(conflicts_with = "all")]
    pub artifact: Option<String>,
    /// Sweep every feature whose dependencies have shipped.
    #[arg(long, conflicts_with = "artifact")]
    pub all: bool,
    /// Bail out after N planner iterations (default 5).
    #[arg(long)]
    pub max_iter: Option<usize>,
    /// Bench override threaded to `verify-graph-author` dispatches.
    #[arg(long, value_name = "BNCH-NNN")]
    pub bench: Option<String>,
    /// Override the product-cli root (default: same as `--workdir`).
    #[arg(long, value_name = "PATH")]
    pub product_root: Option<PathBuf>,
    /// Comma-separated feature filter for --all.
    #[arg(long, value_delimiter = ',', requires = "all")]
    pub filter: Option<Vec<String>>,
    /// Per-feature timeout in seconds for --all. Default 600.
    #[arg(long, requires = "all", default_value = "600")]
    pub per_feature_timeout: u64,
    /// Output format for --all: text, tsv, or json. Default text.
    #[arg(long, requires = "all", default_value = "text")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct ShowArgs {
    /// Feature ID (e.g. FT-113).
    pub feature_id: String,
    /// Filter to drives on a specific bench.
    #[arg(long, value_name = "BNCH-NNN")]
    pub bench: Option<String>,
    /// Re-render every N seconds with screen clear (default 2s).
    #[arg(long)]
    pub watch: bool,
    /// Poll interval for --watch mode in seconds (default 2, range 1-60).
    #[arg(long, requires = "watch")]
    pub interval: Option<u64>,
    /// Start at round N (useful when drive is long).
    #[arg(long)]
    pub since: Option<u32>,
    /// Output format: text or json (default text).
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Show all drives instead of just the most recent.
    #[arg(long)]
    pub all_drives: bool,
}

pub fn run(workdir: &Path, cmd: DriveCmd) -> ExitCode {
    match cmd {
        DriveCmd::Ship(args) => run_ship(workdir, args),
        DriveCmd::DefReady(args) => run_def_ready(workdir, args),
        DriveCmd::Show(args) => run_show(workdir, args),
    }
}

fn run_ship(workdir: &Path, args: ShipArgs) -> ExitCode {
    // FT-112: --env is reserved for future deployment-target use
    if args.env.is_some() {
        eprintln!("dec drive ship: --env is reserved for future deployment-target use; use --bench BNCH-NNN instead");
        return ExitCode::from(2);
    }

    // Route to sweep if --all is set
    if args.all {
        return run_sweep_all(workdir, args);
    }

    // Single-artifact drive
    let Some(artifact_str) = args.artifact else {
        eprintln!("dec drive ship: artifact required (or use --all for sweep)");
        return ExitCode::from(2);
    };

    // Goal is implicit from the subcommand (TC-259)
    let goal = Goal::Ship;
    let artifact = match ArtifactRef::parse(&artifact_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("dec drive ship: {e}");
            return ExitCode::from(2);
        }
    };
    let ctx = match PlanContext::open(workdir.to_path_buf(), args.product_root, args.bench) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dec drive ship: {e}");
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
            eprintln!("dec drive ship: {other}");
            ExitCode::from(1)
        }
    }
}

fn run_sweep_all(workdir: &Path, args: ShipArgs) -> ExitCode {
    // Goal is implicit from the subcommand (TC-259)
    // No validation needed - ship is the only goal for this command

    // Parse format
    let format = match Format::parse(&args.format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive ship --all: {e}");
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
            eprintln!("dec drive ship --all: {e}");
            return ExitCode::from(2);
        }
    };

    // Apply filter
    let features = match apply_filter(resolved, args.filter) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive ship --all: {e}");
            return ExitCode::from(2);
        }
    };

    if features.is_empty() {
        eprintln!("dec drive ship --all: no features match filter");
        return ExitCode::from(2);
    }

    // TODO: implement retire-failing-graphs pre-pass when args.retire_failing_graphs is true
    if args.retire_failing_graphs {
        eprintln!("dec drive ship --all: --retire-failing-graphs not yet implemented");
    }

    // Run sweep
    let sweep_input = SweepInput {
        features,
        env_id: args.bench,
        max_iter: args.max_iter.unwrap_or(6), // FT-111 default is 6, not 5
        per_item_timeout: Duration::from_secs(args.per_feature_timeout),
        goal: Goal::Ship,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("dec drive ship --all: failed to create async runtime: {e}");
            return ExitCode::from(1);
        }
    };

    let (rows, tally) = match runtime.block_on(run_sweep(workdir.to_path_buf(), sweep_input)) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("dec drive ship --all: {e}");
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

fn run_def_ready(workdir: &Path, args: DefReadyArgs) -> ExitCode {
    if args.all {
        return run_def_ready_sweep(workdir, args);
    }
    let Some(artifact_str) = args.artifact else {
        eprintln!("dec drive def-ready: artifact required (or use --all for sweep)");
        return ExitCode::from(2);
    };
    let artifact = match ArtifactRef::parse(&artifact_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("dec drive def-ready: {e}");
            return ExitCode::from(2);
        }
    };
    let ctx = match PlanContext::open(workdir.to_path_buf(), args.product_root, args.bench) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dec drive def-ready: {e}");
            return ExitCode::from(2);
        }
    };
    let run_args = RunArgs {
        goal: Goal::DefReady,
        artifact,
        max_iter: args.max_iter.unwrap_or(DEFAULT_MAX_ITER),
    };
    match drive_run(&ctx, &run_args) {
        Ok(outcome) => {
            println!(
                "drive: reached def-ready in {n} iteration(s)",
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
            eprintln!("dec drive def-ready: {other}");
            ExitCode::from(1)
        }
    }
}

fn run_def_ready_sweep(workdir: &Path, args: DefReadyArgs) -> ExitCode {
    let format = match Format::parse(&args.format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive def-ready --all: {e}");
            return ExitCode::from(2);
        }
    };
    let product_root = args
        .product_root
        .unwrap_or_else(|| workdir.join(".product"));
    let resolved = match resolve_features(&product_root) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive def-ready --all: {e}");
            return ExitCode::from(2);
        }
    };
    let features = match apply_filter(resolved, args.filter) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dec drive def-ready --all: {e}");
            return ExitCode::from(2);
        }
    };
    if features.is_empty() {
        eprintln!("dec drive def-ready --all: no features match filter");
        return ExitCode::from(2);
    }
    let sweep_input = SweepInput {
        features,
        env_id: args.bench,
        max_iter: args.max_iter.unwrap_or(6),
        per_item_timeout: Duration::from_secs(args.per_feature_timeout),
        goal: Goal::DefReady,
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("dec drive def-ready --all: failed to create async runtime: {e}");
            return ExitCode::from(1);
        }
    };
    let (rows, tally) = match runtime.block_on(run_sweep(workdir.to_path_buf(), sweep_input)) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("dec drive def-ready --all: {e}");
            return ExitCode::from(1);
        }
    };
    let output = render(&rows, &tally, format);
    print!("{output}");
    if tally.done == rows.len() && !rows.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_show(workdir: &Path, args: ShowArgs) -> ExitCode {
    let show_args = ft_113_drive_show::ShowArgs {
        feature_id: args.feature_id,
        bench: args.bench,
        watch: args.watch,
        interval: args.interval,
        since: args.since,
        format: args.format,
        all_drives: args.all_drives,
    };

    ft_113_drive_show::run(workdir, show_args)
}
