//! TC-223 — Generated value-stream is byte-deterministic given identical .product/ inputs.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::DefinitionSource;

#[test]
fn tc_223_generated_value_stream_is_deterministic() {
    // Create a temp .product/ with three TCs
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("some-repo");
    let product_dir = repo_root.join(".product");
    let tests_dir = product_dir.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(product_dir.join("product.toml"), "name = \"some-repo\"\n").unwrap();

    // Create three TCs with different runners
    fs::write(
        tests_dir.join("TC-001-cargo.md"),
        "---\nrunner: cargo-test\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-002-bash.md"),
        "---\nrunner: bash\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-003-pytest.md"),
        "---\nrunner: pytest\n---\n# Test\n",
    )
    .unwrap();

    // Generate first time
    let result1 = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result1.is_ok(), "First init should succeed");

    let stream_path1 = repo_root
        .join(".dec")
        .join("streams")
        .join("some-repo-development.ttl");
    assert!(stream_path1.exists(), "Generated stream should be saved");
    let ttl1 = fs::read_to_string(&stream_path1).unwrap();

    // Clear the .dec directory
    fs::remove_dir_all(repo_root.join(".dec")).unwrap();

    // Generate second time
    let result2 = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result2.is_ok(), "Second init should succeed");

    let stream_path2 = repo_root
        .join(".dec")
        .join("streams")
        .join("some-repo-development.ttl");
    let ttl2 = fs::read_to_string(&stream_path2).unwrap();

    // Compare without timestamps (they'll differ)
    let ttl1_no_ts: Vec<_> = ttl1.lines().filter(|l| !l.contains("Timestamp:")).collect();
    let ttl2_no_ts: Vec<_> = ttl2.lines().filter(|l| !l.contains("Timestamp:")).collect();

    assert_eq!(
        ttl1_no_ts, ttl2_no_ts,
        "Generated streams should be deterministic (except timestamp)"
    );

    // Test with zero TCs
    let tmp2 = TempDir::new().unwrap();
    let repo_root2 = tmp2.path().join("empty-repo");
    let product_dir2 = repo_root2.join(".product");
    fs::create_dir_all(&product_dir2).unwrap();
    fs::write(product_dir2.join("product.toml"), "name = \"empty-repo\"\n").unwrap();

    let result3 = decision_cli::init::run(&repo_root2, DefinitionSource::AutoDiscover);
    assert!(
        result3.is_ok(),
        "Init should succeed with zero TCs: {result3:?}"
    );

    // Clear and regenerate
    fs::remove_dir_all(repo_root2.join(".dec")).unwrap();
    let result4 = decision_cli::init::run(&repo_root2, DefinitionSource::AutoDiscover);
    assert!(result4.is_ok(), "Second init should succeed with zero TCs");

    let stream_path3 = repo_root2
        .join(".dec")
        .join("streams")
        .join("empty-repo-development.ttl");
    let ttl3 = fs::read_to_string(&stream_path3).unwrap();

    let stream_path4 = repo_root2
        .join(".dec")
        .join("streams")
        .join("empty-repo-development.ttl");
    fs::remove_dir_all(repo_root2.join(".dec")).unwrap();
    let _ = decision_cli::init::run(&repo_root2, DefinitionSource::AutoDiscover);
    let ttl4 = fs::read_to_string(&stream_path4).unwrap();

    let ttl3_no_ts: Vec<_> = ttl3.lines().filter(|l| !l.contains("Timestamp:")).collect();
    let ttl4_no_ts: Vec<_> = ttl4.lines().filter(|l| !l.contains("Timestamp:")).collect();

    assert_eq!(
        ttl3_no_ts, ttl4_no_ts,
        "Zero-TC streams should also be deterministic"
    );
}
