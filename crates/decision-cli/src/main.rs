//! `dec` — decision-cli binary entry point (dispatch-only per ADR-013).
//! Slice 1 surface plus FT-033 feedback CLI. See ADR-011, ADR-016.

mod cli;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cli::{
    check_goal::CheckGoalArgs, doctor::DoctorArgs, events::EventsCmd, feedback::FeedbackCmd,
    implement::ImplementCmdArgs, init::InitArgs, mcp::McpCmd, session::SessionCmd,
    sparql::SparqlArgs, verify::VerifyCmd,
};

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
    /// Feedback inspection commands (FT-029 / FT-033).
    #[command(subcommand)]
    Feedback(FeedbackCmd),
    /// MCP server (FT-034 / ADR-029).
    #[command(subcommand)]
    Mcp(McpCmd),
    /// Verification artifact management (FT-038 / ADR-028 / ADR-029).
    #[command(subcommand)]
    Verify(VerifyCmd),
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
        Command::Feedback(cmd) => cli::feedback::run(&workdir, cmd),
        Command::Mcp(cmd) => {
            init_tracing_for_mcp();
            cli::mcp::run(&workdir, cmd)
        }
        Command::Verify(cmd) => cli::verify::run(&workdir, cmd),
    }
}

/// Initialise `tracing` for the MCP server. Stdout is reserved for the
/// JSON-RPC wire protocol per FT-034 §Behaviour, so every diagnostic
/// goes to stderr. The filter respects `RUST_LOG` when set; otherwise
/// defaults to `info` for the `dec_mcp` target.
fn init_tracing_for_mcp() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,dec_mcp=info"));
    // Best-effort — if a subscriber is already installed (e.g. tests
    // already set one up), `try_init` returns Err and we move on.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_ansi(false)
        .try_init();
}
