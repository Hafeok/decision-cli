//! `dec migrate provenance` — thin CLI adapter (FT-074).
//!
//! Parses operator-supplied flags, normalises them onto the
//! `ProvenanceArgs` shape, and delegates into
//! [`crate::features::ft_074_migrate_provenance::commands::run`].

#![allow(missing_docs)]

use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Subcommand};

use decision_cli::migrate_provenance::commands::{ProvenanceArgs, ProvenanceMode};

#[derive(Debug, Subcommand)]
pub enum MigrateCmd {
    /// Migrate the existing artifact corpus to dual-provenance
    /// conformance (FT-074 / ADR-042).
    Provenance(ProvenanceCli),
}

#[derive(Debug, Args)]
pub struct ProvenanceCli {
    /// Audit-only mode — classify every artifact, plan backfills /
    /// orphan flagging, but do not commit or write a report file.
    #[arg(long, conflicts_with_all = ["apply", "cutover"])]
    pub dry_run: bool,
    /// Apply mode — commit backfills, emit orphan feedback, write the
    /// report file. Mutually exclusive with `--dry-run` and `cutover`.
    #[arg(long, conflicts_with_all = ["dry_run", "cutover"])]
    pub apply: bool,
    /// Cutover sub-mode — flip GraphWriter from warn-only to reject
    /// mode if the orphan count is at or below the threshold.
    #[arg(long, conflicts_with_all = ["dry_run", "apply"])]
    pub cutover: bool,
    /// Maximum acceptable orphan count for `cutover`. Defaults to 0
    /// (operator can override per FT-074 §Behaviour step 6).
    #[arg(long, default_value_t = 0)]
    pub cutover_threshold: usize,
}

pub fn run(workdir: &Path, cmd: MigrateCmd) -> ExitCode {
    match cmd {
        MigrateCmd::Provenance(cli) => run_provenance(workdir, cli),
    }
}

fn run_provenance(workdir: &Path, cli: ProvenanceCli) -> ExitCode {
    let mode = if cli.cutover {
        ProvenanceMode::Cutover
    } else if cli.apply {
        ProvenanceMode::Apply
    } else {
        // Default mode (no flag specified) is dry-run — safer.
        ProvenanceMode::DryRun
    };
    let args = ProvenanceArgs {
        mode,
        cutover_threshold: cli.cutover_threshold,
    };
    decision_cli::migrate_provenance::commands::run(workdir, args)
}
