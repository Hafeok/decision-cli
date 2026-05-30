//! CLI handlers for `dec _worktree` diagnostic commands (FT-115).

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path};
use decision_cli::features::ft_115_implementer_worktree::{handle_worktree_list, handle_worktree_prune};

#[derive(Debug, Subcommand)]
pub enum WorktreeCmd {
    /// List live worktrees with their session IDs, baselines, and status.
    List,
    /// Prune orphan worktrees (worktrees without live sessions in the store).
    Prune,
}

pub fn run(workdir: &Path, cmd: WorktreeCmd) -> ExitCode {
    let store = match load_store_from_dump(&orchestration_dump_path(workdir)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to load orchestration store: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let result = match cmd {
        WorktreeCmd::List => handle_worktree_list(workdir, &store),
        WorktreeCmd::Prune => handle_worktree_prune(workdir, &store),
    };

    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {:#}", e);
            ExitCode::FAILURE
        }
    }
}
