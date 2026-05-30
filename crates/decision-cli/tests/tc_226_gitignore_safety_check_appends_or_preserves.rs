//! TC-226 — Gitignore safety check appends .env line when missing in TTY mode with --yes.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::{safety, GitignoreOutcome};

#[test]
fn tc_226_gitignore_safety_check_appends_or_preserves() {
    // 1. .env already listed
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().to_path_buf();
    let gitignore = repo_root.join(".gitignore");
    fs::write(&gitignore, ".env\ntarget/\n").unwrap();

    let outcome = safety::ensure_env_gitignored(&repo_root, true).unwrap();
    assert_eq!(
        outcome,
        GitignoreOutcome::Unchanged,
        "Should report unchanged when .env already present"
    );

    let content = fs::read_to_string(&gitignore).unwrap();
    assert_eq!(content, ".env\ntarget/\n", ".gitignore should be unchanged");

    // 2. .env listed with leading slash
    let tmp2 = TempDir::new().unwrap();
    let repo_root2 = tmp2.path().to_path_buf();
    let gitignore2 = repo_root2.join(".gitignore");
    fs::write(&gitignore2, "/.env\ntarget/\n").unwrap();

    let outcome2 = safety::ensure_env_gitignored(&repo_root2, true).unwrap();
    assert_eq!(
        outcome2,
        GitignoreOutcome::Unchanged,
        "Should report unchanged for /.env"
    );

    // 3. .env missing, .gitignore exists
    let tmp3 = TempDir::new().unwrap();
    let repo_root3 = tmp3.path().to_path_buf();
    let gitignore3 = repo_root3.join(".gitignore");
    fs::write(&gitignore3, "target/\n").unwrap();

    let outcome3 = safety::ensure_env_gitignored(&repo_root3, true).unwrap();
    assert_eq!(
        outcome3,
        GitignoreOutcome::Appended,
        "Should report appended"
    );

    let content3 = fs::read_to_string(&gitignore3).unwrap();
    assert!(
        content3.contains("target/"),
        ".gitignore should preserve existing entries"
    );
    assert!(
        content3.contains(".env"),
        ".gitignore should have .env appended"
    );

    // 4. .gitignore does not exist
    let tmp4 = TempDir::new().unwrap();
    let repo_root4 = tmp4.path().to_path_buf();

    let outcome4 = safety::ensure_env_gitignored(&repo_root4, true).unwrap();
    assert_eq!(outcome4, GitignoreOutcome::Created, "Should report created");

    let gitignore4 = repo_root4.join(".gitignore");
    assert!(gitignore4.exists(), ".gitignore should be created");

    let content4 = fs::read_to_string(&gitignore4).unwrap();
    assert_eq!(content4, ".env\n", ".gitignore should contain only .env");

    // 5. .gitignore is a directory (error case)
    let tmp5 = TempDir::new().unwrap();
    let repo_root5 = tmp5.path().to_path_buf();
    let gitignore_dir = repo_root5.join(".gitignore");
    fs::create_dir(&gitignore_dir).unwrap();

    let result5 = safety::ensure_env_gitignored(&repo_root5, true);
    assert!(
        result5.is_err(),
        "Should error when .gitignore is a directory"
    );
}
