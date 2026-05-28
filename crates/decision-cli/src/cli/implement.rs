//! `dec implement` — slice-1 implementer dispatch (FT-011 + FT-013).
//!
//! FT-047 / ADR-031: every feature-targeted dispatch passes through the
//! chain-integrity gate before any session is opened. The CLI maps the
//! gate's structured errors to the exit codes spec'd by the TC:
//!
//!   * `Error::ChainIntegrity` → exit 1 with the actionable message.
//!   * `Error::InvalidArgument(waiver.reason)` → exit 2.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_cli::core::verify::WaiverIntent;
use decision_cli::finalize::FinalizeOutcome;
use decision_cli::implement::{self, ImplementArgs};

#[derive(Debug, clap::Args)]
pub struct ImplementCmdArgs {
    /// Feature id (e.g. `FT-007`).
    pub feature: String,
    /// Override the workspace directory the worker writes into.
    #[arg(long)]
    pub workspace: Option<PathBuf>,
    /// Override the path to product-cli's root (defaults to walking up
    /// from the working dir looking for `.product/`).
    #[arg(long)]
    pub product_root: Option<PathBuf>,
    /// Override the worker command (shell-quoted). Defaults to
    /// `code-writer` on $PATH, falling back to `python3 -m code_writer.main`.
    #[arg(long)]
    pub worker: Option<String>,
    /// Bundle assembly depth handed to `product context`.
    #[arg(long, default_value_t = 1)]
    pub bundle_depth: usize,
    /// FT-047 / ADR-031: override the chain-integrity gate when the
    /// target feature's TCs are not fully covered. The reason must be
    /// at least 16 non-whitespace characters and is persisted as a
    /// `dec:CoverageWaiver` artifact.
    #[arg(long, value_name = "REASON")]
    pub waive_coverage: Option<String>,
}

pub fn run(workdir: &Path, args: ImplementCmdArgs) -> ExitCode {
    let implement_args = build_implement_args(args);
    match implement::run(workdir, &implement_args) {
        Ok(outcome) => {
            print_implement_success(&implement_args, &outcome);
            ExitCode::SUCCESS
        }
        Err(err) => {
            let rendered = format!("{err:#}");
            // FT-047 §Error handling: bad waiver reason maps to exit 2.
            // The gate's error type renders the `Error::InvalidArgument`
            // prefix; we detect it on the rendered chain so the mapping
            // does not depend on downcasting through `anyhow`.
            if rendered.contains("Error::InvalidArgument") && rendered.contains("waiver.reason") {
                eprintln!("dec implement failed: {rendered}");
                return ExitCode::from(2);
            }
            eprintln!("dec implement failed: {rendered}");
            ExitCode::from(1)
        }
    }
}

fn build_implement_args(args: ImplementCmdArgs) -> ImplementArgs {
    let mut implement_args = ImplementArgs::new(args.feature);
    implement_args.workspace = args.workspace;
    implement_args.product_root = args.product_root;
    implement_args.worker_command = args.worker;
    implement_args.bundle_depth = args.bundle_depth;
    implement_args.waiver = args.waive_coverage.map(WaiverIntent::new);
    implement_args
}

fn print_implement_success(implement_args: &ImplementArgs, outcome: &implement::ImplementOutcome) {
    println!("dec implement: success");
    println!("  Feature:        {}", implement_args.feature_id);
    println!("  Session:        {}", outcome.session_iri);
    println!("  Dispatch:       {}", outcome.dispatch_iri);
    println!("  CodeChange:     {}", outcome.code_change_iri);
    let short = &outcome.bundle_hash[..outcome.bundle_hash.len().min(12)];
    println!("  Bundle hash:    sha256:{short}…");
    println!("  Workspace:      {}", outcome.workspace_dir.display());
    println!(
        "  product graph:  {}",
        outcome.product_codechange_path.display()
    );
    for f in &outcome.files_written {
        println!("  wrote:          {}", f.display());
    }
    println!(
        "  Worker:         status={} turns={} latency={:.3}s",
        outcome.worker_status, outcome.turn_count, outcome.latency_seconds
    );
    if let Some(fin) = &outcome.finalize {
        print_finalize_section(&implement_args.feature_id, fin);
    }
}

fn print_finalize_section(feature_id: &str, fin: &FinalizeOutcome) {
    match &fin.commit_sha {
        Some(sha) => println!("  Commit:         {sha}"),
        None => println!("  Commit:         (no working-tree changes)"),
    }
    if fin.status_transitioned {
        println!("  Status:         {feature_id} → complete");
    } else {
        println!("  Status:         {feature_id} → (not transitioned)");
    }
    for note in &fin.notes {
        println!("  Note:           {note}");
    }
}
