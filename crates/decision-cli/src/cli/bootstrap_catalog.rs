//! `dec _bootstrap-catalog` — hidden helper that runs FT-058 catalog
//! bootstrap against an already-initialised orchestration store.
//!
//! The authoritative entry point per ADR-036 / PRD §10.7 is
//! `scripts/bootstrap_catalog.py`; that wrapper shells out to this
//! subcommand so SHACL validation and the atomic transaction stay in
//! Rust (the writer chokepoint is here, not in Python).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_cli::core::bootstrap::{bootstrap_catalog, BootstrapError, BootstrapReport};

#[derive(Debug, clap::Args)]
pub struct BootstrapCatalogArgs {
    /// Path to `config/capabilities.yaml`.
    #[arg(long, value_name = "PATH")]
    pub capabilities: PathBuf,
    /// Path to `config/role-bindings.yaml`.
    #[arg(long, value_name = "PATH")]
    pub bindings: PathBuf,
    /// Also backfill `dec:stakes` and session token-breakdown fields.
    #[arg(long)]
    pub migrate: bool,
    /// Validate and plan but do not write back to the store.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(workdir: &Path, args: BootstrapCatalogArgs) -> ExitCode {
    let result = bootstrap_catalog(
        workdir,
        &args.capabilities,
        &args.bindings,
        args.migrate,
        args.dry_run,
    );
    match result {
        Ok(report) => {
            print_success(&report);
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_error(&err);
            ExitCode::from(1)
        }
    }
}

fn print_success(report: &BootstrapReport) {
    if report.is_unchanged() {
        println!("catalog: unchanged");
    } else {
        println!(
            "catalog: {cw} capabilities ({cn} new), {bw} role bindings ({bn} new), 0 divergent",
            cw = report.capabilities_written + report.capabilities_skipped,
            cn = report.capabilities_written,
            bw = report.bindings_written + report.bindings_skipped,
            bn = report.bindings_written,
        );
    }
    if report.bundles_migrated > 0 || report.sessions_migrated > 0 {
        println!(
            "migrations: {} bundles, {} sessions backfilled",
            report.bundles_migrated, report.sessions_migrated,
        );
    }
    println!(
        "source hashes: capabilities={} bindings={}",
        &report.capabilities_source_hash[..12],
        &report.bindings_source_hash[..12]
    );
}

fn print_error(err: &BootstrapError) {
    eprintln!("dec _bootstrap-catalog: {err}");
}
