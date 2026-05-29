//! TC-177 — combined `dec mcp` server exposes both tool sets without collision.
//!
//! Validates: FT-105 · ADR-067 · FT-034.
//! Spec: `.product/tests/TC-177-combined-dec-mcp-server-exposes-both-product-cli-a.md`
//!
//! The combined registry holds both surfaces:
//!
//!   * dec_* tools, registered through the dec_-prefixed naming gate.
//!   * product_* tools, registered through `register_external` since
//!     they come from an absorbed crate with its own naming convention.
//!
//! The test exercises three structural invariants:
//!
//!   1. Both tool sets are visible after a single `register_external`
//!      pass per product-cli MCP tool.
//!   2. No name collision between any dec_ tool and any product_ tool.
//!   3. Collision is rejected at registration time — the safety net
//!      FT-105 §Invariants names.

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
fn tc_177_combined_dec_mcp_server_exposes_both_product_cli_a() {
    // Scenario B preamble: the two sets do not overlap by construction.
    // We assert this *before* registration so the test surfaces a bad
    // namespace overlap at the source level even if registration
    // somehow accepted both.
    let dec_names: Vec<&str> = vec![
        "dec_verify_bench_new",
        "dec_verify_bench_list",
        "dec_verify_bench_show",
        "dec_verify_graph_new",
        "dec_verify_graph_list",
        "dec_verify_graph_show",
        "dec_verify_step_add",
    ];
    let product_entries = product_cmd::mcp_tool_entries();
    assert!(
        !product_entries.is_empty(),
        "product-cli MCP tool surface is empty — absorption regressed"
    );
    let product_names: Vec<String> = product_entries.iter().map(|e| e.name.clone()).collect();

    let dec_set: BTreeSet<String> = dec_names.iter().map(|s| (*s).to_string()).collect();
    let product_set: BTreeSet<String> = product_names.iter().cloned().collect();
    let overlap: Vec<String> = dec_set.intersection(&product_set).cloned().collect();
    assert!(
        overlap.is_empty(),
        "dec_ / product_ name overlap: {overlap:?}"
    );

    // Scenario A: combined registry holds the union.
    let mut registry = ToolRegistry::new();
    register_dec_set(&mut registry, &dec_names);
    register_product_set(&mut registry);

    let registered: BTreeSet<String> = registry.iter().map(|t| t.name.clone()).collect();
    for name in &dec_names {
        assert!(
            registered.contains(*name),
            "missing dec tool from combined registry: {name}"
        );
    }
    for name in &product_names {
        assert!(
            registered.contains(name),
            "missing product tool from combined registry: {name}"
        );
    }
    assert_eq!(
        registry.len(),
        dec_names.len() + product_names.len(),
        "combined registry size differs from |dec| + |product|"
    );

    // Scenario B safety net: a collision is rejected.
    let collide_name = "product_feature_show";
    let err = registry
        .register_external(descriptor(collide_name))
        .expect_err("duplicate registration must fail");
    match err {
        RegisterError::Duplicate(n) => assert_eq!(n, collide_name),
        other => panic!("expected RegisterError::Duplicate, got {other:?}"),
    }
}
