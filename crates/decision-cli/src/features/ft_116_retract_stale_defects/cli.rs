//! CLI adapter for `dec _retract-stale-defects`.

use std::path::Path;
use std::process::ExitCode;

use super::pipeline::retract_stale_defects;

/// Execute the `_retract-stale-defects` diagnostic command.
pub fn run(workdir: &Path, graph_id: &str, dry_run: bool) -> ExitCode {
    match retract_stale_defects(workdir, graph_id, dry_run) {
        Ok(result) => {
            if dry_run {
                if result.closed_count == 0 {
                    println!("No stale defects to close for {}", result.vgr_id);
                } else {
                    println!(
                        "Would close {} defect(s) based on {}:",
                        result.closed_count, result.vgr_id
                    );
                    for iri in &result.defect_iris {
                        println!("  - {iri}");
                    }
                }
            } else if result.closed_count == 0 {
                println!("No stale defects closed (already processed or no open defects)");
            } else {
                println!(
                    "Closed {} defect(s) based on {} (evidence retraction)",
                    result.closed_count, result.vgr_id
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
