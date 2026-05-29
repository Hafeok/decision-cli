//! `dec verify bench {new, list, show}` — clap adapter for the env surface.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_bench_list::{self, BenchListRequest, OutputFormat as ListFormat};
use decision_cli::verify_bench_new::{self, BenchNewRequest};
use decision_cli::verify_bench_show::{self, BenchShowRequest, OutputFormat as ShowFormat};

use super::exit_code_for;

#[derive(Debug, Subcommand)]
pub enum BenchCmd {
    /// Create a new VerificationBench (FT-038).
    New(BenchNewArgs),
    /// List VerificationBench artifacts (FT-039).
    List(BenchListArgs),
    /// Show a single VerificationBench in detail (FT-040).
    Show(BenchShowArgs),
}

#[derive(Debug, clap::Args)]
pub struct BenchNewArgs {
    /// Caller-supplied id (e.g. BNCH-007). Omitted → mints the next free BNCH-NNN.
    #[arg(long)]
    pub id: Option<String>,
    /// Bench type tag (e.g. `ephemeral-tempdir`, `remote-http`).
    #[arg(long = "type", value_name = "BNCH-TYPE")]
    pub bench_type: String,
    /// Safety class: `isolated`, `shared-non-destructive`, or `production-readonly`.
    #[arg(long = "safety-class")]
    pub safety_class: String,
    /// Comma-separated operation tokens permitted in the bench (e.g. `shell,filesystem`).
    #[arg(long = "allowed-ops")]
    pub allowed_ops: String,
    /// Optional setup shell snippet.
    #[arg(long)]
    pub setup: Option<String>,
    /// Optional teardown shell snippet.
    #[arg(long)]
    pub teardown: Option<String>,
    /// Required iff `--type` is `remote-*`; forbidden for local types.
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Optional repo-relative path to a fixture tree (FT-053 / ADR-032).
    /// Must point at a directory under the working directory.
    #[arg(long = "fixture-source", value_name = "PATH")]
    pub fixture_source: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct BenchListArgs {
    /// Optional safety-class filter (`isolated`, `shared-non-destructive`,
    /// or `production-readonly`).
    #[arg(long = "safety-class")]
    pub safety_class: Option<String>,
    /// Optional bench-type filter (e.g. `ephemeral-tempdir`, `remote-http`).
    #[arg(long = "type", value_name = "BNCH-TYPE")]
    pub bench_type: Option<String>,
    /// Output format. Defaults to `table`.
    #[arg(long, value_name = "FORMAT", default_value = "table")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct BenchShowArgs {
    /// Identifier of the bench to show (e.g. `BNCH-001-ephemeral-cli`).
    pub id: String,
    /// Output format. Defaults to `text`.
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,
}

/// Convert bench-list clap args into the structured [`BenchListRequest`].
pub fn bench_list_request(
    args: &BenchListArgs,
    workdir: &Path,
) -> Result<BenchListRequest, HandlerError> {
    let format = ListFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
        field: "format".to_string(),
        detail: format!(
            "format must be one of {{table, json}}; got {got:?}",
            got = args.format
        ),
    })?;
    Ok(BenchListRequest {
        safety_class: args.safety_class.clone(),
        bench_type: args.bench_type.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert bench-show clap args into the structured [`BenchShowRequest`].
pub fn bench_show_request(
    args: &BenchShowArgs,
    workdir: &Path,
) -> Result<BenchShowRequest, HandlerError> {
    let format = ShowFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
        field: "format".to_string(),
        detail: format!(
            "format must be one of {{text, json}}; got {got:?}",
            got = args.format
        ),
    })?;
    Ok(BenchShowRequest {
        id: args.id.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert clap args into the structured `BenchNewRequest`.
#[must_use]
pub fn bench_new_request(args: &BenchNewArgs, workdir: &Path) -> BenchNewRequest {
    let allowed_ops: Vec<String> = args
        .allowed_ops
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    BenchNewRequest {
        id: args.id.clone(),
        bench_type: args.bench_type.clone(),
        safety_class: args.safety_class.clone(),
        allowed_ops,
        setup: args.setup.clone(),
        teardown: args.teardown.clone(),
        endpoint: args.endpoint.clone(),
        fixture_source: args.fixture_source.clone(),
        workdir: Some(workdir.to_path_buf()),
    }
}

pub(super) fn run(workdir: &Path, cmd: BenchCmd) -> ExitCode {
    match cmd {
        BenchCmd::New(args) => run_env_new(workdir, args),
        BenchCmd::List(args) => run_env_list(workdir, args),
        BenchCmd::Show(args) => run_env_show(workdir, args),
    }
}

fn run_env_new(workdir: &Path, args: BenchNewArgs) -> ExitCode {
    let req = bench_new_request(&args, workdir);
    match verify_bench_new::run(&req) {
        Ok(outcome) => {
            println!("Created VerificationBench {id}", id = outcome.id);
            println!("  Path: {}", outcome.path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify bench new: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_env_list(workdir: &Path, args: BenchListArgs) -> ExitCode {
    let req = match bench_list_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify bench list: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_bench_list::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                ListFormat::Table => print!("{}", verify_bench_list::render_table(&outcome)),
                ListFormat::Json => println!("{}", verify_bench_list::render_json(&outcome)),
            }
            // TC-096 AC #5: emit a one-line stderr advisory naming each
            // corrupt bench id and its failure mode. The listing itself
            // exits 0 (or 2 on partial success) — stdout stays
            // machine-parseable, stderr carries the human triage hint.
            let warnings = verify_bench_list::render_stderr_warnings(&outcome);
            if !warnings.is_empty() {
                eprint!("{warnings}");
                // Exit 2 to signal "partial success" — the listing
                // completed but at least one row could not be fully
                // projected.
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify bench list: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_env_show(workdir: &Path, args: BenchShowArgs) -> ExitCode {
    let req = match bench_show_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify bench show: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_bench_show::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                ShowFormat::Text => print!("{}", verify_bench_show::render_text(&outcome)),
                ShowFormat::Json => println!("{}", verify_bench_show::render_json(&outcome)),
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify bench show: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}
