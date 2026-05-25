//! CLI handler for `dec migrate provenance` (FT-074 §Outputs).
//!
//! Parses operator-supplied flags, loads the orchestration store from
//! the discovered `.dec/`, dispatches to the migration orchestrator
//! ([`super::run_migration`]) or the cutover routine ([`super::run_cutover`]),
//! and persists results.

#![allow(missing_docs)]

use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use oxigraph::store::Store;

use crate::core::store::{
    load_store_from_dump, open_orchestration_store, orchestration_dump_path, persist_store,
};

use super::{run_cutover, run_migration, write_report, MigrateArgs};

/// Parsed operator flags forwarded by the CLI adapter.
#[derive(Debug, Clone)]
pub struct ProvenanceArgs {
    pub mode: ProvenanceMode,
    pub cutover_threshold: usize,
}

/// Selector for the three migration sub-modes per FT-074 §Outputs.
#[derive(Debug, Clone)]
pub enum ProvenanceMode {
    /// `--dry-run`: audit + plan only, no writes, no report file.
    DryRun,
    /// `--apply`: audit + apply backfills + emit orphan feedback +
    /// write the report file.
    Apply,
    /// `cutover`: gate on orphan count, flip warn-only mode to false.
    Cutover,
}

/// Outcome of the command handler. Public so the CLI adapter can
/// optionally render it before the process exits.
#[derive(Debug, Clone)]
pub struct ProvenanceOutcome {
    pub message: String,
}

/// Top-level CLI entry. Returns process exit code mirroring FT-074's
/// "cutover requested while orphan count > threshold → command exits 1"
/// invariant.
pub fn run(workdir: &Path, args: ProvenanceArgs) -> ExitCode {
    match dispatch(workdir, args) {
        Ok(outcome) => {
            println!("{}", outcome.message);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec migrate provenance: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn dispatch(workdir: &Path, args: ProvenanceArgs) -> Result<ProvenanceOutcome> {
    match args.mode {
        ProvenanceMode::DryRun => run_dry_run(workdir, &args),
        ProvenanceMode::Apply => run_apply(workdir, &args),
        ProvenanceMode::Cutover => run_cutover_mode(workdir, &args),
    }
}

fn run_dry_run(workdir: &Path, args: &ProvenanceArgs) -> Result<ProvenanceOutcome> {
    let store = open_orchestration_store(workdir)?;
    let migrate_args = build_migrate_args(args, /*dry_run=*/ true);
    let report = run_migration(&store, &migrate_args)?;
    Ok(ProvenanceOutcome {
        message: format!(
            "dry-run complete: {} total / {} conformant / {} backfillable / {} orphan",
            report.summary.total,
            report.summary.conformant,
            report.summary.backfilled,
            report.summary.orphan
        ),
    })
}

fn run_apply(workdir: &Path, args: &ProvenanceArgs) -> Result<ProvenanceOutcome> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)?;
    let migrate_args = build_migrate_args(args, /*dry_run=*/ false);
    let report = run_migration(&store, &migrate_args)?;
    let report_path = write_report(workdir, &report)?;
    persist_store(&store, &dump)?;
    Ok(ProvenanceOutcome {
        message: format!(
            "migration applied: {} backfilled / {} orphan; report at {}",
            report.summary.backfilled,
            report.summary.orphan,
            report_path.display()
        ),
    })
}

fn run_cutover_mode(workdir: &Path, args: &ProvenanceArgs) -> Result<ProvenanceOutcome> {
    let dump = orchestration_dump_path(workdir);
    let store: Store = load_store_from_dump(&dump)?;
    let outcome = run_cutover(&store, args.cutover_threshold)?;
    persist_store(&store, &dump)?;
    Ok(ProvenanceOutcome {
        message: format!(
            "cutover complete: {} orphan(s) remaining (threshold {}); warn-only={}, flipped={}",
            outcome.orphan_count,
            args.cutover_threshold,
            outcome.warn_only_after,
            outcome.flipped
        ),
    })
}

fn build_migrate_args(args: &ProvenanceArgs, dry_run: bool) -> MigrateArgs {
    let now = chrono::Utc::now();
    let timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let run_id = format!("run-{}", now.format("%Y%m%dT%H%M%SZ"));
    MigrateArgs {
        run_id,
        fallback_timestamp: timestamp.clone(),
        external_origin: format!("FT-074 provenance migration tool run at {timestamp}"),
        cutover_threshold: args.cutover_threshold,
        dry_run,
    }
}
