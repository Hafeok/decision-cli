//! `dec mcp serve` — MCP stdio server subcommand (FT-034 / ADR-029).
//!
//! Thin wrapper around [`crate::core::mcp::serve_stdio`]. The feature
//! constructs a [`ToolRegistry`], optionally seeds it with test
//! fixtures (only when the `DEC_MCP_TEST_FIXTURES` env var is set),
//! and hands it to the stdio loop. No CLI-side business logic lives
//! here; per ADR-029 the single-handler discipline means every tool
//! routes through its own handler in the relevant feature module.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use thiserror::Error;
use tracing::info;

use crate::core::handler::{Request, Response};
use crate::core::mcp::{
    serve_stdio, RegisterError, ServeError, ToolDescriptor, ToolHandler, ToolRegistry,
};

/// Errors surfaced by [`serve`].
#[derive(Debug, Error)]
pub enum McpError {
    /// Registry rejected one of the supplied descriptors.
    #[error("tool registration failed: {0}")]
    Register(#[from] RegisterError),

    /// Stdio loop failed (IO error).
    #[error("mcp server failure: {0}")]
    Serve(#[from] ServeError),
}

/// Build the registry for `dec mcp serve`.
///
/// FT-034 itself registers no tools (it is pure substrate). The
/// `DEC_MCP_TEST_FIXTURES` env var opts in to a minimal fixture set
/// used by the TC-051 / TC-053 bash harnesses to verify the wire
/// protocol without needing a real subcommand-feature handler.
pub fn build_registry(_workdir: &Path) -> Result<ToolRegistry, McpError> {
    let mut registry = ToolRegistry::new();
    if std::env::var_os("DEC_MCP_TEST_FIXTURES").is_some() {
        register_fixture_tools(&mut registry)?;
    }
    Ok(registry)
}

/// Programmatic entry point for the `dec mcp serve` subcommand.
///
/// Logs `mcp server ready` to stderr via tracing (TC-053 AC #1),
/// reads JSON-RPC frames until EOF (TC-053 AC #3), and writes
/// responses to stdout. Returns `Ok(())` on a clean shutdown.
pub fn serve(workdir: &Path) -> Result<(), McpError> {
    let registry = build_registry(workdir)?;
    info!(
        target: "dec_mcp",
        registered = registry.len(),
        "starting `dec mcp serve` over stdio"
    );
    serve_stdio(&registry)?;
    Ok(())
}

/// Test-fixture tools registered only when `DEC_MCP_TEST_FIXTURES=1`.
///
/// The fixture suite is intentionally tiny: it exists so TC-053 can
/// exercise the wire protocol end-to-end (initialize → tools/list →
/// tools/call) without depending on a real subcommand-feature handler.
/// Every fixture name still satisfies the ADR-029 naming rule, so the
/// fixture set doubles as a regression check for the live registry
/// conformance assertion in TC-051 AC #2.
fn register_fixture_tools(registry: &mut ToolRegistry) -> Result<(), RegisterError> {
    let ping_handler: ToolHandler = Arc::new(|req: Request| {
        Ok(Response::with_summary(
            json!({
                "echo": req.arguments,
                "tool": req.tool,
            }),
            "pong",
        ))
    });
    registry.register(ToolDescriptor::new(
        "dec_mcp_ping",
        "Test fixture: echo arguments back (DEC_MCP_TEST_FIXTURES only)",
        json!({
            "type": "object",
            "additionalProperties": true,
        }),
        ping_handler,
    ))?;
    // TC-051 AC #3: a second registration with the same name must
    // cause `dec mcp serve` to fail at startup. The `DEC_MCP_TEST_DUPLICATE`
    // env var seeds that collision deterministically so the bash test
    // harness can assert the exit code and diagnostic without needing
    // a second real feature module.
    if std::env::var_os("DEC_MCP_TEST_DUPLICATE").is_some() {
        let echo_handler: ToolHandler =
            Arc::new(|_req: Request| Ok(Response::structured(json!({"ok": true}))));
        registry.register(ToolDescriptor::new(
            "dec_mcp_ping",
            "Test fixture: duplicate-collision probe",
            json!({"type": "object"}),
            echo_handler,
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn with_fixture_env<F: FnOnce()>(value: Option<&OsStr>, f: F) {
        let key = "DEC_MCP_TEST_FIXTURES";
        let prior = std::env::var_os(key);
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        f();
        match prior {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn registry_is_empty_without_fixture_flag() {
        with_fixture_env(None, || {
            let tmp = std::env::temp_dir();
            let reg = build_registry(&tmp).expect("build registry");
            assert!(reg.is_empty(), "FT-034 ships no production tools");
        });
    }

    #[test]
    fn fixture_flag_registers_ping() {
        with_fixture_env(Some(OsStr::new("1")), || {
            let tmp = std::env::temp_dir();
            let reg = build_registry(&tmp).expect("build registry");
            assert!(reg.get("dec_mcp_ping").is_some());
        });
    }
}
