//! MCP server scaffolding shared by every dec content-management feature (FT-034).
//!
//! Per ADR-029 the dec MCP surface and the dec CLI surface route every
//! tool through one handler. This module supplies the substrate:
//!
//! * [`ToolDescriptor`] — the carrier each feature registers.
//! * [`ToolRegistry`] — in-memory store enforcing naming + uniqueness.
//! * [`serve_stdio`] — entry point that runs the JSON-RPC 2.0 over
//!   stdio MCP transport against a populated registry.
//!
//! The module is transport substrate only — no tool implementations
//! live here. Feature modules construct their own `ToolDescriptor`s
//! and hand them to a registry the binary owns; `serve_stdio` drives
//! the registry against stdin/stdout.

mod naming;
mod protocol;
mod registry;
mod server;

pub use naming::{validate_tool_name, NamingError};
pub use registry::{RegisterError, ToolDescriptor, ToolHandler, ToolRegistry};
pub use server::{serve_stdio, serve_stdio_with_io, ServeError, ShutdownSignal};
