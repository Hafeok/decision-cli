//! `dec preflight FT-XXX` — graph-sourced coverage report (FT-052).
//!
//! Reads `.product/graph/index.ttl` and emits the three-section
//! report defined in [`decision_cli::preflight`]. Exit 0 when there
//! are no blocking gaps; exit 1 otherwise.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;

use decision_cli::preflight;

/// Args for `dec preflight FT-XXX`.
#[derive(Debug, Args)]
pub struct PreflightArgs {
    /// Feature id, e.g. `FT-007`. The case is preserved as given.
    pub feature_id: String,
}

/// Entry point used by `main.rs`.
pub fn run(workdir: &Path, args: PreflightArgs) -> ExitCode {
    match preflight::run(workdir, &args.feature_id) {
        Ok(report) => {
            print!("{}", report.render());
            if report.has_blocking_gaps() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            eprintln!("dec preflight {}: {err}", args.feature_id);
            ExitCode::from(1)
        }
    }
}
