use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_product_status_help() {
    let mut cmd = Command::cargo_bin("dec").unwrap();
    cmd.args(&["product", "status", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Show the status of the project"));
}

#[test]
fn test_product_status_text_format() {
    let mut cmd = Command::cargo_bin("dec").unwrap();
    cmd.args(&["product", "status", "--format", "text"]);
    // We expect this to fail because we don't have a valid product config,
    // but it should fail with a graph-load error, not a subcommand error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("graph load"));
}

#[test]
fn test_product_status_json_format() {
    let mut cmd = Command::cargo_bin("dec").unwrap();
    cmd.args(&["product", "status", "--format", "json"]);
    // We expect this to fail because we don't have a valid product config,
    // but it should fail with a graph-load error, not a subcommand error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("graph load"));
}

#[test]
fn test_product_status_invalid_format() {
    let mut cmd = Command::cargo_bin("dec").unwrap();
    cmd.args(&["product", "status", "--format", "invalid"]);
    // Should fail with usage error since invalid format is provided
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_product_status_with_phase() {
    let mut cmd = Command::cargo_bin("dec").unwrap();
    cmd.args(&["product", "status", "--phase", "1"]);
    // We expect this to fail because we don't have a valid product config,
    // but it should fail with a graph-load error, not a subcommand error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("graph load"));
}