//! MCP tool descriptor + JSON schemas for `dec verify env show`.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::core::handler::{Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};

use super::{parse_request, run, EnvShowRequest, TOOL_NAME};

/// MCP tool descriptor — registered by `cli::mcp`.
pub fn tool_descriptor() -> ToolDescriptor {
    ToolDescriptor::new(
        TOOL_NAME,
        "Show a single dec:VerificationEnvironment artifact (FT-040 / ADR-028).",
        input_schema(),
        tool_handler(),
    )
    .with_output_schema(output_schema())
}

/// MCP handler closure — runs the single handler and renders the response.
fn tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed: EnvShowRequest = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!(
            "showed env {id} from {path}",
            id = outcome.env.id,
            path = outcome.path.display()
        );
        Ok(Response::with_summary(
            json!({
                "env": outcome.env,
                "path": outcome.path,
            }),
            summary,
        ))
    })
}

fn output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["env", "path"],
        "properties": {
            "env": env_document_schema(),
            "path": { "type": "string" },
        },
    })
}

fn env_document_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "env_type", "safety_class", "allowed_ops"],
        "properties": {
            "id": { "type": "string" },
            "env_type": { "type": "string" },
            "safety_class": { "type": "string" },
            "endpoint": { "type": "string" },
            "allowed_ops": {
                "type": "array",
                "items": { "type": "string" },
            },
            "setup": { "type": "string" },
            "teardown": { "type": "string" },
        },
    })
}

/// JSON Schema describing the MCP tool's input arguments.
#[must_use]
pub fn input_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id"],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "format": { "type": "string", "enum": ["text", "json"] },
            "workdir": { "type": "string" },
        },
    })
}
