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
    /// When `true`, the run was a defect-fix (the dispatch bundle
    /// carried non-empty defect_feedback). The finalizer enforces a
    /// scope guard: modifications to files NOT touched by any prior
    /// `[FT-XXX]` commit are rejected, only new files are
    /// unconditionally allowed. This stops "I gave up but made
    /// changes anyway" worker outputs from gutting unrelated
    /// modules. Initial runs (no defect feedback) keep the open
    /// behaviour — anything in the working tree may be committed.
    pub defect_scoped: bool,
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
    /// Defect-scoped guard rejected the worker's diff: the worker
    /// modified files outside the feature's commit history. The
    /// dispatch is aborted before the commit lands so the working
    /// tree change can be inspected and reset.
    #[error(
        "defect-scope violation: worker modified files outside the feature's prior \
         commit history: {paths:?}; aborting commit (initial-implementation runs are \
         unrestricted; this guard only fires when the bundle carries defect feedback)"
    )]
    ScopeViolation {
        /// Out-of-scope paths the worker modified.
        paths: Vec<String>,
    },
}

/// Run FT-017 finalisation. See module docs for the contract.
///
/// Order is `status-transition → git add -A → git commit`. The
/// status transition runs first so the resulting feature_spec file
/// change is absorbed by the same commit — otherwise TC-018 #1
/// (working tree clean after the run) and #5 (`product feature
/// show` reports `complete`) cannot both hold. The FT-017 invariant
/// "status transition is the **last** observable side-effect" still
/// holds in the user-visible sense: the feature_spec landing at
/// `complete` is the terminal artifact state the operator inspects.
/// Hooks are still fully honoured (no `--no-verify`).
pub fn finalize_run(input: &FinalizeInput<'_>) -> Result<FinalizeOutcome, FinalizeError> {
    let mut outcome = FinalizeOutcome::default();

    match transition_feature_status(input.product_root, input.feature_id) {
        Ok(()) => outcome.status_transitioned = true,
        Err(note) => outcome.notes.push(note),
    }

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
        if input.defect_scoped {
            let allowed =
                feature_commit_files(input.repo_root, input.feature_id).unwrap_or_default();
            let dirty = working_tree_modified_files(input.repo_root)?;
            let violations: Vec<String> = dirty
                .into_iter()
                .filter(|(status, _)| status == "M" || status == "D" || status == "R")
                .filter(|(_, path)| !allowed.contains(path))
                .map(|(_, path)| path)
                .collect();
            if !violations.is_empty() {
                return Err(FinalizeError::ScopeViolation { paths: violations });
            }
        }
        let message = build_commit_message(input);
        run_git_add(input.repo_root)?;
        run_git_commit(input.repo_root, &message)?;
        outcome.commit_sha = Some(read_short_sha(input.repo_root)?);
    } else {
        outcome
            .notes
            .push("no working-tree changes — skipping commit".into());
    }

    Ok(outcome)
}

/// Files touched by any prior `[FT-XXX]` commit. The defect-scope
/// guard's allowlist for modifications: a worker fixing defect
/// feedback for FT-X must only touch files this set contains; new
/// files (status `A` in the working tree) are unconditionally
/// allowed. Returns an empty set on the very first run for the
/// feature (no prior commits), in which case the guard's
/// "modifications-only" filter naturally lets the run through with
/// only new-file additions.
fn feature_commit_files(
    repo_root: &Path,
    feature_id: &str,
) -> Result<std::collections::HashSet<String>, FinalizeError> {
    let needle = format!("[{feature_id}]");
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg("--all")
        .arg("--format=%H")
        .arg(format!("--grep={}", regex_escape_brackets(&needle)))
        .arg("--extended-regexp")
        .output()
        .map_err(|e| FinalizeError::StatusFailed {
            detail: format!("git log --grep={needle}: {e}"),
        })?;
    if !out.status.success() {
        return Ok(std::collections::HashSet::new());
    }
    let shas: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
    for sha in shas {
        let show = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("show")
            .arg("--name-only")
            .arg("--format=")
            .arg(&sha)
            .output()
            .map_err(|e| FinalizeError::StatusFailed {
                detail: format!("git show {sha}: {e}"),
            })?;
        if !show.status.success() {
            continue;
        }
        for line in String::from_utf8_lossy(&show.stdout).lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                files.insert(trimmed.to_string());
            }
        }
    }
    Ok(files)
}

/// Escape regex bracket metacharacters in the `--grep` needle so
/// `git log --extended-regexp` matches the literal `[FT-XXX]` tag.
fn regex_escape_brackets(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]")
}

/// `(status, path)` pairs from `git status --porcelain`. `status` is
/// the first column (e.g. `M`, `A`, `D`, `R`, `??`).
fn working_tree_modified_files(
    repo_root: &Path,
) -> Result<Vec<(String, String)>, FinalizeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain=v1")
        .output()
        .map_err(|e| FinalizeError::StatusFailed {
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(FinalizeError::StatusFailed {
            detail: String::from_utf8_lossy(&out.stderr).trim().into(),
        });
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        // Porcelain v1: two status chars (XY), then a space, then the
        // path. For rename, the path is `OLD -> NEW`.
        let (status, rest) = line.split_at(2);
        let path = rest.trim_start();
        let normalized_status = match status.trim() {
            "??" => "A".to_string(),
            s if s.starts_with('R') => "R".to_string(),
            s => s.chars().next().unwrap_or(' ').to_string(),
        };
        let normalized_path = path
            .split(" -> ")
            .last()
            .unwrap_or(path)
            .to_string();
        rows.push((normalized_status, normalized_path));
    }
    Ok(rows)
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
