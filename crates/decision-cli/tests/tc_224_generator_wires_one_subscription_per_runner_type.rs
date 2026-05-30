//! TC-224 — Generator wires one subscription per unique TC runner type observed in .product/tests.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::DefinitionSource;

#[test]
fn tc_224_generator_wires_one_subscription_per_runner_type() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("test-repo");
    let product_dir = repo_root.join(".product");
    let tests_dir = product_dir.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(product_dir.join("product.toml"), "name = \"test-repo\"\n").unwrap();

    // Create 7 TCs: cargo-test (×3), bash (×2), pytest (×1), one with no runner
    fs::write(
        tests_dir.join("TC-001.md"),
        "---\nrunner: cargo-test\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-002.md"),
        "---\nrunner: cargo-test\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-003.md"),
        "---\nrunner: cargo-test\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-004.md"),
        "---\nrunner: bash\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-005.md"),
        "---\nrunner: bash\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("TC-006.md"),
        "---\nrunner: pytest\n---\n# Test\n",
    )
    .unwrap();
    fs::write(tests_dir.join("TC-007.md"), "---\n---\n# Test with no runner\n").unwrap();

    let result = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result.is_ok(), "Init should succeed");

    let stream_path = repo_root
        .join(".dec")
        .join("streams")
        .join("test-repo-development.ttl");
    let ttl = fs::read_to_string(&stream_path).unwrap();

    // Check that the three unique runners are mentioned in comments
    assert!(
        ttl.contains("# - bash"),
        "Generated stream should mention bash runner"
    );
    assert!(
        ttl.contains("# - cargo-test"),
        "Generated stream should mention cargo-test runner"
    );
    assert!(
        ttl.contains("# - pytest"),
        "Generated stream should mention pytest runner"
    );

    // Count runner mentions (should be exactly 3, not 6 or 7)
    let runner_count = ttl.matches("# - ").count();
    assert_eq!(
        runner_count, 3,
        "Should have exactly 3 unique runners mentioned, got {runner_count}"
    );

    // Test with unknown runner
    let tmp2 = TempDir::new().unwrap();
    let repo_root2 = tmp2.path().join("test-repo2");
    let product_dir2 = repo_root2.join(".product");
    let tests_dir2 = product_dir2.join("tests");
    fs::create_dir_all(&tests_dir2).unwrap();
    fs::write(product_dir2.join("product.toml"), "name = \"test-repo2\"\n").unwrap();

    fs::write(
        tests_dir2.join("TC-001.md"),
        "---\nrunner: deno-test\n---\n# Test with unknown runner\n",
    )
    .unwrap();

    let result2 = decision_cli::init::run(&repo_root2, DefinitionSource::AutoDiscover);
    // Should succeed even with unknown runner
    assert!(
        result2.is_ok(),
        "Init should succeed with unknown runner: {result2:?}"
    );

    let stream_path2 = repo_root2
        .join(".dec")
        .join("streams")
        .join("test-repo2-development.ttl");
    let ttl2 = fs::read_to_string(&stream_path2).unwrap();

    // Unknown runner should be mentioned
    assert!(
        ttl2.contains("# - deno-test"),
        "Unknown runner should still be mentioned in comments"
    );
}
