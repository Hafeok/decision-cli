//! JSON Schema definitions for `dec_verify_feature` (FT-099).

use serde_json::{json, Value};

/// Input schema for the MCP tool.
#[must_use]
pub fn input() -> Value {
    json!({
        "type": "object",
        "required": ["feature_id"],
        "additionalProperties": false,
        "properties": {
            "feature_id": { "type": "string", "minLength": 1 },
            "environment_id": { "type": "string" },
            "no_feedback": { "type": "boolean" },
            "include_stale": { "type": "boolean" },
            "dry_run": { "type": "boolean" },
            "workdir": { "type": "string" },
        },
    })
}

/// Output schema for the MCP tool.
#[must_use]
pub fn output() -> Value {
    json!({
        "type": "object",
        "required": ["feature_id", "per_graph", "per_tc", "coverage_gaps"],
        "properties": {
            "session_id": { "type": ["string", "null"] },
            "feature_id": { "type": "string" },
            "per_graph": { "type": "array" },
            "per_tc": { "type": "array" },
            "coverage_gaps": { "type": "array" },
            "aggregate": { "type": ["object", "null"] },
            "dry_run": { "type": "boolean" },
            "enumeration": { "type": ["object", "null"] },
        },
    })
}
