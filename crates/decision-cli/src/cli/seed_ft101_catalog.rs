//! `dec _seed-ft101-catalog` — hidden helper that seeds the FT-101
//! `CapabilityReference` + `OntologyDescription` catalog so the
//! verify-graph-author bundle assembler's enrichment fields stop arriving
//! empty. FT-107 follow-up.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::bootstrap::ft101_catalog::{deactivate_role_binding, seed_ft101_catalog_with};
use decision_cli::core::bootstrap::Ft101SeedReport;

#[derive(Debug, clap::Args)]
pub struct SeedFt101CatalogArgs {
    /// Also deactivate a prior role-binding IRI by flipping its
    /// `dec:active` literal to `"false"` through the writer. Use after
    /// `dec _bootstrap-catalog` bumps a binding to a new version, since
    /// the bootstrap leaves the prior version active and trips the
    /// uniqueness invariant.
    #[arg(long, value_name = "IRI")]
    pub deactivate_binding: Option<String>,
    /// Overwrite existing CapabilityReference / OntologyDescription
    /// artifacts in the catalog graph. Default is idempotent (skip
    /// existing IRIs). Use after the baked-in CR bodies have been
    /// corrected and the prior catalog content is stale.
    #[arg(long)]
    pub force: bool,
}

pub fn run(workdir: &Path, args: SeedFt101CatalogArgs) -> ExitCode {
    if let Some(iri) = args.deactivate_binding.as_deref() {
        match deactivate_role_binding(workdir, iri) {
            Ok(true) => println!("deactivated binding {iri}"),
            Ok(false) => println!("binding {iri} had no active=true quad; nothing to do"),
            Err(err) => {
                eprintln!("dec _seed-ft101-catalog: {err}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }
    match seed_ft101_catalog_with(workdir, args.force) {
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
