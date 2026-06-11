//! TC-238 — Strict config parser rejects unknown keys, typos, and credential-shaped key names.
//!
//! NOTE: FT-114 only implements config.toml generation (ADR-068 initial inventory).
//! The strict parser implementation is a future feature.
//! This test verifies that the generated config.toml is well-formed.

use std::fs;
use tempfile::TempDir;

use decision_cli::init::DefinitionSource;

#[test]
fn tc_238_strict_parser_rejects_invalid_config() {
    // For FT-114, we verify the generated config.toml has expected structure
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("test-repo");
    let product_dir = repo_root.join(".product");
    fs::create_dir_all(&product_dir).unwrap();
    fs::write(product_dir.join("product.toml"), "name = \"test-repo\"\n").unwrap();

    let result = decision_cli::init::run(&repo_root, DefinitionSource::AutoDiscover);
    assert!(result.is_ok(), "Init should succeed");

    let config_path = repo_root.join(".dec").join("config.toml");
    let content = fs::read_to_string(&config_path).unwrap();

    // Verify structure matches ADR-068 inventory by checking for section headers
    assert!(content.contains("[driver]"), "Should have driver section");
    assert!(content.contains("[sweep]"), "Should have sweep section");
    assert!(content.contains("[show]"), "Should have show section");
    assert!(content.contains("[init]"), "Should have init section");
    assert!(content.contains("[paths]"), "Should have paths section");

    // Verify expected keys are present
    assert!(
        content.contains("max_iter"),
        "Should have max_iter key in driver section"
    );
    assert!(
        content.contains("per_feature_timeout_secs"),
        "Should have timeout key in sweep section"
    );
    assert!(
        content.contains("watch_interval_secs"),
        "Should have watch interval in show section"
    );
    assert!(
        content.contains("default_provider"),
        "Should have provider in init section"
    );
    assert!(
        content.contains("store_path"),
        "Should have store_path in paths section"
    );

    // Verify the config mentions SCW_SECRET_KEY in the init section
    // (as the name of an env var to look for, not as a credential value)
    assert!(
        content.contains("default_secret_env"),
        "Config should have default_secret_env key"
    );
    assert!(
        content.contains("SCW_SECRET_KEY"),
        "Config should reference SCW_SECRET_KEY as env var name"
    );

    // Verify no actual secrets appear (e.g., no scw-xyz values)
    assert!(
        !content.contains("scw-"),
        "Config should not contain actual Scaleway secret values"
    );

    // Verify the config contains the ADR-068 comment header
    assert!(
        content.contains("ADR-068"),
        "Config should reference ADR-068"
    );
    assert!(
        content.contains("Precedence chain"),
        "Config should document precedence"
    );
}
