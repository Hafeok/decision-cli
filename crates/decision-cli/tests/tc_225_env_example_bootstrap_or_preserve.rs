//! TC-225 — .env.example is written when absent and left untouched when present.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::safety;

#[test]
fn tc_225_env_example_bootstrap_or_preserve() {
    // 1. Bootstrap path: no .env.example exists
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().to_path_buf();

    let created = safety::bootstrap_env_example(&repo_root).unwrap();
    assert!(created, "Should report that .env.example was created");

    let env_example = repo_root.join(".env.example");
    assert!(env_example.exists(), ".env.example should be created");

    let content = fs::read_to_string(&env_example).unwrap();
    assert!(
        content.contains("SCW_SECRET_KEY="),
        ".env.example should contain SCW_SECRET_KEY"
    );
    assert!(
        content.contains(".env is gitignored"),
        ".env.example should mention gitignore"
    );

    // 2. Preserve path: .env.example already exists with custom content
    let tmp2 = TempDir::new().unwrap();
    let repo_root2 = tmp2.path().to_path_buf();
    let env_example2 = repo_root2.join(".env.example");

    let custom_content = "ACME_CUSTOM_PROVIDER_KEY=foo\n# Custom comment\n";
    fs::write(&env_example2, custom_content).unwrap();

    let created2 = safety::bootstrap_env_example(&repo_root2).unwrap();
    assert!(!created2, "Should report that .env.example was not created");

    let content2 = fs::read_to_string(&env_example2).unwrap();
    assert_eq!(content2, custom_content, ".env.example should be unchanged");

    // 3. Verify no .env file is created
    let env_file = repo_root.join(".env");
    assert!(
        !env_file.exists(),
        ".env should NOT be created by bootstrap_env_example"
    );
}
