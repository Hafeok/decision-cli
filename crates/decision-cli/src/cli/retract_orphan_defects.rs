//! CLI for `dec _retract-orphan-defects` (FT-120 diagnostic).

use std::path::Path;
use std::process::ExitCode;

use clap::{ArgGroup, Args};

use decision_cli::features::ft_120_retract_orphan_defects;
use decision_cli::features::ft_120_retract_orphan_defects::cli::CliScope;

/// Arguments for `_retract-orphan-defects` command.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("scope")
        .required(true)
        .args(["graph", "feature", "all"])
))]
pub struct RetractOrphanDefectsArgs {
    /// Graph ID (e.g., `VG-088`) to scope the sweep to.
    #[arg(long, value_name = "VG-NNN")]
    pub graph: Option<String>,

    /// Feature ID (e.g., `FT-100`) — sweeps all VGs linked to the feature.
    #[arg(long, value_name = "FT-NNN")]
    pub feature: Option<String>,

    /// Sweep every graph in the store.
    #[arg(long)]
    pub all: bool,

    /// Dry-run mode: list candidates without writing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Run the retract-orphan-defects command.
pub fn run(workdir: &Path, args: RetractOrphanDefectsArgs) -> ExitCode {
    let scope = if let Some(vg) = args.graph.as_deref() {
        CliScope::Graph(vg)
    } else if let Some(ft) = args.feature.as_deref() {
        CliScope::Feature(ft)
    } else if args.all {
        CliScope::All
    } else {
        // ArgGroup ensures unreachable; defend in depth.
        eprintln!("Error: one of --graph, --feature, or --all is required");
        return ExitCode::from(2);
    };

    ft_120_retract_orphan_defects::cli::run(workdir, scope, args.dry_run)
}
