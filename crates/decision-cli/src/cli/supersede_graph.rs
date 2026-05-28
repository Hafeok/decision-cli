//! `dec _supersede-graph` — hidden operator helper to mark a
//! VerificationGraph as superseded by another. Mirrors the ADR-024
//! lifecycle the verify-graph-author will use for cross-feature
//! graph design replacements once the worker learns to emit
//! supersession proposals.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::verify::supersede::supersede_graph;

#[derive(Debug, clap::Args)]
pub struct SupersedeGraphArgs {
    /// Short id of the graph being superseded (e.g. `VG-054`).
    pub old: String,
    /// Short id of the replacement graph (e.g. `VG-200`).
    #[arg(long)]
    pub by: String,
}

const VG_IRI_PREFIX: &str = "https://decision-cli.dev/ns/graph/";

pub fn run(workdir: &Path, args: SupersedeGraphArgs) -> ExitCode {
    let old_iri = expand(&args.old);
    let new_iri = expand(&args.by);
    match supersede_graph(workdir, &old_iri, &new_iri) {
        Ok(()) => {
            println!("superseded {old} → {new}", old = args.old, new = args.by);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec _supersede-graph: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn expand(short_or_iri: &str) -> String {
    if short_or_iri.starts_with("http://") || short_or_iri.starts_with("https://") {
        short_or_iri.to_string()
    } else {
        format!("{VG_IRI_PREFIX}{short_or_iri}")
    }
}
