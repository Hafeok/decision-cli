//! `dec verify {bench, graph, ...}` — CLI adapter for the verification surface.
//!
//! Top-level dispatch only. Per-noun args and per-leaf run handlers live
//! under [`bench`] and [`graph`] submodules so this file stays well under
//! the ADR-013 §Rule 1 hard limit.
//!
//! Per ADR-029, every leaf clap subcommand is paired with an MCP tool of
//! the same `dec_<noun>_<verb>` name; both surfaces route through one
//! handler function exported by the corresponding `features/verify_*`
//! module.

pub mod bench;
pub mod feature;
pub mod graph;
pub mod step;

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;

#[allow(unused_imports)]
pub use bench::{
    bench_list_request, bench_new_request, bench_show_request, BenchCmd, BenchListArgs, BenchNewArgs,
    BenchShowArgs,
};
#[allow(unused_imports)]
pub use graph::{
    graph_list_request, graph_new_request, graph_show_request, GraphCmd, GraphListArgs,
    GraphNewArgs, GraphShowArgs,
};
#[allow(unused_imports)]
pub use step::{step_add_request, StepAddArgs, StepCmd};

/// Names of every MCP tool the `dec verify` clap tree pairs with. The
/// TC-052 surface-symmetry harness asserts this list matches the MCP
/// registry. The constant is `pub` so the parity TC's structural check
/// (grep over this file) can confirm one entry per leaf subcommand.
#[allow(dead_code)]
pub const PAIRED_TOOL_NAMES: &[&str] = &[
    "dec_verify_bench_list",
    "dec_verify_bench_new",
    "dec_verify_bench_show",
    "dec_verify_feature",
    "dec_verify_graph_accept",
    "dec_verify_graph_generate",
    "dec_verify_graph_list",
    "dec_verify_graph_new",
    "dec_verify_graph_run",
    "dec_verify_graph_show",
    "dec_verify_step_add",
];

#[derive(Debug, Subcommand)]
pub enum VerifyCmd {
    /// Manage `dec:VerificationBench` artifacts.
    #[command(subcommand)]
    Bench(BenchCmd),
    /// Manage `dec:VerificationGraph` artifacts.
    #[command(subcommand)]
    Graph(GraphCmd),
    /// Manage `dec:VerificationStep` artifacts within a graph.
    #[command(subcommand)]
    Step(StepCmd),
    /// Verify a feature by running every covering VerificationGraph
    /// and aggregating verdicts (FT-099).
    Feature(feature::FeatureArgs),
}

pub fn run(workdir: &Path, cmd: VerifyCmd) -> ExitCode {
    match cmd {
        VerifyCmd::Bench(c) => bench::run(workdir, c),
        VerifyCmd::Graph(c) => graph::run(workdir, c),
        VerifyCmd::Step(c) => step::run(workdir, c),
        VerifyCmd::Feature(args) => feature::run_feature(workdir, args),
    }
}

/// Map handler errors to CLI exit codes per FT-038 §Error handling.
pub(crate) fn exit_code_for(err: &HandlerError) -> u8 {
    match err {
        HandlerError::InvalidArgument { .. } => 2,
        _ => 1,
    }
}
