use std::path::Path;

use super::{build_commit_message, summarise, FinalizeInput};

#[test]
fn commit_message_includes_iris_and_short_hash() {
    let input = FinalizeInput {
        repo_root: Path::new("/tmp"),
        product_root: Path::new("/tmp"),
        feature_id: "FT-099",
        session_iri: "https://decision-cli.dev/ns/session/abc",
        dispatch_iri: "https://decision-cli.dev/ns/dispatch/def",
        code_change_iri: "https://decision-cli.dev/ns/code-change/ghi",
        bundle_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        worker_summary: "Land the thing\n\nMore detail follows.",
        defect_scoped: false,
    };
    let msg = build_commit_message(&input);
    assert!(msg.starts_with("[FT-099] Land the thing\n\n"), "subject");
    assert!(msg.contains("Session:     https://decision-cli.dev/ns/session/abc"));
    assert!(msg.contains("Dispatch:    https://decision-cli.dev/ns/dispatch/def"));
    assert!(msg.contains("CodeChange:  https://decision-cli.dev/ns/code-change/ghi"));
    assert!(msg.contains("Bundle:      sha256:0123456789abcdef"));
}

#[test]
fn commit_message_drops_codechange_line_when_empty() {
    let input = FinalizeInput {
        repo_root: Path::new("/tmp"),
        product_root: Path::new("/tmp"),
        feature_id: "FT-099",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "deadbeefdeadbeef",
        worker_summary: "x",
        defect_scoped: false,
    };
    let msg = build_commit_message(&input);
    assert!(!msg.contains("CodeChange:"), "{msg}");
}

#[test]
fn summary_truncates_long_first_line() {
    let s = "a".repeat(120);
    let out = summarise(&s);
    assert_eq!(out.chars().count(), 72);
    assert!(out.ends_with('…'));
}

#[test]
fn summary_picks_first_nonblank_line() {
    let out = summarise("\n\n   actual line   \nnext line");
    assert_eq!(out, "actual line");
}

/// Scope-guard happy path: worker only modifies a file that was
/// touched by a prior `[FT-XXX]` commit; finalize commits cleanly.
#[test]
fn scope_guard_allows_in_scope_modification() {
    use super::{finalize_run, FinalizeError, FinalizeInput};
    let repo = scope_test_setup_repo(
        "FT-200",
        &["crates/feature_200/lib.rs"],
        "initial impl",
    );
    // Worker modifies the in-scope file.
    std::fs::write(
        repo.join("crates/feature_200/lib.rs"),
        "// edited content for defect fix\n",
    )
    .unwrap();
    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-200",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "fix in-scope",
        defect_scoped: true,
    };
    let outcome = finalize_run(&input).expect("finalize succeeds for in-scope edit");
    assert!(outcome.commit_sha.is_some());
    let _ = FinalizeError::ScopeViolation { paths: vec![] };
}

/// Scope-guard rejection: worker modifies a file the feature's
/// prior commits never touched. The finalizer aborts with
/// `ScopeViolation` and DOESN'T commit.
#[test]
fn scope_guard_blocks_out_of_scope_modification() {
    use super::{finalize_run, FinalizeError, FinalizeInput};
    let repo = scope_test_setup_repo(
        "FT-201",
        &["crates/feature_201/lib.rs"],
        "initial impl",
    );
    // A DIFFERENT feature added the unrelated file later — FT-201's
    // allowlist must not include it.
    std::fs::create_dir_all(repo.join("crates/unrelated")).unwrap();
    std::fs::write(
        repo.join("crates/unrelated/lib.rs"),
        "// touched by FT-other\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .current_dir(&repo)
        .args(["add", "-A"])
        .output()
        .unwrap();
    std::process::Command::new("git")
        .current_dir(&repo)
        .args(["commit", "-m", "[FT-other] unrelated touch", "--no-gpg-sign"])
        .output()
        .unwrap();
    // Worker (running for FT-201 with defect feedback) modifies the
    // unrelated file. That should be blocked.
    std::fs::write(
        repo.join("crates/unrelated/lib.rs"),
        "// worker stomped on this\n",
    )
    .unwrap();
    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-201",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "stomp",
        defect_scoped: true,
    };
    let err = finalize_run(&input).expect_err("expected ScopeViolation");
    match err {
        FinalizeError::ScopeViolation { paths } => {
            assert!(
                paths.iter().any(|p| p.contains("unrelated")),
                "paths: {paths:?}"
            );
        }
        other => panic!("expected ScopeViolation, got {other:?}"),
    }
}

/// New-file additions are unconditionally allowed even with the
/// scope guard active — workers can ADD support files freely; the
/// guard only stops modifications/deletes to existing out-of-scope
/// files.
#[test]
fn scope_guard_allows_new_file_additions() {
    use super::{finalize_run, FinalizeInput};
    let repo = scope_test_setup_repo("FT-202", &["crates/feature_202/lib.rs"], "initial");
    // Worker adds a brand-new file the feature's history never saw.
    std::fs::create_dir_all(repo.join("crates/feature_202")).unwrap();
    std::fs::write(repo.join("crates/feature_202/new_helper.rs"), "// new\n").unwrap();
    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-202",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "add helper",
        defect_scoped: true,
    };
    let outcome = finalize_run(&input).expect("finalize succeeds for added file");
    assert!(outcome.commit_sha.is_some());
}

/// Scope-guard bypass for first-round defect-fix on a feature with no
/// prior `[FT-XXX]` commits. This happens when a fresh feature's VGA
/// auto-ran on round 0 and emitted defects (no scripts / code yet);
/// round 1 dispatches the implementer with defect_scoped=true but
/// the allowlist would be empty, trapping the very first
/// implementation. Bypass is required so the loop can converge.
#[test]
fn scope_guard_bypasses_when_no_prior_feature_commits_exist() {
    use super::{finalize_run, FinalizeInput};
    use std::process::Command;
    // Repo with an [FT-other] commit so `git log` is non-empty, but
    // NO `[FT-300]` commits.
    let base = std::env::temp_dir().join(format!(
        "decision-cli-scope-empty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(&base)
            .args(args)
            .output()
            .expect("git command")
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "test@test"]);
    run(&["config", "user.name", "test"]);
    run(&["config", "commit.gpgsign", "false"]);
    std::fs::create_dir_all(base.join("crates/existing")).unwrap();
    std::fs::write(base.join("crates/existing/lib.rs"), "// seed\n").unwrap();
    run(&["add", "-A"]);
    Command::new("git")
        .current_dir(&base)
        .args(["commit", "-m", "[FT-other] unrelated", "--no-gpg-sign"])
        .output()
        .unwrap();
    // Implementer dispatched for FT-300 (no prior FT-300 commits) with
    // defect_scoped=true (VGA's round-0 auto-run produced defects).
    // Worker modifies an existing file — under the buggy logic this
    // would abort. Under the fix it commits.
    std::fs::write(
        base.join("crates/existing/lib.rs"),
        "// implementer's initial pass on FT-300\n",
    )
    .unwrap();
    let input = FinalizeInput {
        repo_root: &base,
        product_root: &base,
        feature_id: "FT-300",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "initial impl with defects",
        defect_scoped: true,
    };
    let outcome = finalize_run(&input).expect("finalize succeeds for empty-allowlist case");
    assert!(
        outcome.commit_sha.is_some(),
        "expected commit, got outcome: {outcome:?}"
    );
}

/// Scope-guard bypass for the FT-114 shape: prior `[FT-XXX]`
/// commits exist but only authored artifact-graph files
/// (`.product/...` specs, ADRs, TCs). The legitimate first
/// implementation modifying existing `crates/...` code must NOT be
/// rejected — there is no "old implementation" to protect yet.
/// Witnessed concretely on FT-114: two `[FT-114]` commits, both
/// spec-only; round-1 defect dispatch wanted to modify
/// `crates/decision-cli/src/features/init/*.rs` and the guard
/// flagged every edit out-of-scope.
#[test]
fn scope_guard_bypasses_when_prior_commits_are_spec_only() {
    use super::{finalize_run, FinalizeInput};
    // Repo seeded with two `[FT-300]` commits that touch only
    // .product/... — mirrors a spec-authored-but-unimplemented
    // feature exactly. Implementation files exist in the tree
    // (committed under a different feature tag).
    let repo = scope_test_setup_repo(
        "FT-other",
        &["crates/decision-cli/src/features/init/mod.rs"],
        "init shipped earlier under a different feature",
    );
    use std::process::Command;
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .expect("git command")
    };
    // First `[FT-300]` commit: spec authoring only.
    std::fs::create_dir_all(repo.join(".product/features")).unwrap();
    std::fs::write(
        repo.join(".product/features/FT-300-spec.md"),
        "# FT-300 spec\n",
    )
    .unwrap();
    run(&["add", "-A"]);
    Command::new("git")
        .current_dir(&repo)
        .args([
            "commit",
            "-m",
            "[FT-300] author feature spec",
            "--no-gpg-sign",
        ])
        .output()
        .unwrap();
    // Second `[FT-300]` commit: TCs added, still spec-only.
    std::fs::create_dir_all(repo.join(".product/tests")).unwrap();
    std::fs::write(repo.join(".product/tests/TC-700.md"), "# TC-700\n").unwrap();
    run(&["add", "-A"]);
    Command::new("git")
        .current_dir(&repo)
        .args([
            "commit",
            "-m",
            "[FT-300] add TC-700 acceptance criterion",
            "--no-gpg-sign",
        ])
        .output()
        .unwrap();
    // Worker (defect_scoped=true, round 1) writes its initial
    // implementation: modifies the existing init module that
    // FT-300 extends. Under the old bypass condition (allowlist
    // non-empty) this would abort with ScopeViolation.
    std::fs::write(
        repo.join("crates/decision-cli/src/features/init/mod.rs"),
        "// FT-300 initial implementation: auto-bootstrap\n",
    )
    .unwrap();
    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-300",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "initial impl after spec-only history",
        defect_scoped: true,
    };
    let outcome =
        finalize_run(&input).expect("finalize succeeds: spec-only history bypasses guard");
    assert!(
        outcome.commit_sha.is_some(),
        "expected commit, got outcome: {outcome:?}"
    );
}

/// `.dec/` and `.product/` modifications are always permitted under
/// the scope guard, even when the guard is otherwise active. The
/// orchestration store is harness output (not worker output) and
/// must never be flagged. The artifact graph is touched by feature
/// status transitions and cross-cutting work.
#[test]
fn scope_guard_permits_system_path_modifications() {
    use super::{finalize_run, FinalizeInput};
    // Active guard: a real `[FT-400]` code commit exists.
    let repo = scope_test_setup_repo(
        "FT-400",
        &["crates/feature_400/lib.rs"],
        "initial impl committed",
    );
    // Worker modifies the in-scope code file (allowed by
    // allowlist), AND the harness mutates .dec/store and the
    // artifact graph — the latter two must not be flagged.
    std::fs::write(
        repo.join("crates/feature_400/lib.rs"),
        "// targeted defect fix\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.join(".dec/store")).unwrap();
    std::fs::write(
        repo.join(".dec/store/orchestration.nq"),
        "<a> <b> <c> <g> .\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.join(".product/features")).unwrap();
    std::fs::write(
        repo.join(".product/features/FT-400-spec.md"),
        "# FT-400\nstatus: complete\n",
    )
    .unwrap();
    // Sanity check: those two files exist before finalize, were
    // not in the prior commit, and would be flagged without the
    // system-path exemption.
    use std::process::Command;
    let _ = Command::new("git")
        .current_dir(&repo)
        .args(["add", "-N", ".dec/store/orchestration.nq"])
        .output()
        .unwrap();
    let _ = Command::new("git")
        .current_dir(&repo)
        .args(["add", "-N", ".product/features/FT-400-spec.md"])
        .output()
        .unwrap();
    let input = FinalizeInput {
        repo_root: &repo,
        product_root: &repo,
        feature_id: "FT-400",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "abc",
        worker_summary: "in-scope fix plus harness bookkeeping",
        defect_scoped: true,
    };
    let outcome = finalize_run(&input)
        .expect("finalize succeeds: .dec/ and .product/ are system paths");
    assert!(
        outcome.commit_sha.is_some(),
        "expected commit, got outcome: {outcome:?}"
    );
}

/// Build a fresh git repo with one `[FT-XXX]` commit that touches
/// `files`. Returns the repo path. Each file gets created with
/// trivial contents so the commit succeeds.
fn scope_test_setup_repo(
    feature_id: &str,
    files: &[&str],
    msg: &str,
) -> std::path::PathBuf {
    use std::process::Command;
    let base = std::env::temp_dir().join(format!(
        "decision-cli-scope-{feature_id}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(&base)
            .args(args)
            .output()
            .expect("git command")
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "test@test"]);
    run(&["config", "user.name", "test"]);
    run(&["config", "commit.gpgsign", "false"]);
    for f in files {
        let path = base.join(f);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "// seed\n").unwrap();
    }
    run(&["add", "-A"]);
    Command::new("git")
        .current_dir(&base)
        .args(["commit", "-m", &format!("[{feature_id}] {msg}"), "--no-gpg-sign"])
        .output()
        .unwrap();
    base
}
