//! `dec _supersede-misrouted-defects` — one-shot cleanup for legacy
//! implementer-targeted defect feedback that was misclassified before
//! the TC-199 verdict-aggregator demotion logic landed.
//!
//! Hidden subcommand (operator one-shot, not part of the routine
//! surface). Idempotent; safe to re-run.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::feedback::supersede_misrouted::{
    supersede_misrouted_implementer_defects, SupersedeReport,
};

#[derive(Debug, clap::Args)]
pub struct SupersedeMisroutedArgs {
    /// Report what would be superseded without writing any quads.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(workdir: &Path, args: SupersedeMisroutedArgs) -> ExitCode {
    match supersede_misrouted_implementer_defects(workdir, args.dry_run) {
        Ok(report) => {
            print_report(&report, args.dry_run);
            if report.errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            eprintln!("dec _supersede-misrouted-defects: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn print_report(report: &SupersedeReport, dry_run: bool) {
    let verb = if dry_run { "would supersede" } else { "superseded" };
    println!(
        "{verb} {n} of {matched} matched defect feedback(s) (scanned {scanned})",
        n = if dry_run { report.matched } else { report.superseded },
        matched = report.matched,
        scanned = report.scanned,
    );
    for err in &report.errors {
        eprintln!("  ! {err}");
    }
}
