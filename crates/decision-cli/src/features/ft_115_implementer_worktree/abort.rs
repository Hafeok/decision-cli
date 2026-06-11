//! Worktree cleanup logic (FT-115).

use std::process::Command;

use super::{worktree_branch_name, WorktreeError, WorktreePath};

/// Remove a worktree directory and its associated branch.
///
/// Idempotent: if the worktree or branch doesn't exist, succeeds silently.
///
/// # Errors
///
/// - Git command fails (stuck process holding file open, etc.)
///   Logs the error but doesn't fail the operation — orphan cleanup will retry.
pub fn abort_worktree(
    workdir: &std::path::Path,
    worktree: &WorktreePath,
) -> Result<(), WorktreeError> {
    let branch_name = worktree_branch_name(&worktree.session_short_id);

    // git worktree remove --force <path>
    // --force allows removal even if worktree has uncommitted changes
    let remove_output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(&worktree.path)
        .output()
        .map_err(|e| {
            WorktreeError::GitFailed(format!("failed to spawn git worktree remove: {}", e))
        })?;

    if !remove_output.status.success() {
        let stderr = String::from_utf8_lossy(&remove_output.stderr);
        // Log but don't fail — orphan pruning will retry
        eprintln!(
            "warning: git worktree remove failed for {}: {}",
            worktree.path.display(),
            stderr
        );
    }

    // git branch -D dec/sess/<sid>
    let branch_output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("branch")
        .arg("-D")
        .arg(&branch_name)
        .output()
        .map_err(|e| WorktreeError::GitFailed(format!("failed to spawn git branch -D: {}", e)))?;

    if !branch_output.status.success() {
        let stderr = String::from_utf8_lossy(&branch_output.stderr);
        // Also log-only — branch might already be gone
        eprintln!("warning: git branch -D {} failed: {}", branch_name, stderr);
    }

    Ok(())
}
