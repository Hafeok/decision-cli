//! In-memory tool registry for the dec MCP server (FT-034 / ADR-029).
//!
//! Feature modules build [`ToolDescriptor`] values and hand them to a
//! [`ToolRegistry`]. The registry enforces ADR-029's `dec_<noun>_<verb>`
//! naming rule and rejects duplicate names; startup fails fast when
//! either invariant is violated. The registry is *not* a process
//! global — main.rs constructs it, optionally seeds it, and hands it
//! to [`super::serve_stdio`] explicitly. This keeps the
//! slice-level SDP boundary (`core/` does not depend on `features/*`)
//! intact: features hand `ToolDescriptor`s up to the binary, which
//! threads them into the registry.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use super::naming::{validate_tool_name, NamingError};
use crate::core::handler::{Error as HandlerError, Request, Response};

/// Function type implementing a single tool. Both CLI and MCP surfaces
/// invoke a `ToolHandler` against a [`Request`] and bubble the result
/// back through their respective renderers. Wrapped in [`Arc`] so the
/// registry can hand out cheap clones to async tasks.
pub type ToolHandler = Arc<dyn Fn(Request) -> Result<Response, HandlerError> + Send + Sync>;

/// Single MCP tool record. Fields mirror the slice-1 `tools/list`
/// schema (per the MCP spec) plus a [`ToolHandler`] reference used by
/// the routing layer in [`super::server`].
#[derive(Clone)]
pub struct ToolDescriptor {
    /// Tool name, e.g. `dec_verify_env_new`. Must satisfy
    /// [`validate_tool_name`]; the registry refuses to accept a
    /// descriptor whose name is malformed.
    pub name: String,
    /// One-line description rendered on `tools/list`.
    pub description: String,
    /// JSON Schema describing the tool's expected arguments. Required
    /// by the MCP wire protocol; pass `serde_json::json!({})` for a
    /// schemaless tool.
    pub input_schema: Value,
    /// Optional JSON Schema describing the tool's structured output.
    pub output_schema: Option<Value>,
    /// The handler routed to by both surfaces.
    pub handler: ToolHandler,
}

impl std::fmt::Debug for ToolDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolDescriptor")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("output_schema", &self.output_schema)
            .field("handler", &"<fn>")
            .finish()
    }
}

impl ToolDescriptor {
    /// Construct a descriptor. Performs no validation — the registry
    /// validates at insertion time so callers can build descriptors
    /// freely (e.g. in tests) without paying the naming check up-front.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: ToolHandler,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            output_schema: None,
            handler,
        }
    }

    /// Attach an output schema for tools that publish a structured
    /// result shape on `tools/list`.
    #[must_use]
    pub fn with_output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

/// Errors raised by [`ToolRegistry::register`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RegisterError {
    /// The descriptor's name failed ADR-029 validation.
    #[error("tool naming rejected: {0}")]
    Naming(#[from] NamingError),

    /// A descriptor with this name is already registered. FT-034
    /// §Error handling: duplicate registration is a startup failure.
    #[error("duplicate tool name '{0}' — already registered")]
    Duplicate(String),
}

/// In-memory registry of MCP tools.
///
/// Wrapping a [`BTreeMap`] gives us deterministic iteration order
/// (lexicographic by tool name), which keeps `tools/list` stable
/// across invocations — a property test harnesses rely on when
/// asserting registry conformance.
#[derive(Clone, Default, Debug)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolDescriptor>,
}

impl ToolRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a descriptor. Validates the name first, then enforces
    /// uniqueness. Returns the inserted name on success so callers can
    /// log it; returns [`RegisterError`] when either check fails.
    pub fn register(&mut self, descriptor: ToolDescriptor) -> Result<String, RegisterError> {
        validate_tool_name(&descriptor.name)?;
        if self.tools.contains_key(&descriptor.name) {
            return Err(RegisterError::Duplicate(descriptor.name));
        }
        let name = descriptor.name.clone();
        self.tools.insert(name.clone(), descriptor);
        Ok(name)
    }

    /// Insert a descriptor from an absorbed external namespace (FT-105 /
    /// ADR-067).
    ///
    /// Skips the `dec_*` naming gate so the combined registry can host
    /// the absorbed product-cli `product_*` tool set without
    /// renaming. Duplicate-name detection still runs, so a collision
    /// between any dec_ tool and any product_ tool is rejected at
    /// startup — TC-177's "no collision" invariant.
    pub fn register_external(
        &mut self,
        descriptor: ToolDescriptor,
    ) -> Result<String, RegisterError> {
        if descriptor.name.is_empty() {
            return Err(RegisterError::Naming(NamingError::Empty));
        }
        if self.tools.contains_key(&descriptor.name) {
            return Err(RegisterError::Duplicate(descriptor.name));
        }
        let name = descriptor.name.clone();
        self.tools.insert(name.clone(), descriptor);
        Ok(name)
    }

    /// Look up a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ToolDescriptor> {
        self.tools.get(name)
    }

    /// Iterate tools in lexicographic order by name.
    pub fn iter(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.tools.values()
    }

    /// Number of registered tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// True iff no tools are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Borrow the underlying `BTreeMap` (testing aid).
    #[must_use]
    #[doc(hidden)]
    pub fn as_map(&self) -> &BTreeMap<String, ToolDescriptor> {
        &self.tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn noop_handler() -> ToolHandler {
        Arc::new(|_req: Request| Ok(Response::structured(json!({"ok": true}))))
    }

    fn descriptor(name: &str) -> ToolDescriptor {
        ToolDescriptor::new(name, "test", json!({"type": "object"}), noop_handler())
    }

    #[test]
    fn registers_valid_tool() {
        let mut reg = ToolRegistry::new();
        let inserted = reg.register(descriptor("dec_mcp_ping")).expect("ok");
        assert_eq!(inserted, "dec_mcp_ping");
        assert!(reg.get("dec_mcp_ping").is_some());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn rejects_malformed_name() {
        let mut reg = ToolRegistry::new();
        // TC-051 AC #1: a name with a space is rejected.
        let err = reg.register(descriptor("bad name")).unwrap_err();
        assert!(matches!(
            err,
            RegisterError::Naming(NamingError::MissingPrefix(_))
        ));
        // TC-051 AC #1: a name missing the `dec_` prefix is rejected.
        let err = reg.register(descriptor("verify_env_new")).unwrap_err();
        assert!(matches!(
            err,
            RegisterError::Naming(NamingError::MissingPrefix(_))
        ));
        assert!(reg.is_empty());
    }

    #[test]
    fn rejects_duplicate_name() {
        // TC-051 AC #3: registering the same name twice is rejected.
        let mut reg = ToolRegistry::new();
        reg.register(descriptor("dec_mcp_ping")).expect("first ok");
        let err = reg.register(descriptor("dec_mcp_ping")).unwrap_err();
        match err {
            RegisterError::Duplicate(n) => assert_eq!(n, "dec_mcp_ping"),
            other => panic!("expected Duplicate, got {other:?}"),
        }
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn iteration_order_is_lexicographic() {
        let mut reg = ToolRegistry::new();
        reg.register(descriptor("dec_zebra_zip")).expect("ok");
        reg.register(descriptor("dec_alpha_act")).expect("ok");
        reg.register(descriptor("dec_mango_make")).expect("ok");
        let names: Vec<&str> = reg.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["dec_alpha_act", "dec_mango_make", "dec_zebra_zip"]
        );
    }
}
