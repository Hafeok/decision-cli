//! Integration tests for FT-137 / ADR-078 — defect-scope guard's
//! expanded always-allowed predicate covers build manifests etc.,
//! while still trapping unrelated code drift.
//!
//! TC-343 (positive): defect-scoped finalize succeeds when worker
//! touches `Cargo.toml` plus a prior-set file.
//! TC-344 (regression): defect-scoped finalize raises ScopeViolation
//! when worker touches a code file outside the prior set and outside
//! the allowlist.

use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::finalize::{finalize_run, FinalizeError, FinalizeInput};

// ---------------------------------------------------------------------
// TC-343 — positive: Cargo.toml + prior-set file passes the guard.
// ---------------------------------------------------------------------

#[test]
fn defect_scoped_cargo_toml_succeeds() {
    let repo = setup_repo_with_prior_impl("FT-X", "crates/foo/src/lib.rs");

    // Worker iteration touches an in-scope code file...
    std::fs::write(
        repo.join("crates/foo/src/lib.rs"),
        "// edited content for defect fix\n",
    )
    .unwrap();
    // ...and a build manifest that the original commit didn't touch.
    std::fs::write(repo.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    // ...and a repo-level doc that the original commit didn't touch.
    std::fs::write(repo.join("CLAUDE.md"), "# CLAUDE.md\n").unwrap();

    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-X",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "fix in-scope plus Cargo.toml",
        defect_scoped: true,
        scope_guard_extras: &[],
    };

    let outcome = finalize_run(&input).expect("finalize must succeed");
    assert!(
        outcome.commit_sha.is_some(),
        "expected a commit to land, got notes: {:?}",
        outcome.notes
    );
}

// ---------------------------------------------------------------------
// TC-344 — regression: out-of-scope code file still raises violation.
// ---------------------------------------------------------------------

#[test]
fn defect_scoped_unrelated_code_fails() {
    let repo = setup_repo_with_prior_impl("FT-X", "crates/foo/src/lib.rs");

    // Worker iteration touches an in-scope file (allowed)...
    std::fs::write(
        repo.join("crates/foo/src/lib.rs"),
        "// edited content for defect fix\n",
    )
    .unwrap();
    // ...AND drifts into an unrelated code path (forbidden).
    let bar_path = repo.join("crates/bar/src/lib.rs");
    std::fs::create_dir_all(bar_path.parent().unwrap()).unwrap();
    std::fs::write(&bar_path, "// drift\n").unwrap();
    // Stage it so the guard sees it as M, not A (new files are always
    // allowed; that's the guard's existing behaviour, not the path
    // FT-137 changes). Easiest way: commit, then modify.
    run_git(&repo, &["add", "crates/bar/src/lib.rs"]);
    Command::new("git")
        .current_dir(&repo)
        .args([
            "commit",
            "-m",
            "[FT-Y] seed unrelated file",
            "--no-gpg-sign",
        ])
        .output()
        .expect("git commit");
    std::fs::write(&bar_path, "// drift edit (now M)\n").unwrap();

    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-X",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "drift",
        defect_scoped: true,
        scope_guard_extras: &[],
    };

    let err = finalize_run(&input).expect_err("finalize must raise ScopeViolation");
    match err {
        FinalizeError::ScopeViolation { paths } => {
            assert!(
                paths.iter().any(|p| p == "crates/bar/src/lib.rs"),
                "expected bar/src/lib.rs in violations, got: {paths:?}"
            );
            assert!(
                !paths.iter().any(|p| p == "crates/foo/src/lib.rs"),
                "in-scope foo/src/lib.rs must NOT be flagged, got: {paths:?}"
            );
        }
        other => panic!("expected ScopeViolation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------

/// Initialise a tempdir-backed git repo, seed `file` with `// seed\n`,
/// and create one prior `[feature_id] Initial implementation` commit.
fn setup_repo_with_prior_impl(feature_id: &str, file: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "dec-ft137-{}-{}",
        feature_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    run_git(&base, &["init", "-q"]);
    run_git(&base, &["config", "user.email", "test@test"]);
    run_git(&base, &["config", "user.name", "test"]);
    run_git(&base, &["config", "commit.gpgsign", "false"]);

    let path = base.join(file);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, "// seed\n").unwrap();
    run_git(&base, &["add", "-A"]);
    Command::new("git")
        .current_dir(&base)
        .args([
            "commit",
            "-m",
            &format!("[{feature_id}] Initial implementation"),
            "--no-gpg-sign",
        ])
        .output()
        .unwrap();
    base
}

fn run_git(base: &Path, args: &[&str]) {
    Command::new("git")
        .current_dir(base)
        .args(args)
        .output()
        .expect("git command");
}
