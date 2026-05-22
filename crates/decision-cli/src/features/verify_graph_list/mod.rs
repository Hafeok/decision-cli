//! `dec verify graph list` — single-handler implementation (FT-042 / ADR-029).
//!
//! Read-only listing of `dec:VerificationGraph` artifacts. One handler,
//! two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct a [`GraphListRequest`] and
//! route it through [`run`].
//!
//! Behaviour mirrors FT-042 §Behaviour: query the verify-graph named
//! graph via SPARQL, apply optional `verifies` / `environment` filter
//! predicates, compute `step_count` server-side, return graph summaries
//! in ascending `VG-NNN` order.

mod query;
mod render;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};

pub use render::{render_json, render_table};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_graph_list";

/// Output format selector for the CLI surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable table (default).
    Table,
    /// Machine-readable JSON array.
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Table
    }
}

impl OutputFormat {
    /// Parse the wire value. Returns `None` for unknown strings.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "table" => Some(Self::Table),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Structured request the single handler consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraphListRequest {
    /// Optional `verifies` filter — `FT-NNN` or `TC-NNN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifies: Option<String>,
    /// Optional environment filter (e.g. `ENV-001-ephemeral-cli`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Output format. Defaults to `Table` for CLI rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Working directory the handler reads against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// One row of the listing — matches the spec's `GraphSummary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSummary {
    /// `VG-NNN[-suffix]` identifier.
    pub id: String,
    /// `dec:verifies` reference (canonical `FT-NNN` or `TC-NNN`).
    pub verifies: String,
    /// `dec:environment` reference (canonical `ENV-NNN[-suffix]`).
    pub environment: String,
    /// Count of `dec:VerificationStep`s the graph contains.
    pub step_count: u64,
}

/// Structured response — surfaced verbatim by MCP, rendered as text or
/// JSON by the CLI per `req.format`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphListResponse {
    /// Ordered list of graph summaries (ascending `VG-NNN` numeric tail).
    pub graphs: Vec<GraphSummary>,
}

/// Parse the structured `Request` envelope into [`GraphListRequest`].
pub fn parse_request(req: &Request) -> Result<GraphListRequest, HandlerError> {
    let mut parsed: GraphListRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_graph_list arguments: {e}"),
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
    ToolDescriptor::new(
        TOOL_NAME,
        "List dec:VerificationGraph artifacts (FT-042 / ADR-028).",
        input_schema(),
        build_tool_handler(),
    )
    .with_output_schema(output_schema())
}

fn build_tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!("listed {n} verification graph(s)", n = outcome.graphs.len());
        Ok(Response::with_summary(
            json!({
                "graphs": outcome.graphs,
            }),
            summary,
        ))
    })
}

fn output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["graphs"],
        "properties": {
            "graphs": {
                "type": "array",
                "items": graph_summary_schema(),
            },
        },
    })
}

fn graph_summary_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "verifies", "environment", "step_count"],
        "properties": {
            "id": { "type": "string" },
            "verifies": { "type": "string" },
            "environment": { "type": "string" },
            "step_count": { "type": "integer", "minimum": 0 },
        },
    })
}

/// JSON Schema describing the MCP tool's input arguments.
#[must_use]
pub fn input_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "verifies": { "type": "string", "minLength": 1 },
            "environment": { "type": "string", "minLength": 1 },
            "format": { "type": "string", "enum": ["table", "json"] },
            "workdir": { "type": "string" },
        },
    })
}

/// Validate filter inputs ahead of the SPARQL query so unknown filter
/// values surface as `InvalidArgument` (exit 2 on the CLI).
fn validate(req: &GraphListRequest) -> Result<(), HandlerError> {
    if let Some(v) = &req.verifies {
        validate_verifies_filter(v.trim())?;
    }
    if let Some(e) = &req.environment {
        validate_environment_filter(e.trim())?;
    }
    Ok(())
}

fn validate_verifies_filter(trimmed: &str) -> Result<(), HandlerError> {
    if trimmed.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "verifies".to_string(),
            detail: "verifies filter must be a non-empty string".to_string(),
        });
    }
    if !trimmed.starts_with("FT-") && !trimmed.starts_with("TC-") {
        return Err(HandlerError::InvalidArgument {
            field: "verifies".to_string(),
            detail: format!("verifies filter must start with FT- or TC-; got {trimmed:?}"),
        });
    }
    Ok(())
}

fn validate_environment_filter(trimmed: &str) -> Result<(), HandlerError> {
    if trimmed.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "environment".to_string(),
            detail: "environment filter must be a non-empty string".to_string(),
        });
    }
    if !trimmed.starts_with("ENV-") {
        return Err(HandlerError::InvalidArgument {
            field: "environment".to_string(),
            detail: format!("environment filter must start with ENV-; got {trimmed:?}"),
        });
    }
    Ok(())
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &GraphListRequest) -> Result<GraphListResponse, HandlerError> {
    validate(req)?;
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    let mut graphs =
        query::query_graphs(workdir, req.verifies.as_deref(), req.environment.as_deref())?;
    graphs.sort_by(|a, b| query::graph_sort_key(&a.id).cmp(&query::graph_sort_key(&b.id)));
    Ok(GraphListResponse { graphs })
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
        assert_eq!(OutputFormat::parse("table"), Some(OutputFormat::Table));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert!(OutputFormat::parse("yaml").is_none());
    }

    #[test]
    fn request_roundtrips_through_json() {
        let req = GraphListRequest {
            verifies: Some("FT-001".to_string()),
            environment: Some("ENV-001-ephemeral-cli".to_string()),
            format: Some(OutputFormat::Json),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: GraphListRequest = serde_json::from_value(v).expect("de");
        assert_eq!(req, back);
    }

    #[test]
    fn validate_rejects_malformed_verifies() {
        let req = GraphListRequest {
            verifies: Some("garbage".to_string()),
            ..Default::default()
        };
        let err = validate(&req).expect_err("garbage must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "verifies"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_malformed_environment() {
        let req = GraphListRequest {
            environment: Some("nope".to_string()),
            ..Default::default()
        };
        let err = validate(&req).expect_err("nope must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "environment"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_accepts_empty_filters() {
        let req = GraphListRequest::default();
        validate(&req).expect("no filters should pass validation");
    }

    #[test]
    fn validate_accepts_well_formed_filters() {
        let req = GraphListRequest {
            verifies: Some("FT-001".to_string()),
            environment: Some("ENV-001-ephemeral-cli".to_string()),
            ..Default::default()
        };
        validate(&req).expect("valid filters should pass");
        let req2 = GraphListRequest {
            verifies: Some("TC-013".to_string()),
            ..Default::default()
        };
        validate(&req2).expect("TC- prefix should pass");
    }

    #[test]
    fn input_schema_advertises_optional_filters() {
        let s = input_schema();
        let props = s
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("props");
        assert!(props.contains_key("verifies"));
        assert!(props.contains_key("environment"));
        assert!(props.contains_key("format"));
    }
}
