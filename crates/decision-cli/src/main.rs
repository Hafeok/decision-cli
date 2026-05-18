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
use decision_cli::bundled;
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
    /// Display the active value stream's identity and bootstrap provenance.
    Status,
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
        Command::Status => run_status(&workdir),
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

/// `dec status` — read the persisted orchestration store and surface the
/// active value stream's identity, definition provenance, terminal
/// ValueAction, authorized goals, and store path (FT-012 / TC-006).
///
/// All inputs are recorded by `dec init` (FT-008 / ADR-006) and reachable
/// via PROV-O lineage from the bootstrap session (ADR-004). The display
/// shape mirrors `decision-cli-slice-1-bounds.md` §3.7.
fn run_status(workdir: &std::path::Path) -> ExitCode {
    use oxigraph::io::RdfFormat;
    use oxigraph::sparql::QueryResults;
    use oxigraph::store::Store;

    let dec_dir = workdir.join(".dec");
    if !dec_dir.exists() {
        eprintln!(
            "dec status: not inside an initialised decision-cli working dir.\n  \
             Run `dec init --template engineering-development` first."
        );
        return ExitCode::from(1);
    }
    let dump_path = dec_dir.join("store").join("orchestration.nq");
    let bytes = match std::fs::read(&dump_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("dec status: failed to read {}: {e}", dump_path.display());
            return ExitCode::from(1);
        }
    };
    let store = match Store::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("dec status: failed to open store: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = store.load_from_reader(RdfFormat::NQuads, bytes.as_slice()) {
        eprintln!("dec status: failed to load store: {e}");
        return ExitCode::from(1);
    }

    // -- Stream identity: name + terminal ValueAction.
    let stream_q = "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?stream ?name ?action WHERE {
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  OPTIONAL { ?stream dec:name ?name }
} LIMIT 1";
    let (stream_iri, stream_name, action_iri) = match store.query(stream_q) {
        Ok(QueryResults::Solutions(mut sols)) => match sols.next() {
            Some(Ok(sol)) => {
                let stream = sol
                    .get("stream")
                    .map(term_iri_string)
                    .unwrap_or_else(|| "(unknown)".into());
                let name = sol
                    .get("name")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let action = sol
                    .get("action")
                    .map(term_iri_string)
                    .unwrap_or_else(|| "(unknown)".into());
                (stream, name, action)
            }
            _ => {
                eprintln!("dec status: no dec:ValueStream artifact in the store");
                return ExitCode::from(1);
            }
        },
        Ok(_) => {
            eprintln!("dec status: unexpected SPARQL result shape for stream lookup");
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("dec status: SPARQL error: {e}");
            return ExitCode::from(1);
        }
    };

    // -- Authorized goals: order-preserving collection.
    let goals_q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?goal WHERE {{ <{stream_iri}> dec:authorizedGoals ?goal }}"
    );
    let mut authorized_goals: Vec<String> = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(goals_q.as_str()) {
        for sol in sols.flatten() {
            if let Some(t) = sol.get("goal") {
                let v = term_literal_string(t);
                if !v.is_empty() {
                    authorized_goals.push(v);
                }
            }
        }
    }
    authorized_goals.sort();
    authorized_goals.dedup();

    // -- Bootstrap-session provenance: source, hash, ontology version.
    let prov_q = "PREFIX dec:  <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?source ?hash ?version WHERE {
  <https://decision-cli.dev/ns/session/init-001>
      prov:wasDerivedFrom ?source ;
      dec:definitionHash  ?hash ;
      dec:ontologyVersion ?version .
} LIMIT 1";
    let (source, hash, ontology_version) = match store.query(prov_q) {
        Ok(QueryResults::Solutions(mut sols)) => match sols.next() {
            Some(Ok(sol)) => {
                let source = sol
                    .get("source")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let hash = sol
                    .get("hash")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let ver = sol
                    .get("version")
                    .map(term_literal_string)
                    .unwrap_or_default();
                (source, hash, ver)
            }
            _ => (String::new(), String::new(), String::new()),
        },
        _ => (String::new(), String::new(), String::new()),
    };

    let action_label = shorten_value_action(&action_iri);
    let provenance_label = if bundled::lookup_value_action(&action_iri).is_some() {
        format!("bundled, ontology v{ontology_version}")
    } else {
        format!("custom, ontology v{ontology_version}")
    };

    // Hash short form (first 12 hex chars) for the headline; full hash
    // still queryable via SPARQL.
    let hash_short = if hash.len() >= 12 {
        format!("sha256:{}…", &hash[..12])
    } else if !hash.is_empty() {
        format!("sha256:{hash}")
    } else {
        "sha256:(unknown)".to_string()
    };

    let display_name = if stream_name.is_empty() {
        shorten_stream(&stream_iri)
    } else {
        stream_name
    };

    let store_display = dec_dir
        .join("store")
        .to_string_lossy()
        .into_owned();

    println!("Value Stream:      {display_name}");
    if source.is_empty() {
        println!("Definition:        ({hash_short})");
    } else {
        println!("Definition:        {source} ({hash_short})");
    }
    println!("  full hash:       sha256:{hash}");
    println!("Terminal Value:    {action_label} ({provenance_label})");
    println!(
        "Authorized Goals:  {}",
        if authorized_goals.is_empty() {
            "(none)".to_string()
        } else {
            authorized_goals.join(", ")
        }
    );
    println!("Graph Store:       {store_display}");

    ExitCode::SUCCESS
}

fn term_iri_string(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

fn term_literal_string(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        other => other.to_string(),
    }
}

/// Render a ValueAction IRI in its prefixed display form. Bundled URIs
/// under `https://decision-cli.dev/ns/value-actions/` shorten to
/// `va:<local>`; other IRIs are echoed verbatim.
fn shorten_value_action(iri: &str) -> String {
    let prefix = "https://decision-cli.dev/ns/value-actions/";
    if let Some(local) = iri.strip_prefix(prefix) {
        format!("va:{local}")
    } else {
        iri.to_string()
    }
}

/// Render a ValueStream IRI in its prefixed display form.
fn shorten_stream(iri: &str) -> String {
    let prefix = "https://decision-cli.dev/ns/streams/";
    if let Some(local) = iri.strip_prefix(prefix) {
        local.to_string()
    } else if let Some(local) = iri.strip_prefix("stream:") {
        local.to_string()
    } else {
        iri.to_string()
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
