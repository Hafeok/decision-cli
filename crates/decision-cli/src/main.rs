//! `dec` — decision-cli binary entry point.
//!
//! Slice 1 surface: init, status, health, implement, events, session.
//! See ADR-011 (CLI shape) and FT-012.

mod cli;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use cli::check_goal::CheckGoalArgs;
use cli::events::EventsCmd;
use cli::implement::ImplementCmdArgs;
use cli::init::InitArgs;
use cli::session::SessionCmd;
use cli::sparql::SparqlArgs;

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
    /// Hidden helper for tests/CI (FT-010 / TC-007): exercise the goal
    /// validation gate. Slice 1 does not ship `dec drive`, so this is
    /// the surface that drives the same code path any dispatch-
    /// initiating command will use.
    #[command(name = "_check-goal", hide = true)]
    CheckGoal(CheckGoalArgs),
    /// Implement a feature end-to-end (FT-011 + FT-013): assemble a
    /// bundle for the target feature, dispatch the code-writer role,
    /// record the Session + CodeChange with PROV-O lineage.
    Implement(ImplementCmdArgs),
    /// Liveness check (FT-012): ontology parses, store opens, writer
    /// is operational. Runs even outside an initialised working tree.
    Health,
    /// Inspect persisted events (FT-012). Backed by FT-005 replay
    /// (`since`) and the FT-004 SSE endpoint (`tail`).
    #[command(subcommand)]
    Events(EventsCmd),
    /// Session inspection commands (FT-012).
    #[command(subcommand)]
    Session(SessionCmd),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let workdir = cli
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match cli.command {
        Command::Init(args) => cli::init::run(&workdir, args),
        Command::Status => cli::status::run(&workdir),
        Command::Sparql(args) => cli::sparql::run(&workdir, args),
        Command::CheckGoal(args) => cli::check_goal::run(&workdir, args),
        Command::Implement(args) => cli::implement::run(&workdir, args),
        Command::Health => cli::health::run(&workdir),
        Command::Events(cmd) => cli::events::run(&workdir, cmd),
        Command::Session(cmd) => cli::session::run(&workdir, cmd),
    }
}
