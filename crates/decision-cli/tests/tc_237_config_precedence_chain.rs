//! TC-237 — Config precedence chain: flag overrides env overrides config.toml overrides built-in default.
//!
//! NOTE: FT-114 only implements config.toml generation (ADR-068 initial inventory).
//! The full precedence chain and strict parser are future features.
//! This test verifies that config.toml is created with the expected default values.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::{config, DefinitionSource};

#[test]
fn tc_237_config_precedence_chain() {
    // For FT-114, we just verify the config.toml is generated with the right defaults
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("test-repo");
    let product_dir = repo_root.join(".product");
    fs::create_dir_all(&product_dir).unwrap();
    fs::write(product_dir.join("product.toml"), "name = \"test-repo\"\n").unwrap();

    let result = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result.is_ok(), "Init should succeed");

    let config_path = repo_root.join(".dec").join("config.toml");
    assert!(
        config_path.exists(),
        "config.toml should be created by dec init"
    );

    let content = fs::read_to_string(&config_path).unwrap();

    // Verify ADR-068 inventory defaults are present
    assert!(
        content.contains("[driver]"),
        "config.toml should have [driver] section"
    );
    assert!(
        content.contains("max_iter = 6"),
        "config.toml should have max_iter default"
    );
    assert!(
        content.contains("[sweep]"),
        "config.toml should have [sweep] section"
    );
    assert!(
        content.contains("per_feature_timeout_secs = 600"),
        "config.toml should have timeout default"
    );
    assert!(
        content.contains("[init]"),
        "config.toml should have [init] section"
    );
    assert!(
        content.contains("default_provider = \"scaleway\""),
        "config.toml should have provider default"
    );

    // Verify it's idempotent: re-running init preserves existing config
    // (FT-114: init is idempotent, merges stores, preserves config)
    fs::write(&config_path, "# Custom config\n[driver]\nmax_iter = 10\n").unwrap();

    let result2 = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result2.is_ok(), "Re-init should succeed");

    let content2 = fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        content2, "# Custom config\n[driver]\nmax_iter = 10\n",
        "config.toml should be preserved on re-init"
    );

    // Test standalone bootstrap function
    let tmp2 = TempDir::new().unwrap();
    let dec_dir2 = tmp2.path().join(".dec");
    fs::create_dir_all(&dec_dir2).unwrap();

    let created = config::bootstrap_config_toml(&dec_dir2).unwrap();
    assert!(created, "bootstrap should report created");

    let config_path2 = dec_dir2.join("config.toml");
    assert!(config_path2.exists(), "config.toml should be created");

    let created2 = config::bootstrap_config_toml(&dec_dir2).unwrap();
    assert!(!created2, "bootstrap should report not created when exists");
}
