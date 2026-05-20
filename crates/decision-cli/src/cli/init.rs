//! `dec init` — initialise the orchestration store.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_cli::init::{self, DefinitionSource, InitError};
use decision_cli::worker;

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    /// Initialise from a bundled stream template by name.
    #[arg(long, group = "src")]
    pub template: Option<String>,
    /// Initialise from a local Turtle definition file.
    #[arg(long, group = "src", value_name = "PATH")]
    pub from: Option<PathBuf>,
}

pub fn run(workdir: &Path, args: InitArgs) -> ExitCode {
    let source = match resolve_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match init::run(workdir, source) {
        Ok(outcome) => {
            print_init_success(&outcome);
            // FT-016: advisory worker preflight after bootstrap. Init
            // never rolls back on a missing worker — exit 2 is the
            // "store initialised but workers missing" advisory status.
            let report = worker::build_report(
                worker::ACTIVE_ROLES_ENGINEERING_DEVELOPMENT,
                Some(workdir),
                None,
                None,
            );
            println!();
            print!("{}", worker::format_report_text(&report));
            if report.is_all_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(err) => {
            print_init_error(&err);
            ExitCode::from(1)
        }
    }
}

fn resolve_source(args: InitArgs) -> Result<DefinitionSource, ExitCode> {
    match (args.template, args.from) {
        (Some(t), None) => Ok(DefinitionSource::Template(t)),
        (None, Some(p)) => Ok(DefinitionSource::File(p)),
        (Some(_), Some(_)) => {
            eprintln!("dec init: pass exactly one of --template or --from, not both");
            Err(ExitCode::from(2))
        }
        (None, None) => {
            eprintln!(
                "dec init: a definition reference is required.\n  \
                 Try: dec init --template engineering-development\n  \
                 Or:  dec init --from ./streams/decision-cli-development.ttl"
            );
            Err(ExitCode::from(2))
        }
    }
}

fn print_init_success(outcome: &init::InitOutcome) {
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
