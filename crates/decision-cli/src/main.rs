//! `dec` — decision-cli binary entry point.
//!
//! Slice 1 surface: init, status, health, implement, events, session.
//! See ADR-011 (CLI shape) and FT-012.
//!
//! FT-006 lands `dec init` end-to-end against the embedded ontology
//! (TC-001 happy-path, TC-003 SHACL-fail path). Later features add
//! the remaining subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use decision_cli::init::{self, DefinitionSource, InitError};

#[derive(Debug, Parser)]
#[command(
    name = "dec",
    about = "decision-cli — orchestration system for Decision-Driven Design",
    version
)]
struct Cli {
    /// Override the working directory (defaults to CWD; ADR-012 walk-up
    /// not implemented yet).
    #[arg(long, global = true)]
    workdir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialise the orchestration store from a ValueStream definition.
    Init(InitArgs),
    /// Hidden helper for tests/CI: run a SPARQL query against the
    /// persisted orchestration store.
    #[command(name = "_sparql", hide = true)]
    Sparql(SparqlArgs),
}

#[derive(Debug, clap::Args)]
struct InitArgs {
    /// Initialise from a bundled stream template by name.
    #[arg(long, group = "src")]
    template: Option<String>,
    /// Initialise from a local Turtle definition file.
    #[arg(long, group = "src", value_name = "PATH")]
    from: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
struct SparqlArgs {
    /// SPARQL query text.
    #[arg(long)]
    query: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let workdir = cli
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match cli.command {
        Command::Init(args) => run_init(&workdir, args),
        Command::Sparql(args) => run_sparql(&workdir, args),
    }
}

fn run_init(workdir: &std::path::Path, args: InitArgs) -> ExitCode {
    let source = match (args.template, args.from) {
        (Some(t), None) => DefinitionSource::Template(t),
        (None, Some(p)) => DefinitionSource::File(p),
        (Some(_), Some(_)) => {
            eprintln!("dec init: pass exactly one of --template or --from, not both");
            return ExitCode::from(2);
        }
        (None, None) => {
            eprintln!(
                "dec init: a definition reference is required.\n  \
                 Try: dec init --template engineering-development\n  \
                 Or:  dec init --from ./streams/decision-cli-development.ttl"
            );
            return ExitCode::from(2);
        }
    };

    match init::run(workdir, source) {
        Ok(outcome) => {
            println!(
                "Initialised orchestration store in {}",
                outcome.store_dir.display()
            );
            println!("  ValueStream:       {}", outcome.stream_iri);
            println!("  ValueAction:       {}", outcome.value_action_iri);
            println!("  Bootstrap session: {}", outcome.session_iri);
            let short = &outcome.definition_hash[..outcome.definition_hash.len().min(12)];
            println!(
                "  Definition source: {} (sha256:{short}…)",
                outcome.definition_source
            );
            println!("  Ontology version:  {}", outcome.ontology_version);
            println!(
                "  Authorized goals:  {}",
                outcome.authorized_goals.join(", ")
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_init_error(&err);
            ExitCode::from(1)
        }
    }
}

fn print_init_error(err: &InitError) {
    eprintln!("dec init failed: {err}");
    match err {
        InitError::ShaclViolation { report, .. } => {
            eprintln!("\nSHACL violations:\n{report}");
        }
        InitError::UnknownValueAction { iri, available } => {
            eprintln!(
                "\nUnresolvable ValueAction URI: <{iri}>.\nBundled URIs available: {available}"
            );
        }
        InitError::UnauthorizedGoal {
            goal,
            value_action,
            compatible,
        } => {
            eprintln!(
                "\nThis stream pursues <{value_action}>; `{goal}` is not in its compatible-goals set ({compatible})."
            );
        }
        _ => {}
    }
}

fn run_sparql(workdir: &std::path::Path, args: SparqlArgs) -> ExitCode {
    use oxigraph::io::RdfFormat;
    use oxigraph::sparql::QueryResults;
    use oxigraph::store::Store;

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
