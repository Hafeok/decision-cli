//! `dec mcp serve` — MCP stdio server subcommand (FT-034 / ADR-029).
//!
//! Thin wrapper around [`crate::core::mcp::serve_stdio`]. The feature
//! exposes:
//!
//!   * [`build_registry`] — empty registry plus optional test fixtures.
//!   * [`serve_with_registry`] — run the stdio loop against a registry.
//!   * [`serve`] — convenience that calls both for callers (tests) that
//!     want the fixture-only surface.
//!
//! Per ADR-016, this module MUST NOT import from sibling feature
//! modules. Composition of production tool descriptors happens in the
//! binary (`crates/decision-cli/src/cli/mcp.rs`), which is allowed to
//! see every feature.

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
/// FT-034 itself registers no tools — production tool composition
/// lives in the binary (per ADR-016: `features::mcp` MUST NOT depend
/// on sibling feature modules). `DEC_MCP_TEST_FIXTURES=1` opts in to a
/// minimal fixture set used by the TC-051 / TC-053 bash harnesses to
/// verify the wire protocol without a real subcommand-feature handler.
pub fn build_registry(_workdir: &Path) -> Result<ToolRegistry, McpError> {
    let mut registry = ToolRegistry::new();
    if std::env::var_os("DEC_MCP_TEST_FIXTURES").is_some() {
        register_fixture_tools(&mut registry)?;
    }
    Ok(registry)
}

/// Programmatic entry point for `dec mcp serve` with a caller-supplied
/// registry. The binary uses this after assembling production tools
/// from each feature module; tests use this to drive the loop with
/// fixture-only registries.
///
/// Logs `mcp server ready` to stderr via tracing (TC-053 AC #1),
/// reads JSON-RPC frames until EOF (TC-053 AC #3), and writes
/// responses to stdout. Returns `Ok(())` on a clean shutdown.
pub fn serve_with_registry(_workdir: &Path, registry: ToolRegistry) -> Result<(), McpError> {
    info!(
        target: "dec_mcp",
        registered = registry.len(),
        "starting `dec mcp serve` over stdio"
    );
    serve_stdio(&registry)?;
    Ok(())
}

/// Convenience: build the fixture-only registry and serve it.
///
/// Production callers (the `dec` binary) should NOT use this directly —
/// they compose feature-module descriptors first, then call
/// [`serve_with_registry`]. This helper exists for tests / scripts that
/// only need the wire protocol with no real tools.
pub fn serve(workdir: &Path) -> Result<(), McpError> {
    let registry = build_registry(workdir)?;
    serve_with_registry(workdir, registry)
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

    // The two env-sensitive tests below share global process state
    // (the `DEC_MCP_TEST_FIXTURES` env var). Cargo runs them in
    // parallel so we serialise through a mutex.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn registry_is_empty_without_fixture_flag() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        with_fixture_env(None, || {
            let tmp = std::env::temp_dir();
            let reg = build_registry(&tmp).expect("build registry");
            // FT-034 (this module) ships no production tools.
            // Production tool composition happens in the binary.
            assert!(
                reg.is_empty(),
                "features::mcp must not register feature tools"
            );
        });
    }

    #[test]
    fn fixture_flag_registers_ping() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        with_fixture_env(Some(OsStr::new("1")), || {
            let tmp = std::env::temp_dir();
            let reg = build_registry(&tmp).expect("build registry");
            assert!(reg.get("dec_mcp_ping").is_some());
        });
    }
}
