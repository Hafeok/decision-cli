use assert_cmd::Command;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;

#[test]
fn test_product_status_help() {
    Command::new("cargo")
        .args(["run", "--bin", "dec", "--", "product", "status", "--help"])
        .assert()
        .success();
}

#[test]
fn test_product_status_no_project() {
    let temp_dir = tempfile::tempdir().unwrap();
    Command::new("cargo")
        .args(["run", "--bin", "dec", "--", "product", "status"])
        .current_dir(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No product configuration found"));
}

#[test]
fn test_product_status_with_mock_project() {
    let temp_dir = tempfile::tempdir().unwrap();
    let product_dir = temp_dir.path().join(".product");
    std::fs::create_dir_all(&product_dir).unwrap();
    
    // Create a mock product config
    let mut config_file = File::create(product_dir.join("config.toml")).unwrap();
    writeln!(config_file, r#"name = "Test Project""#).unwrap();
    
    // Create a mock graph file
    let mut graph_file = File::create(product_dir.join("graph.json")).unwrap();
    writeln!(graph_file, r#"{"features": []}"#).unwrap();
    
    Command::new("cargo")
        .args(["run", "--bin", "dec", "--", "product", "status"])
        .current_dir(temp_dir.path())
        .assert()
        .success();
}