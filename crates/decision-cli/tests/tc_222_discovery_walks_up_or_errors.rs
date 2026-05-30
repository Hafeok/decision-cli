//! TC-222 — Discovery walks up from workdir to find .product/ and errors clearly when none found.

use std::fs;
use std::path::PathBuf;

use decision_cli::init::{DefinitionSource, InitError};
use tempfile::TempDir;

#[test]
fn tc_222_discovery_walks_up_or_errors() {
    // 1. Walk-up success: .product/ exists in parent directory
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("some-repo");
    let product_dir = repo_root.join(".product");
    let nested_dir = repo_root.join("crates").join("inner");
    fs::create_dir_all(&product_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(product_dir.join("product.toml"), "name = \"test-repo\"\n").unwrap();

    let result = decision_cli::init::run(&nested_dir, DefinitionSource::AutoDiscover);
    assert!(
        result.is_ok(),
        "Discovery should succeed when .product/ exists in ancestor"
    );

    // 2. Workdir-direct success: .product/ exists at workdir
    let tmp2 = TempDir::new().unwrap();
    let repo_root2 = tmp2.path().join("some-repo");
    let product_dir2 = repo_root2.join(".product");
    fs::create_dir_all(&product_dir2).unwrap();
    fs::write(product_dir2.join("product.toml"), "name = \"test-repo\"\n").unwrap();

    let result2 = decision_cli::init::run(&repo_root2, DefinitionSource::AutoDiscover);
    assert!(
        result2.is_ok(),
        "Discovery should succeed when .product/ exists at workdir"
    );

    // 3. Not-found error: no .product/ anywhere
    let tmp3 = TempDir::new().unwrap();
    let empty_dir = tmp3.path().join("empty-dir");
    fs::create_dir_all(&empty_dir).unwrap();

    let result3 = decision_cli::init::run(&empty_dir, DefinitionSource::AutoDiscover);
    assert!(result3.is_err(), "Discovery should fail when no .product/ found");

    match result3 {
        Err(InitError::Internal(msg)) => {
            assert!(
                msg.contains("No .product/"),
                "Error should mention .product/: {msg}"
            );
            assert!(
                msg.contains("product feature new"),
                "Error should suggest remediation: {msg}"
            );
        }
        other => panic!("Expected Internal error, got: {other:?}"),
    }
}
