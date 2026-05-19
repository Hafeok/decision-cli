//! `dec _check-goal` — hidden helper exercising the FT-010 goal gate.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::scope::{ActiveScope, ScopeError};

#[derive(Debug, clap::Args)]
pub struct CheckGoalArgs {
    /// Goal verb to validate against the active stream's authorized-goals list.
    pub goal: String,
}

/// `dec _check-goal <goal>` — load the active stream from the persisted
/// store (FT-009 / ADR-012) and call [`ActiveScope::validate_goal`].
///
/// Per ADR-005 this is the chokepoint every dispatch-initiating command
/// (`dec implement`, future `dec drive`) must run **before** writing any
/// Session / Goal / Dispatch artifact.
pub fn run(workdir: &Path, args: CheckGoalArgs) -> ExitCode {
    let scope = match ActiveScope::load(workdir) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("dec _check-goal: {err}");
            return ExitCode::from(1);
        }
    };
    match scope.validate_goal(&args.goal) {
        Ok(()) => {
            println!(
                "goal `{}` is authorized on stream <{}>",
                args.goal, scope.stream_iri
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_scope_error(&err);
            ExitCode::from(1)
        }
    }
}

fn print_scope_error(err: &ScopeError) {
    eprintln!("dec: {err}");
}
