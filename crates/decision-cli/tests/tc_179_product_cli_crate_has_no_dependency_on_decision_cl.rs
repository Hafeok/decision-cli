//! TC-179 — product-cli crate has no dependency on decision-cli or oxi-events.
//!
//! Validates: FT-105 · ADR-067 · ADR-001.
//! Spec: `.product/tests/TC-179-product-cli-crate-has-no-dependency-on-decision-cl.md`
//!
//! SDP fitness check. The dependency direction must be
//! `decision-cli → product-cli → (nothing in this workspace)`. Asserted
//! three ways:
//!
//!   1. Cargo.toml structural check — no decision-cli / oxi-events
//!      named in any dependencies section.
//!   2. Source-level grep — no `use decision_cli::*` / `use oxi_events::*`
//!      and no fully-qualified `decision_cli::` / `oxi_events::` paths.
//!   3. `cargo metadata` confirms product-cli's resolved dependency set
//!      contains neither of the two crates.
//!
//! Plus the reverse-direction sanity check: decision-cli DOES declare a
//! path dependency on product-cli, so `dec product *` and the MCP merge
//! actually have something to call into.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/decision-cli")
        .to_path_buf()
}

fn read_cargo_toml(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

const FORBIDDEN_DEP_NAMES: &[&str] = &["decision-cli", "decision_cli", "oxi-events", "oxi_events"];

fn assert_no_forbidden_dep_lines(cargo_toml: &str) {
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        for forbidden in FORBIDDEN_DEP_NAMES {
            // Match a Cargo.toml table key shape: `name = …` or
            // `name.workspace = …` — must be followed by `=` or `.`.
            // Avoid false positives inside descriptions ("decision-cli"
            // appears in the package description text).
            let pat_eq = format!("{forbidden} =");
            let pat_dot = format!("{forbidden}.");
            if trimmed.starts_with(&pat_eq) || trimmed.starts_with(&pat_dot) {
                panic!(
                    "product-cli Cargo.toml has forbidden dependency '{forbidden}': line: {trimmed}"
                );
            }
        }
    }
}

fn assert_no_forbidden_use_paths(src_dir: &Path) {
    let forbidden_substrings = [
        "use decision_cli",
        "use oxi_events",
        "decision_cli::",
        "oxi_events::",
    ];
    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            let ftype = entry.file_type().expect("file_type");
            if ftype.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let body = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            for line in body.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("//!") {
                    continue;
                }
                for forbidden in forbidden_substrings {
                    assert!(
                        !line.contains(forbidden),
                        "{}:{} contains forbidden import path '{forbidden}': {line}",
                        path.display(),
                        forbidden
                    );
                }
            }
        }
    }
}

#[test]
fn tc_179_product_cli_crate_has_no_dependency_on_decision_cl() {
    // Scenario A — structural Cargo.toml check.
    let product_cli_toml = read_cargo_toml("crates/product-cli/Cargo.toml");
    assert_no_forbidden_dep_lines(&product_cli_toml);

    // Scenario B — source-level grep.
    let src = repo_root().join("crates/product-cli/src");
    assert!(
        src.exists(),
        "crates/product-cli/src does not exist; absorption skeleton incomplete"
    );
    assert_no_forbidden_use_paths(&src);

    // Scenario C — cargo metadata confirms the resolved dependency set
    // does not name decision-cli or oxi-events.
    let out = Command::new(env!("CARGO"))
        .current_dir(repo_root())
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .expect("invoke cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("cargo metadata produces JSON");
    let packages = json
        .get("packages")
        .and_then(|v| v.as_array())
        .expect("packages array");
    let product_pkg = packages
        .iter()
        .find(|p| p.get("name").and_then(|n| n.as_str()) == Some("product-cli"))
        .expect("product-cli package present in cargo metadata");
    let deps = product_pkg
        .get("dependencies")
        .and_then(|v| v.as_array())
        .expect("product-cli has a dependencies array");
    for dep in deps {
        let name = dep.get("name").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            !FORBIDDEN_DEP_NAMES.contains(&name),
            "product-cli depends on forbidden crate '{name}'"
        );
    }

    // Scenario E — reverse direction sanity: decision-cli does declare a
    // dependency on product-cli (path or workspace form). Without this
    // edge, `dec product *` and the MCP merge would be vapor.
    let decision_toml = read_cargo_toml("crates/decision-cli/Cargo.toml");
    let names_product_dep = decision_toml.lines().any(|line| {
        let t = line.trim();
        t.starts_with("product-cli =") || t.starts_with("product-cli.")
    });
    assert!(
        names_product_dep,
        "crates/decision-cli/Cargo.toml does not declare a product-cli dependency"
    );
}
