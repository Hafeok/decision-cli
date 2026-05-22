//! MCP / CLI surface adapters for `verify_graph_generate` (FT-049 / ADR-029).
//!
//! Both surfaces route through [`super::run_generate`] / [`super::run_accept`];
//! this module owns only the surface-specific machinery (parse, schema,
//! response rendering, tool descriptors) so [`super`]'s `mod.rs` stays
//! under the ADR-013 §Rule 1 limit.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};

use super::proposal::ProposalKind;
use super::{
    run_accept, run_generate, AcceptRequest, AcceptResponse, GenerateRequest, GenerateResponse,
    TOOL_NAME_ACCEPT, TOOL_NAME_GENERATE,
};

/// Parse a `Request` envelope into a [`GenerateRequest`].
pub fn parse_generate_request(req: &Request) -> Result<GenerateRequest, HandlerError> {
    let mut parsed: GenerateRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_graph_generate arguments: {e}"),
            }
        })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// Parse a `Request` envelope into an [`AcceptRequest`].
pub fn parse_accept_request(req: &Request) -> Result<AcceptRequest, HandlerError> {
    let mut parsed: AcceptRequest = serde_json::from_value(req.arguments.clone()).map_err(|e| {
        HandlerError::InvalidArgument {
            field: "arguments".to_string(),
            detail: format!("malformed dec_verify_graph_accept arguments: {e}"),
        }
    })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// JSON Schema describing the generate tool's input.
#[must_use]
pub fn generate_input_schema() -> Value {
    json!({
        "type": "object",
        "required": ["feature_id", "environment_id"],
        "additionalProperties": false,
        "properties": {
            "feature_id": { "type": "string", "minLength": 1 },
            "environment_id": { "type": "string", "minLength": 1 },
            "mode": { "type": "string", "enum": ["interactive", "accept", "print_only"] },
            "workdir": { "type": "string" },
            "product_root": { "type": "string" },
        },
    })
}

/// JSON Schema describing the accept tool's input.
#[must_use]
pub fn accept_input_schema() -> Value {
    json!({
        "type": "object",
        "required": ["proposal", "proposal_token", "feature_id", "environment_id"],
        "additionalProperties": false,
        "properties": {
            "proposal": { "type": "object" },
            "proposal_token": { "type": "string", "minLength": 1 },
            "feature_id": { "type": "string", "minLength": 1 },
            "environment_id": { "type": "string", "minLength": 1 },
            "workdir": { "type": "string" },
            "product_root": { "type": "string" },
        },
    })
}

/// Render a `Response` from a [`GenerateResponse`].
#[must_use]
pub fn response_for_generate(outcome: &GenerateResponse) -> Response {
    let summary = render_generate_summary(outcome);
    let data = serde_json::to_value(outcome).unwrap_or(Value::Null);
    Response::with_summary(data, summary)
}

/// Render a `Response` from an [`AcceptResponse`].
#[must_use]
pub fn response_for_accept(outcome: &AcceptResponse) -> Response {
    let summary = format!(
        "persisted graph {id} at {path}",
        id = outcome.persisted.graph_id,
        path = outcome.persisted.graph_path.display()
    );
    let data = serde_json::to_value(outcome).unwrap_or(Value::Null);
    Response::with_summary(data, summary)
}

/// MCP tool descriptor for `dec_verify_graph_generate`.
#[must_use]
pub fn generate_tool_descriptor() -> ToolDescriptor {
    let handler: ToolHandler = Arc::new(|req: Request| {
        let parsed = parse_generate_request(&req)?;
        let outcome = run_generate(&parsed)?;
        Ok(response_for_generate(&outcome))
    });
    ToolDescriptor::new(
        TOOL_NAME_GENERATE,
        "Propose a dec:VerificationGraph for a feature in a target environment (FT-049 / ADR-030).",
        generate_input_schema(),
        handler,
    )
}

/// MCP tool descriptor for `dec_verify_graph_accept`.
#[must_use]
pub fn accept_tool_descriptor() -> ToolDescriptor {
    let handler: ToolHandler = Arc::new(|req: Request| {
        let parsed = parse_accept_request(&req)?;
        let outcome = run_accept(&parsed)?;
        Ok(response_for_accept(&outcome))
    });
    ToolDescriptor::new(
        TOOL_NAME_ACCEPT,
        "Persist a GraphProposal previously returned by dec_verify_graph_generate (FT-049 / ADR-030).",
        accept_input_schema(),
        handler,
    )
}

fn render_generate_summary(outcome: &GenerateResponse) -> String {
    match outcome.proposal.kind {
        ProposalKind::Match => match &outcome.proposal.match_payload {
            Some(m) => format!(
                "{graph} already covers this feature in this environment; no new graph needed",
                graph = m.graph_id,
            ),
            None => "match proposal".to_string(),
        },
        ProposalKind::New => match &outcome.persisted {
            Some(p) => format!(
                "persisted graph {id} at {path}",
                id = p.graph_id,
                path = p.graph_path.display()
            ),
            None => "new proposal — review and re-run with --accept to persist".to_string(),
        },
        ProposalKind::Gap => "gap — worker cannot honestly produce a covering graph".to_string(),
    }
}
