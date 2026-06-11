//! CLI adapter for `dec _retract-orphan-defects`.

use std::path::Path;
use std::process::ExitCode;

use super::pipeline::{
    retract_orphans_all, retract_orphans_for_feature, retract_orphans_for_graph, RetractSummary,
};

/// Mutually-exclusive scope for the CLI command.
#[derive(Debug, Clone)]
pub enum CliScope<'a> {
    /// `--graph VG-NNN`
    Graph(&'a str),
    /// `--feature FT-NNN`
    Feature(&'a str),
    /// `--all`
    All,
}

/// Execute the `_retract-orphan-defects` diagnostic command.
pub fn run(workdir: &Path, scope: CliScope<'_>, dry_run: bool) -> ExitCode {
    let result = match scope {
        CliScope::Graph(vg) => retract_orphans_for_graph(workdir, vg, dry_run),
        CliScope::Feature(ft) => retract_orphans_for_feature(workdir, ft, dry_run),
        CliScope::All => retract_orphans_all(workdir, dry_run),
    };

    match result {
        Ok(summary) => {
            report(&summary, dry_run);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(2)
        }
    }
}

fn report(summary: &RetractSummary, dry_run: bool) {
    let verb = if dry_run {
        "would retract"
    } else {
        "retracted"
    };

    if summary.per_graph.is_empty() {
        println!("No orphan defects found.");
        return;
    }

    for tally in &summary.per_graph {
        if tally.retracted == 0 && tally.candidates.is_empty() {
            println!("{}: 0", tally.vg_id);
            continue;
        }
        println!("{}: {} {}", tally.vg_id, tally.retracted, verb);
        for cand in &tally.candidates {
            println!(
                "  - {} (TC: {})",
                cand.feedback_iri.as_str(),
                short(&cand.tc_iri)
            );
        }
    }

    println!("total: {} {}", summary.total, verb);
}

fn short(iri: &str) -> String {
    iri.rsplit('/').next().unwrap_or(iri).to_string()
}
