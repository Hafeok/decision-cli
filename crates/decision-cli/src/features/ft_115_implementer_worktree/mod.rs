//! FT-115: Per-session git worktree support for implementer dispatches.
//!
//! Each implementer session runs in an ephemeral git worktree at
//! `.dec/worktrees/<short-session-id>/`, isolating source edits from main
//! until the harness chooses to merge them. This prevents pollution from
//! incomplete dispatches, enables parallel implementer dispatches (in future),
//! and provides clean atomic abort.

mod abort;
mod cli;
mod coordinate;
mod create;
mod merge;
#[cfg(test)]
mod tests;

pub use abort::abort_worktree;
pub use cli::{handle_worktree_list, handle_worktree_prune};
pub use coordinate::{
    prune_orphans, record_worktree_aborted, record_worktree_created, record_worktree_merged,
    PruneOutcome,
};
pub use create::{create_worktree, WorktreePath};
pub use merge::{fast_forward_into_main, MergeError, MergeOutcome};

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("git worktree command failed: {0}")]
    GitFailed(String),

    #[error("worktree path error: {0}")]
    PathError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("orchestration store error: {0}")]
    StoreError(String),
}

/// Compute the worktree directory path for a given session short ID.
pub fn worktree_dir(workdir: &std::path::Path, session_short_id: &str) -> PathBuf {
    workdir.join(".dec/worktrees").join(session_short_id)
}

/// Compute the worktree branch name for a given session short ID.
pub fn worktree_branch_name(session_short_id: &str) -> String {
    format!("dec/sess/{}", session_short_id)
}

/// Extract the short session ID from a worktree branch name.
/// Returns None if the branch name doesn't match the expected pattern.
pub fn session_short_id_from_branch(branch_name: &str) -> Option<String> {
    branch_name.strip_prefix("dec/sess/").map(String::from)
}
