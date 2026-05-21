//! `dec verify graph show` — single-handler implementation (FT-043 / ADR-029).
//!
//! Read-only detail view of a single `dec:VerificationGraph`. One handler,
//! two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct a [`GraphShowRequest`] and
//! route it through [`run`].
//!
//! Behaviour mirrors FT-043 §Behaviour: validate the id, locate the
//! on-disk graph file under `.dec/verify/graph/<id>.ttl`, reconstruct
//! the full `VerificationGraph` (header + ordered steps), and return
//! the projected [`GraphDocument`] alongside the resolved path.

mod document;
mod render;
mod resolve;
mod roundtrip;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};

pub use document::{GraphDocument, StepDocument};
pub use render::{render_json, render_text};
pub use roundtrip::{document_to_graph, ReconstructError};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_graph_show";

/// Output format selector for the CLI surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable multi-line render (default).
    Text,
    /// Machine-readable JSON object.
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl OutputFormat {
    /// Parse the wire value. Returns `None` for unknown strings.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Structured request the single handler consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraphShowRequest {
    /// Identifier of the graph to show (e.g. `VG-001` or `VG-001-foo`).
    pub id: String,
    /// Output format. Defaults to `Text` for CLI rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Working directory the handler reads against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Structured response — surfaced verbatim by MCP (as `{ graph, path }`),
/// rendered as text or JSON by the CLI per `req.format`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphShowResponse {
    /// Full graph document.
    pub graph: GraphDocument,
    /// Absolute path of the on-disk `.dec/verify/graph/<id>.ttl` file.
    pub path: PathBuf,
    /// Safety class of the referenced environment, if it could be loaded
    /// from `.dec/verify/env/`. Used by the CLI text renderer; not part
    /// of the JSON / MCP graph document per FT-043 §Outputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_safety: Option<String>,
}

/// Parse the structured `Request` envelope into [`GraphShowRequest`].
pub fn parse_request(req: &Request) -> Result<GraphShowRequest, HandlerError> {
    let mut parsed: GraphShowRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_graph_show arguments: {e}"),
            }
        })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// MCP tool descriptor — registered by the binary in `cli::mcp`.
#[must_use]
pub fn tool_descriptor() -> ToolDescriptor {
    let handler: ToolHandler = Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!(
            "showed graph {id} from {path}",
            id = outcome.graph.id,
            path = outcome.path.display()
        );
        Ok(Response::with_summary(
            json!({
                "graph": outcome.graph,
                "path": outcome.path,
            }),
            summary,
        ))
    });
    ToolDescriptor::new(
        TOOL_NAME,
        "Show a single dec:VerificationGraph artifact (FT-043 / ADR-028).",
        input_schema(),
        handler,
    )
    .with_output_schema(json!({
        "type": "object",
        "required": ["graph", "path"],
        "properties": {
            "graph": {
                "type": "object",
                "required": ["id", "verifies", "environment", "steps"],
                "properties": {
                    "id": { "type": "string" },
                    "verifies": { "type": "string" },
                    "environment": { "type": "string" },
                    "steps": {
                        "type": "array",
                        "items": { "type": "object" },
                    },
                },
            },
            "path": { "type": "string" },
        },
    }))
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

/// Validate the id format ahead of any I/O. FT-043 §Error handling:
/// malformed ids surface as `InvalidArgument { field: "id" }` (exit 2),
/// distinct from missing ids which surface as `ArtifactNotFound` (exit 1).
fn validate_id(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("VG-") {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id must start with 'VG-', got {id:?}"),
        });
    }
    if id.len() < 4 {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id {id:?} is too short (expected VG-NNN[-suffix])"),
        });
    }
    let tail = &id[3..];
    if !tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id {id:?} must match VG-NNN[-suffix]"),
        });
    }
    Ok(())
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &GraphShowRequest) -> Result<GraphShowResponse, HandlerError> {
    validate_id(&req.id)?;
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    let graph_dir = workdir.join(".dec").join("verify").join("graph");
    let (path, graph) = resolve::load_graph(&graph_dir, &req.id)?;
    let document = GraphDocument::from_graph(&graph);
    let env_dir = workdir.join(".dec").join("verify").join("env");
    let environment_safety = resolve::load_environment_safety(&env_dir, &document.environment);
    Ok(GraphShowResponse {
        graph: document,
        path: path.canonicalize().unwrap_or(path),
        environment_safety,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_descriptor_has_canonical_name() {
        assert_eq!(tool_descriptor().name, TOOL_NAME);
    }

    #[test]
    fn output_format_parse_roundtrip() {
        assert_eq!(OutputFormat::parse("text"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert!(OutputFormat::parse("yaml").is_none());
    }

    #[test]
    fn request_roundtrips_through_json() {
        let req = GraphShowRequest {
            id: "VG-001".to_string(),
            format: Some(OutputFormat::Json),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: GraphShowRequest = serde_json::from_value(v).expect("de");
        assert_eq!(req, back);
    }

    #[test]
    fn input_schema_advertises_required_id() {
        let s = input_schema();
        let required = s
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");
        let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(names, vec!["id"]);
    }

    #[test]
    fn validate_id_rejects_missing_prefix() {
        let err = validate_id("not-an-id").expect_err("missing prefix must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_rejects_short_id() {
        let err = validate_id("VG-").expect_err("short id must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_rejects_non_numeric_tail() {
        let err = validate_id("VG-foo").expect_err("non-numeric tail must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_accepts_plain_vg_nnn() {
        validate_id("VG-007").expect("VG-007 should be a valid id format");
    }

    #[test]
    fn validate_id_accepts_vg_nnn_with_suffix() {
        validate_id("VG-001-foo").expect("suffixed id should be valid");
    }
}
