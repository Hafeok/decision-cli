//! `dec` — decision-cli binary entry point (dispatch-only per ADR-013).
//!
//! Slice 1 surface: init, status, health, implement, events, session.
//! See ADR-011 (CLI shape), FT-012, and ADR-016 (vertical-slice SDP).

mod cli;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use cli::check_goal::CheckGoalArgs;
use cli::doctor::DoctorArgs;
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
    /// not yet implemented).
    #[arg(long, global = true)]
    workdir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialise the orchestration store from a ValueStream definition.
    Init(InitArgs),
    /// Report the active value stream's bootstrap provenance.
    Status,
    /// Hidden helper (tests/CI): run SPARQL against the orchestration store.
    #[command(name = "_sparql", hide = true)]
    Sparql(SparqlArgs),
    /// Hidden helper (FT-010 / TC-007): exercise the goal-validation gate.
    #[command(name = "_check-goal", hide = true)]
    CheckGoal(CheckGoalArgs),
    /// Implement a feature end-to-end (FT-011 + FT-013).
    Implement(ImplementCmdArgs),
    /// Liveness check (FT-012). Runs outside an initialised working tree.
    Health,
    /// Worker preflight audit (FT-016 / TC-047).
    Doctor(DoctorArgs),
    /// Inspect persisted events (FT-012; FT-005 replay, FT-004 SSE tail).
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
        Command::Doctor(args) => cli::doctor::run(&workdir, args),
        Command::Events(cmd) => cli::events::run(&workdir, cmd),
        Command::Session(cmd) => cli::session::run(&workdir, cmd),
    }
}
