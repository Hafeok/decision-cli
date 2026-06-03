//! FT-136 / TC-330 — dec mcp registers product-mcp and dec tools without name collision
//!
//! Validates that FT-136's MCP merge (Phase 3) registers both product_mcp::ToolRegistry
//! tools and dec's own tools without name collision. This test exercises the same
//! invariant as TC-177, adapted for FT-136's context.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;

use decision_cli::core::handler::{Request, Response};
use decision_cli::core::mcp::{RegisterError, ToolDescriptor, ToolHandler, ToolRegistry};
use decision_cli::product_cmd;

fn noop_handler() -> ToolHandler {
    Arc::new(|_req: Request| Ok(Response::structured(json!({"ok": true}))))
}

fn descriptor(name: &str) -> ToolDescriptor {
    ToolDescriptor::new(name, "test", json!({"type": "object"}), noop_handler())
}

fn register_dec_set(registry: &mut ToolRegistry, names: &[&str]) {
    for name in names {
        registry
            .register(descriptor(name))
            .unwrap_or_else(|e| panic!("register dec tool {name}: {e}"));
    }
}

fn register_product_set(registry: &mut ToolRegistry) {
    for entry in product_cmd::mcp_tool_entries() {
        registry
            .register_external(descriptor(&entry.name))
            .unwrap_or_else(|e| panic!("register product tool {}: {e}", entry.name));
    }
}

#[test]
fn product_mcp_tools_are_not_empty() {
    // TC-330 prerequisite: product_cmd::mcp_tool_entries() returns non-empty list
    let product_entries = product_cmd::mcp_tool_entries();
    assert!(
        !product_entries.is_empty(),
        "product-cli MCP tool surface should not be empty after FT-136"
    );

    // Verify all tool names start with product_
    for entry in &product_entries {
        assert!(
            entry.name.starts_with("product_"),
            "Product tool name should start with product_, got: {}",
            entry.name
        );
    }
}

#[test]
fn dec_and_product_tools_have_no_name_collision() {
    // TC-330 AC: no two entries share the same name field
    let dec_names: Vec<&str> = vec![
        "dec_verify_bench_new",
        "dec_verify_bench_list",
        "dec_verify_bench_show",
        "dec_verify_graph_new",
        "dec_verify_graph_list",
        "dec_verify_graph_show",
        "dec_verify_graph_run",
        "dec_verify_graph_generate",
        "dec_verify_graph_accept",
        "dec_verify_step_add",
        "dec_verify_feature",
    ];

    let product_entries = product_cmd::mcp_tool_entries();
    let product_names: Vec<String> = product_entries.iter().map(|e| e.name.clone()).collect();

    let dec_set: BTreeSet<String> = dec_names.iter().map(|s| (*s).to_string()).collect();
    let product_set: BTreeSet<String> = product_names.iter().cloned().collect();
    let overlap: Vec<String> = dec_set.intersection(&product_set).cloned().collect();

    assert!(
        overlap.is_empty(),
        "dec_ / product_ name overlap detected: {overlap:?}"
    );
}

#[test]
fn combined_registry_holds_both_tool_sets() {
    // TC-330 AC: combined registry contains at least one dec_ and one product_ tool
    let dec_names: Vec<&str> = vec![
        "dec_verify_bench_new",
        "dec_verify_bench_list",
        "dec_verify_bench_show",
    ];

    let mut registry = ToolRegistry::new();
    register_dec_set(&mut registry, &dec_names);
    register_product_set(&mut registry);

    let registered: BTreeSet<String> = registry.iter().map(|t| t.name.clone()).collect();

    // Assert at least one dec_ tool is present
    let has_dec_tool = registered.iter().any(|name| name.starts_with("dec_"));
    assert!(
        has_dec_tool,
        "Combined registry should contain at least one dec_ tool"
    );

    // Assert at least one product_ tool is present
    let has_product_tool = registered.iter().any(|name| name.starts_with("product_"));
    assert!(
        has_product_tool,
        "Combined registry should contain at least one product_ tool"
    );

    // Verify total count is sum of both sets
    let product_entries = product_cmd::mcp_tool_entries();
    assert_eq!(
        registry.len(),
        dec_names.len() + product_entries.len(),
        "Combined registry size should equal |dec| + |product|"
    );
}

#[test]
fn duplicate_registration_is_rejected() {
    // TC-330 safety net: collision at registration time is rejected
    let mut registry = ToolRegistry::new();

    // Register a product tool first
    let product_entries = product_cmd::mcp_tool_entries();
    if let Some(first_tool) = product_entries.first() {
        registry
            .register_external(descriptor(&first_tool.name))
            .expect("first registration should succeed");

        // Attempt to register the same name again
        let err = registry
            .register_external(descriptor(&first_tool.name))
            .expect_err("duplicate registration must fail");

        match err {
            RegisterError::Duplicate(n) => assert_eq!(n, first_tool.name),
            other => panic!("expected RegisterError::Duplicate, got {other:?}"),
        }
    }
}
