//! `dec _sparql` — hidden helper that runs a SPARQL query against the
//! persisted orchestration store.

use std::path::Path;
use std::process::ExitCode;

use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

#[derive(Debug, clap::Args)]
pub struct SparqlArgs {
    /// SPARQL query text.
    #[arg(long)]
    pub query: String,
}

pub fn run(workdir: &Path, args: SparqlArgs) -> ExitCode {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    if !dump_path.exists() {
        eprintln!("no orchestration store at {}", dump_path.display());
        return ExitCode::from(1);
    }
    let bytes = match std::fs::read(&dump_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", dump_path.display());
            return ExitCode::from(1);
        }
    };
    let store = match Store::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("store: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = store.load_from_reader(RdfFormat::NQuads, bytes.as_slice()) {
        eprintln!("load: {e}");
        return ExitCode::from(1);
    }
    match store.query(args.query.as_str()) {
        Ok(QueryResults::Solutions(sols)) => {
            for sol in sols {
                let Ok(sol) = sol else { continue };
                let mut row = Vec::new();
                for (var, term) in sol.iter() {
                    row.push(format!("?{}={}", var.as_str(), term));
                }
                println!("{}", row.join("\t"));
            }
            ExitCode::SUCCESS
        }
        Ok(QueryResults::Boolean(b)) => {
            println!("{b}");
            ExitCode::SUCCESS
        }
        Ok(QueryResults::Graph(quads)) => {
            for q in quads {
                let Ok(q) = q else { continue };
                println!("{q}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("query: {e}");
            ExitCode::from(1)
        }
    }
}
