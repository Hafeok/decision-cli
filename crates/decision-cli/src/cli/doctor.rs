//! `dec doctor` — operator-facing worker preflight audit (FT-016 / TC-047 / TC-048).
//!
//! Authoritative on-demand audit. Unlike `dec init`, the exit code
//! reflects whether every required worker resolved (zero on all-ok,
//! 2 on any-missing). Read-only — never writes to the store, never
//! writes to the working tree, never invokes a worker with stdin.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::worker;

/// Output formats supported by `dec doctor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DoctorFormat {
    /// Human-readable fixed-width table.
    Text,
    /// Single-document JSON report (TC-048).
    Json,
}

impl Default for DoctorFormat {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, clap::Args)]
pub struct DoctorArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = DoctorFormat::Text)]
    pub format: DoctorFormat,
    /// Restrict to a single role.
    #[arg(long)]
    pub role: Option<String>,
}

pub fn run(workdir: &Path, args: DoctorArgs) -> ExitCode {
    if let Some(filter) = args.role.as_deref() {
        if worker::role_entry(filter).is_none() {
            eprintln!(
                "dec doctor: unknown role '{filter}'. Manifest roles: {}",
                manifest_role_list()
            );
            return ExitCode::from(2);
        }
    }

    let report = worker::build_report(
        worker::ACTIVE_ROLES_ENGINEERING_DEVELOPMENT,
        Some(workdir),
        None,
        args.role.as_deref(),
    );
    match args.format {
        DoctorFormat::Text => {
            print!("{}", worker::format_report_text(&report));
        }
        DoctorFormat::Json => {
            // Single JSON document, single trailing newline.
            println!("{}", worker::format_report_json(&report));
        }
    }
    if report.is_all_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn manifest_role_list() -> String {
    worker::MANIFEST
        .iter()
        .map(|e| e.role)
        .collect::<Vec<_>>()
        .join(", ")
}
