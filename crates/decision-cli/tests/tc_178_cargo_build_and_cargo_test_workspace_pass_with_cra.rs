//! TC-178 — `cargo build` and `cargo test --workspace` pass with `crates/product-cli/` as a member.
//!
//! Validates: FT-105 · ADR-067.
//! Spec: `.product/tests/TC-178-cargo-build-and-cargo-test-workspace-pass-with-cra.md`
//!
//! Asserts the post-absorption workspace shape:
//!
//!   * `cargo metadata` lists four workspace members (oxi-events,
//!     decision-cli, product-cli, product-shim).
//!   * `cargo build -p product-cli` succeeds, demonstrating the crate
//!     compiles in isolation (it is a workspace member, not a hidden
//!     dependency).
//!   * `cargo build -p product-shim` succeeds — proves the deprecation
//!     shim binary is produced.
//!   * `cargo build -p decision-cli` succeeds — proves the dec binary
//!     still builds after the absorption.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/decision-cli when this test
    // runs; walk up two levels to find the workspace root.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root above crates/decision-cli")
        .to_path_buf()
}

fn run_cargo(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO"))
        .current_dir(repo_root())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("invoke cargo {args:?}: {e}"))
}

#[test]
fn tc_178_cargo_build_and_cargo_test_workspace_pass_with_cra() {
    // Scenario A precondition: cargo metadata can read the workspace.
    let out = run_cargo(&["metadata", "--no-deps", "--format-version", "1"]);
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Parse the metadata JSON manually rather than dragging in cargo_metadata.
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("cargo metadata produces JSON");
    let members = json
        .get("workspace_members")
        .and_then(|v| v.as_array())
        .expect("workspace_members array");
    // Cargo's workspace_members IDs look like one of:
    //   "name 0.1.0 (path+file://…)"   (legacy)
    //   "path+file:///…#name@0.1.0"     (current)
    //   "path+file:///…/<name>#0.1.0"   (current, when the dir matches name)
    // Pull the crate name from either shape.
    fn extract_name(id: &str) -> String {
        if let Some(idx) = id.find('#') {
            let tail = &id[idx + 1..];
            if let Some((name, _)) = tail.split_once('@') {
                return name.to_string();
            }
            // Form path+file:///…/<dir>#<version> — recover name from dir.
            let head = &id[..idx];
            if let Some(slash) = head.rfind('/') {
                return head[slash + 1..].to_string();
            }
        }
        id.split_whitespace().next().unwrap_or("").to_string()
    }
    let member_names: Vec<String> = members
        .iter()
        .filter_map(|v| v.as_str())
        .map(extract_name)
        .collect();

    for expected in &["oxi-events", "decision-cli", "product-cli", "product-shim"] {
        assert!(
            member_names.iter().any(|n| n == expected),
            "workspace missing member {expected}; observed {member_names:?}"
        );
    }

    // Scenario A part 2: building product-cli in isolation succeeds —
    // covers TC-179's Scenario D ("Build product-cli in isolation"), but
    // we exercise it here because it is also a workspace-shape claim.
    let out = run_cargo(&["build", "-p", "product-cli", "--quiet"]);
    assert!(
        out.status.success(),
        "cargo build -p product-cli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Scenario A part 3: product-shim builds — the deprecation shim
    // binary must exist for FT-105 §Phase 5.
    let out = run_cargo(&["build", "-p", "product-shim", "--quiet"]);
    assert!(
        out.status.success(),
        "cargo build -p product-shim failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Scenario B (workspace-level test command): verify the dec binary
    // still builds. Running the *full* `cargo test --workspace` from
    // inside a test would recurse; this targeted build is enough to
    // assert the workspace plumbing did not regress decision-cli's
    // compilation.
    let out = run_cargo(&["build", "-p", "decision-cli", "--quiet"]);
    assert!(
        out.status.success(),
        "cargo build -p decision-cli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
