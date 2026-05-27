//! Clap argument trees for the `dec` binary (dispatch-only per ADR-013).
//!
//! Keeps the per-subcommand attributes / doc strings here so `main.rs`
//! stays at the 80-line cap ADR-013 §Rule 3 places on binary entry
//! points. The `dispatch` function is wiring: one arm per subcommand,
//! routing into the relevant feature module's `run` (per ADR-016).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use super::{
    bootstrap_catalog, bootstrap_catalog::BootstrapCatalogArgs, check_goal,
    check_goal::CheckGoalArgs, doctor, doctor::DoctorArgs, events, events::EventsCmd, feedback,
    feedback::FeedbackCmd, health, implement, implement::ImplementCmdArgs, init, init::InitArgs,
    internal_dispatch, internal_dispatch::InternalDispatchCmd, mcp, mcp::McpCmd, migrate,
    migrate::MigrateCmd, preflight, preflight::PreflightArgs, product, product::ProductArgs, query,
    query::QueryCmd, seed_ft101_catalog, seed_ft101_catalog::SeedFt101CatalogArgs, session,
    session::SessionCmd, sparql, sparql::SparqlArgs, status, verify, verify::VerifyCmd, workers,
    workers::WorkersCmd,
};

#[derive(Debug, Parser)]
#[command(
    name = "dec",
    about = "decision-cli — orchestration system for Decision-Driven Design",
    version
)]
pub struct Cli {
    /// Override the working directory (defaults to CWD; ADR-012 walk-up
    /// not yet implemented).
    #[arg(long, global = true)]
    pub workdir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
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
    /// Hidden helper (FT-058 / TC-104): bootstrap the capability + role
    /// binding catalog from `config/*.yaml` files. The Python wrapper
    /// at `scripts/bootstrap_catalog.py` is the operator-facing entry
    /// point; this command is the Rust-side chokepoint it shells out to.
    #[command(name = "_bootstrap-catalog", hide = true)]
    BootstrapCatalog(BootstrapCatalogArgs),
    /// Hidden helper (FT-107 follow-up): seed the FT-101 catalog
    /// (CapabilityReference + OntologyDescription) so the
    /// verify-graph-author bundle's enrichment fields are non-empty.
    #[command(name = "_seed-ft101-catalog", hide = true)]
    SeedFt101Catalog(SeedFt101CatalogArgs),
    /// Implement a feature end-to-end (FT-011 + FT-013).
    Implement(ImplementCmdArgs),
    /// Feature-coverage report sourced from the internal product-cli
    /// graph projection (FT-052 / TC-087). Reads `.product/graph/index.ttl`;
    /// does not re-parse markdown.
    Preflight(PreflightArgs),
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
    /// Provenance migration tooling (FT-074 / ADR-042).
    #[command(subcommand)]
    Migrate(MigrateCmd),
    /// Absorbed product-cli surface (FT-105 / ADR-067).
    Product(ProductArgs),
    /// QueryTemplate catalog inspection (FT-075 / ADR-043).
    #[command(subcommand)]
    Query(QueryCmd),
    /// Worker container lifecycle (FT-095 — slice 1: `run` only).
    #[command(subcommand)]
    Workers(WorkersCmd),
    /// Hidden helper (FT-100 tests): drive subscription dispatch handlers.
    #[command(name = "_dispatch", hide = true, subcommand)]
    InternalDispatch(InternalDispatchCmd),
}

/// Route a parsed `Command` into the matching feature module. The
/// per-variant dispatch table that would otherwise inflate `main.rs`.
pub fn dispatch(workdir: &Path, command: Command) -> ExitCode {
    match command {
        Command::Init(args) => init::run(workdir, args),
        Command::Status => status::run(workdir),
        Command::Sparql(args) => sparql::run(workdir, args),
        Command::CheckGoal(args) => check_goal::run(workdir, args),
        Command::BootstrapCatalog(args) => bootstrap_catalog::run(workdir, args),
        Command::SeedFt101Catalog(args) => seed_ft101_catalog::run(workdir, args),
        Command::Implement(args) => implement::run(workdir, args),
        Command::Preflight(args) => preflight::run(workdir, args),
        Command::Health => health::run(workdir),
        Command::Doctor(args) => doctor::run(workdir, args),
        Command::Events(cmd) => events::run(workdir, cmd),
        Command::Session(cmd) => session::run(workdir, cmd),
        Command::Feedback(cmd) => feedback::run(workdir, cmd),
        Command::Mcp(cmd) => mcp::run(workdir, cmd),
        Command::Verify(cmd) => verify::run(workdir, cmd),
        Command::Migrate(cmd) => migrate::run(workdir, cmd),
        Command::Product(args) => product::run(workdir, args),
        Command::Query(cmd) => query::run(workdir, cmd),
        Command::Workers(cmd) => workers::run(workdir, cmd),
        Command::InternalDispatch(cmd) => internal_dispatch::run(workdir, cmd),
    }
}
