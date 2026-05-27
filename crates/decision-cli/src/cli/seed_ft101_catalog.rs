//! `dec _seed-ft101-catalog` — hidden helper that seeds the FT-101
//! `CapabilityReference` + `OntologyDescription` catalog so the
//! verify-graph-author bundle assembler's enrichment fields stop arriving
//! empty. FT-107 follow-up.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::bootstrap::{seed_ft101_catalog, Ft101SeedReport};

#[derive(Debug, clap::Args)]
pub struct SeedFt101CatalogArgs {}

pub fn run(workdir: &Path, _args: SeedFt101CatalogArgs) -> ExitCode {
    match seed_ft101_catalog(workdir) {
        Ok(report) => {
            print_success(&report);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec _seed-ft101-catalog: {err}");
            ExitCode::from(1)
        }
    }
}

fn print_success(report: &Ft101SeedReport) {
    println!(
        "catalog: {written} capability references written, {skipped} skipped (already present); ontology: {ont}",
        written = report.capabilities_written,
        skipped = report.capabilities_skipped,
        ont = if report.ontology_written {
            "written"
        } else {
            "already present"
        },
    );
}
