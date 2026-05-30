///! Value-stream generation from .product/ discovery (FT-114).

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::InitError;

/// Generate a default value-stream definition by discovering `.product/` in workdir.
///
/// FT-114: Walks up from workdir to find product.toml, scans TCs for runner types,
/// and generates a .ttl with standard role catalog + subscriptions.
pub(super) fn generate_value_stream(workdir: &Path) -> Result<String, InitError> {
    // Discover .product/ directory
    let product_dir = discover_product_dir(workdir)?;

    // Get repo name from product.toml or directory name
    let repo_name = get_repo_name(&product_dir)?;

    // Scan TCs for runner types
    let runners = scan_tc_runners(&product_dir)?;

    // Generate the TTL
    Ok(generate_ttl(&repo_name, &runners))
}

fn discover_product_dir(start: &Path) -> Result<std::path::PathBuf, InitError> {
    let mut current = start.to_path_buf();
    loop {
        let product_dir = current.join(".product");
        let product_toml = product_dir.join("product.toml");
        if product_toml.exists() {
            return Ok(product_dir);
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return Err(InitError::Internal(
                "No .product/ graph discovered. Run `product feature new ...` first.".to_string()
            ));
        }
    }
}

fn get_repo_name(product_dir: &Path) -> Result<String, InitError> {
    let product_toml = product_dir.join("product.toml");
    let content = fs::read_to_string(&product_toml)
        .map_err(|e| InitError::Internal(format!("Failed to read product.toml: {}", e)))?;

    // Try to parse the name field from product.toml
    for line in content.lines() {
        if let Some(name) = line.strip_prefix("name") {
            if let Some(quoted) = name.trim().strip_prefix("=").and_then(|s| s.trim().strip_prefix('"')) {
                if let Some(name_val) = quoted.strip_suffix('"') {
                    return Ok(name_val.to_string());
                }
            }
        }
    }

    // Fall back to parent directory name
    Ok(product_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string())
}

fn scan_tc_runners(product_dir: &Path) -> Result<HashSet<String>, InitError> {
    let mut runners = HashSet::new();
    let tests_dir = product_dir.join("tests");

    if !tests_dir.exists() {
        return Ok(runners);
    }

    let entries = fs::read_dir(&tests_dir)
        .map_err(|e| InitError::Internal(format!("Failed to read tests directory: {}", e)))?;

    for entry in entries {
        let entry = entry.map_err(|e| InitError::Internal(format!("Failed to read directory entry: {}", e)))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(runner) = extract_runner_from_tc(&path) {
                runners.insert(runner);
            }
        }
    }

    Ok(runners)
}

fn extract_runner_from_tc(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_frontmatter = false;

    for line in content.lines() {
        if line.trim() == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter {
            if let Some(runner) = line.strip_prefix("runner:") {
                return Some(runner.trim().to_string());
            }
        }
    }
    None
}

fn generate_ttl(repo_name: &str, runners: &HashSet<String>) -> String {
    let stream_iri = format!("https://decision-cli.dev/ns/streams/{}-development", repo_name);

    let mut ttl = String::new();
    ttl.push_str(&format!("# Generated value-stream definition by dec init (FT-114).\n"));
    ttl.push_str(&format!("# Auto-discovered from .product/ graph.\n"));
    ttl.push_str(&format!("# Timestamp: {}\n\n", chrono::Utc::now().to_rfc3339()));
    ttl.push_str("@prefix dec:   <https://decision-cli.dev/ns#> .\n");
    ttl.push_str("@prefix va:    <https://decision-cli.dev/ns/value-actions/> .\n");
    ttl.push_str("@prefix role:  <https://decision-cli.dev/ns/roles/> .\n");
    ttl.push_str("@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .\n\n");

    ttl.push_str(&format!("<{}> a dec:ValueStream ;\n", stream_iri));
    ttl.push_str(&format!("    dec:name                \"{}-development\" ;\n", repo_name));
    ttl.push_str(&format!("    dec:title               \"{} Development\" ;\n", repo_name));
    ttl.push_str("    dec:description         \"Auto-generated value stream for development.\" ;\n");
    ttl.push_str("    dec:terminalValueAction va:shipped-feature ;\n");
    ttl.push_str("    dec:authorizedGoals     \"ship\" , \"land\" .\n\n");

    // Role catalog (slice-1 defaults per FT-114 spec)
    ttl.push_str("# Role catalog (slice-1 defaults):\n");
    ttl.push_str("role:implementer a dec:Role ;\n");
    ttl.push_str("    rdfs:label \"Implementer\" ;\n");
    ttl.push_str("    dec:description \"Implements features from bundle to code.\" .\n\n");

    ttl.push_str("role:verifier a dec:Role ;\n");
    ttl.push_str("    rdfs:label \"Verifier\" ;\n");
    ttl.push_str("    dec:description \"Runs verification graphs and emits feedback.\" .\n\n");

    ttl.push_str("role:verify-graph-author a dec:Role ;\n");
    ttl.push_str("    rdfs:label \"Verify Graph Author\" ;\n");
    ttl.push_str("    dec:description \"Authors verification graphs from TCs.\" .\n\n");

    // Capability bindings (Scaleway default)
    ttl.push_str("# Capability bindings (Scaleway Claude endpoint):\n");
    ttl.push_str("# Requires SCW_SECRET_KEY environment variable.\n\n");

    if !runners.is_empty() {
        ttl.push_str("# Subscriptions discovered from .product/tests/ runner types:\n");
        let mut sorted_runners: Vec<_> = runners.iter().collect();
        sorted_runners.sort();
        for runner in sorted_runners {
            ttl.push_str(&format!("# - {}\n", runner));
        }
        ttl.push_str("\n");
    }

    ttl
}
