//! Hidden CLI helpers for FT-100 subscription dispatch (tests only).
//!
//! These commands simulate the natural event triggers that the FT-100
//! subscriptions react to. They let test scripts drive the
//! `graph_accepted_dispatch` and `code_change_committed_dispatch`
//! handlers directly without standing up a long-running orchestrator
//! process. Replay-style invocations (TC-164 dedup tests) re-invoke
//! the same handler to verify the ledger short-circuits.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::subscriptions;

#[derive(Debug, Subcommand)]
pub enum InternalDispatchCmd {
    /// Fire the graph-accepted subscription handler for `<graph-id>`.
    GraphAccepted(GraphAcceptedArgs),
    /// Fire the code-change-committed subscription handler for
    /// `<feature-id>` and `<code-change-iri>`.
    CodeChangeCommitted(CodeChangeCommittedArgs),
    /// Dump dedup ledger state for the graph-accepted subscription.
    LedgerGraphAccepted(LedgerGraphAcceptedArgs),
    /// Dump dedup ledger state for the code-change-committed subscription.
    LedgerCodeChange(LedgerCodeChangeArgs),
}

#[derive(Debug, clap::Args)]
pub struct GraphAcceptedArgs {
    /// VG-NNN short id.
    pub graph_id: String,
}

#[derive(Debug, clap::Args)]
pub struct CodeChangeCommittedArgs {
    /// FT-NNN short id.
    pub feature_id: String,
    /// IRI of the dec:CodeChange artifact.
    pub code_change_iri: String,
}

#[derive(Debug, clap::Args)]
pub struct LedgerGraphAcceptedArgs {
    /// VG-NNN short id.
    pub graph_id: String,
    /// Env short id.
    pub env: String,
}

#[derive(Debug, clap::Args)]
pub struct LedgerCodeChangeArgs {
    /// FT-NNN short id.
    pub feature_id: String,
    /// IRI of the dec:CodeChange artifact.
    pub code_change_iri: String,
}

pub fn run(workdir: &Path, cmd: InternalDispatchCmd) -> ExitCode {
    match cmd {
        InternalDispatchCmd::GraphAccepted(args) => run_graph_accepted(workdir, &args),
        InternalDispatchCmd::CodeChangeCommitted(args) => run_code_change(workdir, &args),
        InternalDispatchCmd::LedgerGraphAccepted(args) => run_ledger_graph(workdir, &args),
        InternalDispatchCmd::LedgerCodeChange(args) => run_ledger_code_change(workdir, &args),
    }
}

fn run_graph_accepted(workdir: &Path, args: &GraphAcceptedArgs) -> ExitCode {
    match subscriptions::dispatch_for_graph(workdir, &args.graph_id) {
        Ok(outcome) => {
            if outcome.disabled {
                println!("disabled");
                return ExitCode::SUCCESS;
            }
            println!(
                "dispatched={} skipped_dedup={} env_errors={}",
                outcome.dispatched.len(),
                outcome.skipped_dedup.len(),
                outcome.env_errors.len(),
            );
            for tup in &outcome.dispatched {
                println!(
                    "tuple {} {} session={} result={} verdict={}",
                    tup.graph_short,
                    tup.env_short,
                    tup.session_iri.as_str(),
                    tup.result_iri.as_ref().map(|n| n.as_str()).unwrap_or("(none)"),
                    tup.verdict.as_deref().unwrap_or("(none)"),
                );
            }
            for (env, err) in &outcome.env_errors {
                eprintln!("env-error {env}: {err}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("_dispatch graph-accepted: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_code_change(workdir: &Path, args: &CodeChangeCommittedArgs) -> ExitCode {
    match subscriptions::dispatch_for_code_change(workdir, &args.feature_id, &args.code_change_iri) {
        Ok(outcome) => {
            if outcome.disabled {
                println!("disabled");
                return ExitCode::SUCCESS;
            }
            if outcome.skipped_dedup {
                println!("skipped_dedup");
                return ExitCode::SUCCESS;
            }
            println!(
                "aggregate_session={} verdict={} per_graph={} feedback={} coverage_gap={}",
                outcome
                    .aggregate_session
                    .as_ref()
                    .map(|n| n.as_str())
                    .unwrap_or("(none)"),
                outcome.aggregate_verdict.as_deref().unwrap_or("(none)"),
                outcome.per_graph.len(),
                outcome.aggregate_feedback.len(),
                outcome.coverage_gap,
            );
            for d in &outcome.per_graph {
                println!(
                    "tuple {} {} session={} verdict={}",
                    d.graph_short,
                    d.env_short,
                    d.session.as_str(),
                    d.verdict.as_deref().unwrap_or("(none)"),
                );
            }
            for fb in &outcome.aggregate_feedback {
                println!("feedback={}", fb.as_str());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("_dispatch code-change-committed: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_ledger_graph(workdir: &Path, args: &LedgerGraphAcceptedArgs) -> ExitCode {
    let dump_path = decision_cli::orchestration_dump_path(workdir);
    let store = match decision_cli::load_store_from_dump(&dump_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("load store: {e}");
            return ExitCode::from(1);
        }
    };
    let graph_iri = format!(
        "https://decision-cli.dev/ns/graph/{graph}",
        graph = args.graph_id
    );
    let env_iri = format!(
        "https://decision-cli.dev/ns/bench/{env}",
        env = args.env
    );
    match subscriptions::graph_accepted_dispatch::ledger::get_entry(
        &store,
        &graph_iri,
        &env_iri,
    ) {
        Ok(Some(entry)) => {
            println!(
                "entry graph={} env={} last_dispatch_at={}",
                entry.graph, entry.env, entry.last_dispatch_at
            );
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("(no entry)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ledger: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_ledger_code_change(workdir: &Path, args: &LedgerCodeChangeArgs) -> ExitCode {
    let dump_path = decision_cli::orchestration_dump_path(workdir);
    let store = match decision_cli::load_store_from_dump(&dump_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("load store: {e}");
            return ExitCode::from(1);
        }
    };
    match subscriptions::code_change_committed_dispatch::ledger::get_entry(
        &store,
        &args.code_change_iri,
        &args.feature_id,
    ) {
        Ok(Some(entry)) => {
            println!(
                "entry code_change={} feature={} last_dispatch_at={}",
                entry.code_change, entry.feature, entry.last_dispatch_at
            );
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("(no entry)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ledger: {e}");
            ExitCode::from(1)
        }
    }
}
