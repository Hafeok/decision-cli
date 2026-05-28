//! `dec _sparql` — hidden helper that runs SPARQL against the persisted store.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::store::open_orchestration_store;
use oxigraph::sparql::QueryResults;

#[derive(Debug, clap::Args)]
pub struct SparqlArgs {
    /// SPARQL query text.
    #[arg(long)]
    pub query: String,
}

pub fn run(workdir: &Path, args: SparqlArgs) -> ExitCode {
    let store = match open_orchestration_store(workdir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e:#}");
            return ExitCode::from(1);
        }
    };
    match store.query(args.query.as_str()) {
        Ok(results) => print_query_results(results),
        Err(e) => {
            eprintln!("query: {e}");
            ExitCode::from(1)
        }
    }
}

fn print_query_results(results: QueryResults) -> ExitCode {
    match results {
        QueryResults::Solutions(sols) => {
            for sol in sols {
                let Ok(sol) = sol else { continue };
                let mut row = Vec::new();
                for (var, term) in sol.iter() {
                    row.push(format!("?{}={}", var.as_str(), term));
                }
                println!("{}", row.join("\t"));
            }
        }
        QueryResults::Boolean(b) => {
            println!("{b}");
        }
        QueryResults::Graph(quads) => {
            for q in quads {
                let Ok(q) = q else { continue };
                println!("{q}");
            }
        }
    }
    ExitCode::SUCCESS
}
