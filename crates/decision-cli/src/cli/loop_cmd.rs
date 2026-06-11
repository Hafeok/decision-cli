//! `dec loop` subcommand surface — operator-facing audit-trail views
//! over the verify → re-fix loop (FT-109).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::loop_inspect::list::StateFilter;
use decision_cli::loop_inspect::{
    ListArgs as InspectListArgs, OutputFormat, ShowArgs as InspectShowArgs,
};

#[derive(Debug, Subcommand)]
pub enum LoopCmd {
    /// Walk the chronological chain of every defect feedback for a feature.
    Show(LoopShowArgs),
    /// Roll-up across all features by open/closed defect-feedback count.
    List(LoopListArgs),
}

#[derive(Debug, clap::Args)]
pub struct LoopShowArgs {
    /// Feature id (e.g. `FT-019`).
    pub feature_id: String,
    /// Output format: `text` (default) or `json`.
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Override the product-cli root (default: walk up from `--workdir`).
    #[arg(long, value_name = "PATH")]
    pub product_root: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct LoopListArgs {
    /// Which feedback states to include: `open` (default), `closed`, or `all`.
    #[arg(long, default_value = "open")]
    pub state: String,
    /// Output format: `text` (default) or `json`.
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Override the product-cli root (default: walk up from `--workdir`).
    #[arg(long, value_name = "PATH")]
    pub product_root: Option<PathBuf>,
}

pub fn run(workdir: &Path, cmd: LoopCmd) -> ExitCode {
    match cmd {
        LoopCmd::Show(args) => {
            let format = match OutputFormat::parse(&args.format) {
                Some(f) => f,
                None => {
                    eprintln!(
                        "dec loop show: unknown --format {:?}; use text or json",
                        args.format
                    );
                    return ExitCode::from(1);
                }
            };
            decision_cli::loop_inspect::run_show(
                workdir,
                &InspectShowArgs {
                    feature_id: args.feature_id,
                    product_root: args.product_root,
                    format,
                },
            )
        }
        LoopCmd::List(args) => {
            let state = match StateFilter::parse(&args.state) {
                Some(s) => s,
                None => {
                    eprintln!(
                        "dec loop list: unknown --state {:?}; use open, closed, or all",
                        args.state
                    );
                    return ExitCode::from(1);
                }
            };
            let format = match OutputFormat::parse(&args.format) {
                Some(f) => f,
                None => {
                    eprintln!(
                        "dec loop list: unknown --format {:?}; use text or json",
                        args.format
                    );
                    return ExitCode::from(1);
                }
            };
            decision_cli::loop_inspect::run_list(
                workdir,
                &InspectListArgs {
                    state,
                    product_root: args.product_root,
                    format,
                },
            )
        }
    }
}
