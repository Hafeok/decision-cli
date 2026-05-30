//! Worktree creation logic (FT-115).

use std::path::PathBuf;
use std::process::Command;

use super::{worktree_branch_name, worktree_dir, WorktreeError};

/// The absolute path to a created worktree.
#[derive(Debug, Clone)]
pub struct WorktreePath {
    pub path: PathBuf,
    pub session_short_id: String,
    pub baseline_commit: String,
}

/// Create a git worktree for the given session at `.dec/worktrees/<session_short_id>`,
/// branched off `baseline_commit`.
///
/// Returns the absolute path to the worktree on success.
///
/// # Errors
///
/// - Git command fails (uncommitted changes in main, disk full, etc.)
/// - Path canonicalization fails
pub fn create_worktree(
    workdir: &std::path::Path,
    session_short_id: &str,
    baseline_commit: &str,
) -> Result<WorktreePath, WorktreeError> {
    let worktree_path = worktree_dir(workdir, session_short_id);
    let branch_name = worktree_branch_name(session_short_id);

    // Ensure .dec/worktrees/ exists
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            WorktreeError::PathError(format!("failed to create .dec/worktrees/: {}", e))
        })?;
    }

    // git worktree add -b dec/sess/<sid> .dec/worktrees/<sid> <baseline_commit>
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&branch_name)
        .arg(&worktree_path)
        .arg(baseline_commit)
        .output()
        .map_err(|e| WorktreeError::GitFailed(format!("failed to spawn git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitFailed(format!(
            "git worktree add failed: {}",
            stderr
        )));
    }

    // Canonicalize the path to get the absolute form
    let canonical = worktree_path.canonicalize().map_err(|e| {
        WorktreeError::PathError(format!(
            "failed to canonicalize worktree path {}: {}",
            worktree_path.display(),
            e
        ))
    })?;

    Ok(WorktreePath {
        path: canonical,
        session_short_id: session_short_id.to_string(),
        baseline_commit: baseline_commit.to_string(),
    })
}
