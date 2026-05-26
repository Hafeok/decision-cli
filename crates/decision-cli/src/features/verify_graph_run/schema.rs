//! JSON Schema definitions for `dec_verify_graph_run` (FT-099).

use serde_json::{json, Value};

/// Input schema for the MCP tool.
#[must_use]
pub fn input() -> Value {
    json!({
        "type": "object",
        "required": ["graph_id"],
        "additionalProperties": false,
        "properties": {
            "graph_id": { "type": "string", "minLength": 1 },
            "capture_bindings": {
                "type": "object",
                "additionalProperties": { "type": "string" },
            },
            "no_feedback": { "type": "boolean" },
            "keep_tmp": { "type": "boolean" },
            "workdir": { "type": "string" },
        },
    })
}

/// Output schema for the MCP tool.
#[must_use]
pub fn output() -> Value {
    json!({
        "type": "object",
        "required": ["result_id", "graph_id", "verdict", "step_outcomes"],
        "properties": {
            "session_id": { "type": ["string", "null"] },
            "result_id": { "type": "string" },
            "graph_id": { "type": "string" },
            "environment_id": { "type": "string" },
            "verdict": { "type": "string" },
            "rationale": { "type": "string" },
            "step_outcomes": { "type": "array" },
            "emitted_feedback": { "type": "array", "items": { "type": "string" } },
        },
    })
}
