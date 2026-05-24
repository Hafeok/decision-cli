//! Per-kind step vocabulary + JSON Schemas for the verify-graph-author bundle.
//!
//! Extracted from `bundle.rs` to honour ADR-013 file-length limits. The
//! six FT-036 seed step kinds each carry a `fields_schema` here so the
//! Python worker can apply per-kind schema enforcement (closes the
//! dogfood gap where Qwen3-Coder had to guess at `fields.command` etc.
//! because the bundle previously sent `fields_schema: {}` for every kind).
//!
//! Field keys are kebab-case to match `core::verify::safety::known_keys`
//! and `features/verify_step_add/fields.rs::build_*` which both parse
//! kebab-case.

use crate::core::vocab::{
    STEP_KIND_CAPTURE, STEP_KIND_FILE_ASSERTION, STEP_KIND_HTTP_REQUEST, STEP_KIND_SHELL_COMMAND,
    STEP_KIND_SPARQL_ASSERTION, STEP_KIND_WAIT_FOR,
};

use super::bundle::StepKindRecord;

/// Assemble the six seed step kinds with their `required_ops` and
/// per-kind JSON Schemas. Order is deterministic for bundle-hash
/// stability.
pub(super) fn default_step_vocabulary() -> Vec<StepKindRecord> {
    let mut vocab = assertion_step_kinds();
    vocab.extend(control_step_kinds());
    vocab
}

fn assertion_step_kinds() -> Vec<StepKindRecord> {
    vec![
        step_kind_record(
            STEP_KIND_SHELL_COMMAND,
            &["shell", "filesystem"],
            "Run a shell command; assert exit code. Use to invoke `dec`, \
             `cargo`, `pytest`, or any binary on PATH.",
            shell_command_fields_schema(),
        ),
        step_kind_record(
            STEP_KIND_SPARQL_ASSERTION,
            &["sparql-local"],
            "Run a SPARQL SELECT against a local store; assert row count.",
            sparql_assertion_fields_schema(),
        ),
        step_kind_record(
            STEP_KIND_FILE_ASSERTION,
            &["filesystem"],
            "Assert file existence or content (hash / literal substring).",
            file_assertion_fields_schema(),
        ),
        step_kind_record(
            STEP_KIND_HTTP_REQUEST,
            &["http"],
            "Make an HTTP request; assert status code.",
            http_request_fields_schema(),
        ),
    ]
}

fn control_step_kinds() -> Vec<StepKindRecord> {
    vec![
        step_kind_record(
            STEP_KIND_WAIT_FOR,
            &[],
            "Poll a sub-condition with timeout. `condition` is an IRI \
             referencing another step or vocabulary term.",
            wait_for_fields_schema(),
        ),
        step_kind_record(
            STEP_KIND_CAPTURE,
            &[],
            "Bind a prior step's stdout/result to a name for later steps.",
            capture_fields_schema(),
        ),
    ]
}

fn step_kind_record(
    kind: &str,
    required_ops: &[&str],
    description: &str,
    fields_schema: serde_json::Value,
) -> StepKindRecord {
    StepKindRecord {
        kind: kind.to_string(),
        required_ops: required_ops.iter().map(|s| (*s).to_string()).collect(),
        fields_schema,
        description: description.to_string(),
    }
}

fn shell_command_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["command"],
        "additionalProperties": false,
        "properties": {
            "command": {
                "type": "string",
                "description": "Shell command line (e.g. `dec doctor --format json`). Required."
            },
            "expect-exit-code": {
                "type": "integer",
                "description": "Asserted exit code (default 0)."
            },
            "capture-output": {
                "type": "boolean",
                "description": "If true, stdout is captured for later `capture` steps."
            }
        }
    })
}

fn sparql_assertion_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["target", "query"],
        "additionalProperties": false,
        "properties": {
            "target": {
                "type": "string",
                "description": "Store path relative to the workdir (e.g. `.dec/store`). Required."
            },
            "query": {
                "type": "string",
                "description": "Full SPARQL query text including PREFIX clauses. Required."
            },
            "expect-rows": {
                "type": "integer",
                "description": "Expected row count (omit to skip the assertion)."
            }
        }
    })
}

fn file_assertion_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["path"],
        "additionalProperties": false,
        "properties": {
            "path": {
                "type": "string",
                "description": "Filesystem path relative to the workdir. Required."
            },
            "expect-hash": {
                "type": "string",
                "description": "Optional SHA-256 hex of the file contents."
            },
            "expect-content": {
                "type": "string",
                "description": "Optional literal substring that must appear in the file."
            }
        }
    })
}

fn http_request_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["method", "url"],
        "additionalProperties": false,
        "properties": {
            "method": {
                "type": "string",
                "description": "HTTP verb (e.g. `GET`, `POST`). Required."
            },
            "url": {
                "type": "string",
                "description": "Full request URL. Required."
            },
            "expect-status": {
                "type": "integer",
                "description": "Asserted HTTP status code (default 200)."
            }
        }
    })
}

fn wait_for_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["condition", "timeout"],
        "additionalProperties": false,
        "properties": {
            "condition": {
                "type": "string",
                "description": "IRI referencing another step or vocabulary term. Required."
            },
            "timeout": {
                "type": "string",
                "description": "Timeout literal (e.g. `30s`, `2m`). Required."
            }
        }
    })
}

fn capture_fields_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["bind-as"],
        "additionalProperties": false,
        "properties": {
            "bind-as": {
                "type": "string",
                "description": "Variable name to bind the prior step's output to. Required."
            },
            "from-step": {
                "type": "string",
                "description": "Optional IRI of the source step (defaults to the immediately preceding step)."
            }
        }
    })
}
