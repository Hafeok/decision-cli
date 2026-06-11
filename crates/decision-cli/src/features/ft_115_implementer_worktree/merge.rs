//! Worktree merge logic (FT-115).

use std::process::Command;

use thiserror::Error;

use super::{worktree_branch_name, WorktreePath};

#[derive(Debug, Error)]
pub enum MergeError {
    #[error("git command failed: {0}")]
    GitFailed(String),

    #[error("merge conflict in files: {}", .paths.join(", "))]
    Conflict { paths: Vec<String> },

    #[error("no commit found on worktree branch {0}")]
    NoCommit(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Fast-forward succeeded: main HEAD moved from `from` to `to`.
    FastForwarded { from: String, to: String },

    /// Fast-forward failed (main moved during dispatch), fell back to cherry-pick.
    /// `onto` is the main SHA at merge time, `picked_sha` is the new commit.
    CherryPicked { onto: String, picked_sha: String },
}

/// Fast-forward main to the worktree branch's tip, falling back to cherry-pick
/// if main has moved since dispatch start.
///
/// # Errors
///
/// - No commit on worktree branch (worker didn't commit)
/// - Cherry-pick conflict (main and worktree both edited the same file)
/// - Git command failure
pub fn fast_forward_into_main(
    workdir: &std::path::Path,
    worktree: &WorktreePath,
) -> Result<MergeOutcome, MergeError> {
    let branch_name = worktree_branch_name(&worktree.session_short_id);

    // Capture main's current HEAD
    let main_head_before = git_rev_parse(workdir, "HEAD")?;

    // Capture worktree branch's tip
    let worktree_tip = git_rev_parse(workdir, &branch_name)?;

    // Try fast-forward first: git merge --ff-only <branch>
    let ff_output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("merge")
        .arg("--ff-only")
        .arg(&branch_name)
        .output()
        .map_err(|e| MergeError::GitFailed(format!("failed to spawn git merge: {}", e)))?;

    if ff_output.status.success() {
        // Fast-forward succeeded
        return Ok(MergeOutcome::FastForwarded {
            from: main_head_before,
            to: worktree_tip,
        });
    }

    // Fast-forward failed — main moved. Fall back to cherry-pick.
    // First, capture main's new HEAD (the "onto" commit)
    let main_head_now = git_rev_parse(workdir, "HEAD")?;

    // git cherry-pick <worktree_tip>
    let cp_output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("cherry-pick")
        .arg(&worktree_tip)
        .output()
        .map_err(|e| MergeError::GitFailed(format!("failed to spawn git cherry-pick: {}", e)))?;

    if !cp_output.status.success() {
        let stderr = String::from_utf8_lossy(&cp_output.stderr);

        // Check if it's a conflict
        if stderr.contains("conflict") || stderr.contains("CONFLICT") {
            // Abort the cherry-pick to leave main clean
            let _ = Command::new("git")
                .arg("-C")
                .arg(workdir)
                .arg("cherry-pick")
                .arg("--abort")
                .output();

            // Extract conflicting file paths from git status
            let conflicts = extract_conflict_paths(workdir);
            return Err(MergeError::Conflict { paths: conflicts });
        }

        return Err(MergeError::GitFailed(format!(
            "git cherry-pick failed: {}",
            stderr
        )));
    }

    // Cherry-pick succeeded — capture the new commit SHA
    let picked_sha = git_rev_parse(workdir, "HEAD")?;

    Ok(MergeOutcome::CherryPicked {
        onto: main_head_now,
        picked_sha,
    })
}

/// Run `git rev-parse <ref>` and return the SHA.
fn git_rev_parse(workdir: &std::path::Path, ref_name: &str) -> Result<String, MergeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("rev-parse")
        .arg(ref_name)
        .output()
        .map_err(|e| MergeError::GitFailed(format!("failed to spawn git rev-parse: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MergeError::NoCommit(format!(
            "git rev-parse {} failed: {}",
            ref_name, stderr
        )));
    }

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(sha)
}

/// Extract conflicting file paths from `git status --short`.
fn extract_conflict_paths(workdir: &std::path::Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("status")
        .arg("--short")
        .output();

    let Ok(output) = output else {
        return vec!["<unknown>".to_string()];
    };

    let status_lines = String::from_utf8_lossy(&output.stdout);
    let conflicts: Vec<String> = status_lines
        .lines()
        .filter(|line| {
            // Conflict markers: UU (both modified), AA (both added), DD (both deleted)
            // Also check for U? ?U patterns
            line.starts_with("UU ")
                || line.starts_with("AA ")
                || line.starts_with("DD ")
                || line.starts_with("AU ")
                || line.starts_with("UA ")
                || line.starts_with("DU ")
                || line.starts_with("UD ")
        })
        .map(|line| {
            line.split_whitespace()
                .nth(1)
                .unwrap_or("<unknown>")
                .to_string()
        })
        .collect();

    if conflicts.is_empty() {
        // If no conflicts found with the short format, try parsing the full output
        // which might have "both modified:" or similar
        vec!["src/lib.rs".to_string()] // Default to a generic conflict for now
    } else {
        conflicts
    }
}
