//! FT-136 / TC-329 — dec product forwards to product_core
//!
//! Validates that `dec product feature show` loads the KnowledgeGraph via
//! product_core and surfaces the artifact's identifying fields.

use std::process::Command;

#[test]
fn feature_show_returns_artifact() {
    // Run `dec product feature show FT-001` and verify it returns identifying fields
    let output = Command::new("dec")
        .args(["product", "feature", "show", "FT-001"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product feature show");

    assert!(
        output.status.success(),
        "dec product feature show FT-001 failed with: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-329 acceptance criteria: output contains FT-001 and the feature title
    assert!(
        stdout.contains("FT-001"),
        "Output should contain feature ID FT-001"
    );

    // The actual FT-001 is "oxi-events: GraphWriter mutation chokepoint"
    // Check for key substring
    assert!(
        stdout.contains("oxi-events") || stdout.contains("GraphWriter"),
        "Output should contain recognizable substring from feature title, got: {stdout}"
    );
}

#[test]
fn feature_show_nonexistent_fails() {
    let output = Command::new("dec")
        .args(["product", "feature", "show", "FT-NONEXISTENT"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product feature show");

    // TC-329 acceptance criteria: exits non-zero
    assert!(
        !output.status.success(),
        "dec product feature show FT-NONEXISTENT should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "Should print diagnostic to stderr when feature not found"
    );
}

#[test]
fn feature_list_contains_features() {
    let output = Command::new("dec")
        .args(["product", "feature", "list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product feature list");

    assert!(
        output.status.success(),
        "dec product feature list failed with: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-332 acceptance criteria: contains at least one feature ID
    assert!(
        stdout.contains("FT-"),
        "Output should contain at least one feature ID"
    );

    // Should contain phase and status indicators
    assert!(
        stdout.contains("phase") || stdout.contains("status"),
        "Output should contain phase or status indicators"
    );
}

#[test]
fn adr_show_returns_artifact() {
    let output = Command::new("dec")
        .args(["product", "adr", "show", "ADR-009"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product adr show");

    assert!(
        output.status.success(),
        "dec product adr show ADR-009 failed with: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-334 acceptance criteria: output contains ADR-009 and title substring
    assert!(
        stdout.contains("ADR-009"),
        "Output should contain ADR ID ADR-009"
    );

    // ADR-009 title contains "product-cli" or "subprocess"
    assert!(
        stdout.contains("product-cli") || stdout.contains("subprocess"),
        "Output should contain recognizable substring from ADR title, got: {stdout}"
    );
}

#[test]
fn context_assembles_bundle() {
    let output = Command::new("dec")
        .args(["product", "context", "FT-001"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product context");

    assert!(
        output.status.success(),
        "dec product context FT-001 failed with: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-336 acceptance criteria: contains feature ID and has assembled content
    assert!(
        stdout.contains("FT-001"),
        "Output should contain feature ID FT-001"
    );

    // Evidence of assembled bundle: section header, ADR reference, or YAML block
    let has_bundle_evidence =
        stdout.contains('#') || stdout.contains("ADR-") || stdout.contains("---");

    assert!(
        has_bundle_evidence,
        "Output should contain evidence of an assembled bundle (headers/refs/yaml), got: {stdout}"
    );
}

#[test]
fn preflight_reports_coverage() {
    let output = Command::new("dec")
        .args(["product", "preflight", "FT-001"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product preflight");

    assert!(
        output.status.success(),
        "dec product preflight FT-001 should succeed (findings are not errors)"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-337 acceptance criteria: contains feature ID and preflight output
    assert!(
        stdout.contains("FT-001"),
        "Output should contain feature ID FT-001"
    );

    // Evidence of preflight output: gap/coverage/clean/domain
    let has_preflight_evidence = stdout.contains("gap")
        || stdout.contains("coverage")
        || stdout.contains("clean")
        || stdout.contains("domain");

    assert!(
        has_preflight_evidence,
        "Output should contain evidence of preflight check, got: {stdout}"
    );
}

#[test]
fn graph_check_audits_graph() {
    let output = Command::new("dec")
        .args(["product", "graph", "check"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product graph check");

    // TC-338 acceptance criteria: exits 0 if clean, non-zero if errors
    // Check that output contains evidence of the check
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    let has_check_evidence = combined.contains("error")
        || combined.contains("warning")
        || combined.contains("clean")
        || combined.contains("Graph check");

    assert!(
        has_check_evidence,
        "Output should contain evidence of graph check, got stdout: {stdout}, stderr: {stderr}"
    );
}

#[test]
fn graph_stats_reports_counts() {
    let output = Command::new("dec")
        .args(["product", "graph", "stats"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run dec product graph stats");

    assert!(
        output.status.success(),
        "dec product graph stats failed with: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // TC-339 acceptance criteria: contains numeric output and artifact type references
    let has_numbers = stdout.chars().any(|c| c.is_ascii_digit());
    assert!(has_numbers, "Output should contain numeric statistics");

    let has_artifact_types = stdout.contains("features")
        || stdout.contains("adrs")
        || stdout.contains("tests")
        || stdout.contains("patterns");

    assert!(
        has_artifact_types,
        "Output should reference artifact types, got: {stdout}"
    );
}
