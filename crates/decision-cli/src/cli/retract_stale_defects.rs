//! CLI for `dec _retract-stale-defects` (FT-116 diagnostic).

use std::path::Path;
use std::process::ExitCode;

use clap::Args;

use decision_cli::features::ft_116_retract_stale_defects;

/// Arguments for `_retract-stale-defects` command.
#[derive(Debug, Args)]
pub struct RetractStaleDefectsArgs {
    /// Graph ID (e.g., VG-123) to process.
    #[arg(long, value_name = "VG-NNN")]
    pub graph: String,

    /// Dry-run mode: show what would be closed without writing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Run the retract-stale-defects command.
pub fn run(workdir: &Path, args: RetractStaleDefectsArgs) -> ExitCode {
    ft_116_retract_stale_defects::cli::run(workdir, &args.graph, args.dry_run)
}
