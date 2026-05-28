//! `dec implement` post-run finalisation (FT-017).
//!
//! After [`crate::implement::run`] persists the orchestration store, the
//! harness has produced files on disk and a `CodeChange` in the
//! product-cli graph slice — but two operator-facing steps remain:
//!
//! 1. Commit the working-tree changes the worker produced, with a
//!    message that ties the commit to the Session / Dispatch / CodeChange
//!    IRIs (so `git log` is a usable audit trail without product-cli).
//! 2. Flip the feature_spec's status to `complete` via
//!    `product feature status FT-XXX complete`.
//!
//! Both steps run **after** the orchestration record is durable, so a
//! failure here surfaces as a warning or finalisation error rather than
//! corrupting the Session record. This file owns FT-017 in isolation so
//! the host `implement.rs` stays under the ADR-013 file-length limit.
//!
//! See `.product/features/FT-017-*.md` for the full contract.

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Outcome of [`finalize_run`]. Surfaces enough state for `dec implement`
/// to print the trailing telemetry block and for tests to assert on.
#[derive(Debug, Clone, Default)]
pub struct FinalizeOutcome {
    /// Short SHA (`git rev-parse --short HEAD`) of the commit produced
    /// for this run, or `None` if the working tree was clean.
    pub commit_sha: Option<String>,
    /// Whether the feature_spec status transition ran to success.
    /// `false` when `product` is missing or returned non-zero.
    pub status_transitioned: bool,
    /// Human-readable note emitted alongside the outcome — usually
    /// "no working-tree changes — skipping commit" or a warning when
    /// `product` was unavailable.
    pub notes: Vec<String>,
}

/// Inputs the finaliser needs from a successful [`crate::implement::run`].
#[derive(Debug, Clone)]
pub struct FinalizeInput<'a> {
    /// Repo root the harness operated against (where `git` is run).
    pub repo_root: &'a Path,
    /// Product-cli root (passed as `--root` to `product feature status`).
    pub product_root: &'a Path,
    /// The feature id being implemented (e.g. `"FT-015"`).
    pub feature_id: &'a str,
    /// Session IRI minted by FT-011 — included in the commit body.
    pub session_iri: &'a str,
    /// Dispatch IRI minted by FT-011 — included in the commit body.
    pub dispatch_iri: &'a str,
    /// CodeChange IRI returned by the worker — included in the commit
    /// body. Empty string is tolerated for stub runs that did not mint
    /// a CodeChange (the field then drops out of the message body).
    pub code_change_iri: &'a str,
    /// Full SHA-256 of the bundle — short-formed into the commit body.
    pub bundle_hash: &'a str,
    /// Free-form summary from the worker. The first non-blank line
    /// (truncated) is used as the commit subject after `[FT-XXX] `.
    pub worker_summary: &'a str,
}

/// Finalisation errors that abort the `dec implement` run.
#[derive(Debug, Error)]
pub enum FinalizeError {
    /// `git add` / `git commit` exited non-zero.
    #[error("git commit failed: {detail}")]
    CommitFailed {
        /// Stderr from the failing git invocation, trimmed.
        detail: String,
    },
    /// `git status --porcelain` failed to execute.
    #[error("git status failed: {detail}")]
    StatusFailed {
        /// Stderr from the failing git invocation, trimmed.
        detail: String,
    },
}

/// Run FT-017 finalisation. See module docs for the contract.
pub fn finalize_run(input: &FinalizeInput<'_>) -> Result<FinalizeOutcome, FinalizeError> {
    let mut outcome = FinalizeOutcome::default();

    if !git_on_path() {
        outcome
            .notes
            .push("git not on $PATH — skipping commit step".into());
    } else if !is_git_repo(input.repo_root) {
        outcome.notes.push(format!(
            "{} is not inside a git work tree — skipping commit step",
            input.repo_root.display()
        ));
    } else if working_tree_dirty(input.repo_root)? {
        let message = build_commit_message(input);
        run_git_add(input.repo_root)?;
        run_git_commit(input.repo_root, &message)?;
        outcome.commit_sha = Some(read_short_sha(input.repo_root)?);
    } else {
        outcome
            .notes
            .push("no working-tree changes — skipping commit".into());
    }

    match transition_feature_status(input.product_root, input.feature_id) {
        Ok(()) => outcome.status_transitioned = true,
        Err(note) => outcome.notes.push(note),
    }

    Ok(outcome)
}

fn git_on_path() -> bool {
    which("git").is_some()
}

fn is_git_repo(repo_root: &Path) -> bool {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim() == "true",
        _ => false,
    }
}

fn working_tree_dirty(repo_root: &Path) -> Result<bool, FinalizeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|e| FinalizeError::StatusFailed {
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(FinalizeError::StatusFailed {
            detail: String::from_utf8_lossy(&out.stderr).trim().into(),
        });
    }
    Ok(!out.stdout.is_empty())
}

fn run_git_add(repo_root: &Path) -> Result<(), FinalizeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("add")
        .arg("-A")
        .output()
        .map_err(|e| FinalizeError::CommitFailed {
            detail: format!("git add: {e}"),
        })?;
    if !out.status.success() {
        return Err(FinalizeError::CommitFailed {
            detail: String::from_utf8_lossy(&out.stderr).trim().into(),
        });
    }
    Ok(())
}

fn run_git_commit(repo_root: &Path, message: &str) -> Result<(), FinalizeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("commit")
        .arg("-m")
        .arg(message)
        .output()
        .map_err(|e| FinalizeError::CommitFailed {
            detail: format!("git commit: {e}"),
        })?;
    if !out.status.success() {
        let mut detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if detail.is_empty() {
            detail = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
        return Err(FinalizeError::CommitFailed { detail });
    }
    Ok(())
}

fn read_short_sha(repo_root: &Path) -> Result<String, FinalizeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .map_err(|e| FinalizeError::CommitFailed {
            detail: format!("git rev-parse: {e}"),
        })?;
    if !out.status.success() {
        return Err(FinalizeError::CommitFailed {
            detail: String::from_utf8_lossy(&out.stderr).trim().into(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn build_commit_message(input: &FinalizeInput<'_>) -> String {
    let subject = format!("[{}] {}", input.feature_id, summarise(input.worker_summary));
    let short_hash = input.bundle_hash.get(..16).unwrap_or(input.bundle_hash);
    let mut body = format!(
        "Session:     {}\nDispatch:    {}\n",
        input.session_iri, input.dispatch_iri
    );
    if !input.code_change_iri.is_empty() {
        body.push_str(&format!("CodeChange:  {}\n", input.code_change_iri));
    }
    body.push_str(&format!("Bundle:      sha256:{short_hash}"));
    format!("{subject}\n\n{body}\n")
}

fn summarise(raw: &str) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("(no summary)");
    truncate_chars(line, 72)
}

fn truncate_chars(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn transition_feature_status(product_root: &Path, feature_id: &str) -> Result<(), String> {
    if which("product").is_none() {
        return Err(format!(
            "product CLI not on $PATH — skipped `product feature status {feature_id} complete`; \
             run it by hand to keep the feature_spec in sync"
        ));
    }
    let out = Command::new("product")
        .arg("feature")
        .arg("status")
        .arg(feature_id)
        .arg("complete")
        .arg("--root")
        .arg(product_root)
        .output()
        .map_err(|e| {
            format!("product feature status {feature_id} complete: failed to spawn ({e})")
        })?;
    if !out.status.success() {
        let detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!(
            "product feature status {feature_id} complete: non-zero exit ({}). {detail}",
            out.status
        ));
    }
    Ok(())
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
