//! `dec verify env list` — single-handler implementation (FT-039 / ADR-029).
//!
//! Read-only listing of `dec:VerificationEnvironment` artifacts. One
//! handler, two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct an [`EnvListRequest`] and
//! route it through [`run`].
//!
//! Behaviour mirrors FT-039 §Behaviour: query the verify-env named
//! graph via SPARQL, apply optional safety-class / env-type filters,
//! return env summaries in ascending `ENV-NNN` order.

mod query;
mod render;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};
use crate::core::ontology::verification_env::SafetyClass;
use crate::core::vocab::{
    SAFETY_ISOLATED, SAFETY_PRODUCTION_READONLY, SAFETY_SHARED_NON_DESTRUCTIVE,
};

pub use render::{render_json, render_stderr_warnings, render_table};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_env_list";

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
pub struct EnvListRequest {
    /// Optional safety-class filter — one of the three controlled values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_class: Option<String>,
    /// Optional env-type filter (e.g. `ephemeral-tempdir`, `remote-http`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_type: Option<String>,
    /// Output format. Defaults to `Table` for CLI rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Working directory the handler reads against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// One row of the listing — matches the spec's `EnvSummary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSummary {
    /// `ENV-NNN[-suffix]` identifier.
    pub id: String,
    /// `dec:envType` value.
    pub env_type: String,
    /// `dec:safetyClass` value.
    pub safety_class: String,
    /// `dec:endpoint` value (omitted when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Ordered list of allowed-ops tokens. Empty when [`error`] flags the
    /// row as corrupt (TC-096): we cannot render an authoritative list, so
    /// downstream consumers should branch on `error.is_some()` first.
    pub allowed_ops: Vec<String>,
    /// `dec:setup` snippet (omitted when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// `dec:teardown` snippet (omitted when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teardown: Option<String>,
    /// `dec:fixtureSource` path (omitted when absent). FT-053 / ADR-032.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_source: Option<String>,
    /// Structured per-row error marker (TC-096). Absent for healthy envs;
    /// present when the row could not be fully projected — e.g. an env
    /// with two `dec:allowedOps` heads from a write-path bug or a manual
    /// store edit. The listing surfaces the row anyway so an operator
    /// can triage rather than seeing the whole command abort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<EnvRowError>,
}

/// Structured per-row error marker for corrupt env entries (TC-096).
///
/// Carries enough information for an operator to identify what went
/// wrong: the discriminant lives on `kind`, and `detail` is a human-
/// readable description. Additional structured context lives on the
/// variant fields (e.g. [`EnvRowErrorKind::MultipleAllowedOpsHeads`]
/// carries the head count).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvRowError {
    /// Discriminant identifying the failure mode (e.g.
    /// `"MultipleAllowedOpsHeads"`).
    pub kind: String,
    /// Optional head count — populated for
    /// `MultipleAllowedOpsHeads`/`CyclicAllowedOps`, omitted otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    /// One-line human-readable description.
    pub detail: String,
}

impl EnvRowError {
    /// Build a `MultipleAllowedOpsHeads` marker. Used when the SPARQL
    /// projection observes more than one `dec:allowedOps` triple bound
    /// to the same env subject.
    #[must_use]
    pub fn multiple_allowed_ops_heads(count: usize) -> Self {
        Self {
            kind: "MultipleAllowedOpsHeads".to_string(),
            count: Some(count),
            detail: format!("env declares {count} dec:allowedOps heads"),
        }
    }

    /// Generic fallback marker — used when a corruption shape doesn't
    /// match one of the structured kinds. Carries no count.
    #[must_use]
    pub fn corrupt(detail: impl Into<String>) -> Self {
        Self {
            kind: "Corrupt".to_string(),
            count: None,
            detail: detail.into(),
        }
    }
}

/// Structured response — surfaced verbatim by MCP, rendered as text or
/// JSON by the CLI per `req.format`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvListResponse {
    /// Ordered list of env summaries (ascending `ENV-NNN` numeric tail).
    pub envs: Vec<EnvSummary>,
}

/// Parse the structured `Request` envelope into [`EnvListRequest`].
pub fn parse_request(req: &Request) -> Result<EnvListRequest, HandlerError> {
    let mut parsed: EnvListRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_env_list arguments: {e}"),
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
        "List dec:VerificationEnvironment artifacts (FT-039 / ADR-028).",
        input_schema(),
        tool_handler(),
    )
    .with_output_schema(output_schema())
}

/// MCP handler closure — builds an [`EnvListRequest`] from the wire
/// envelope and renders the response back as a structured Value.
fn tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!("listed {n} environment(s)", n = outcome.envs.len());
        Ok(Response::with_summary(
            json!({ "envs": outcome.envs }),
            summary,
        ))
    })
}

/// JSON Schema for the MCP tool's structured output.
fn output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["envs"],
        "properties": {
            "envs": {
                "type": "array",
                "items": env_summary_schema(),
            },
        },
    })
}

/// JSON Schema fragment describing one [`EnvSummary`] row.
fn env_summary_schema() -> Value {
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
            "error": {
                "type": "object",
                "description": "Per-row corruption marker (TC-096). Present iff the row could not be fully projected.",
                "required": ["kind", "detail"],
                "properties": {
                    "kind": { "type": "string" },
                    "count": { "type": "integer" },
                    "detail": { "type": "string" },
                },
            },
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
            "safety_class": {
                "type": "string",
                "enum": [
                    SAFETY_ISOLATED,
                    SAFETY_SHARED_NON_DESTRUCTIVE,
                    SAFETY_PRODUCTION_READONLY,
                ],
            },
            "env_type": { "type": "string", "minLength": 1 },
            "format": { "type": "string", "enum": ["table", "json"] },
            "workdir": { "type": "string" },
        },
    })
}

/// Validate filter inputs ahead of the SPARQL query so unknown filter
/// values surface as `InvalidArgument` (exit 2 on the CLI).
fn validate(req: &EnvListRequest) -> Result<(), HandlerError> {
    if let Some(sc) = &req.safety_class {
        if SafetyClass::parse(sc).is_none() {
            return Err(HandlerError::InvalidArgument {
                field: "safety_class".to_string(),
                detail: format!(
                    "safety-class must be one of {{{SAFETY_ISOLATED}, \
                     {SAFETY_SHARED_NON_DESTRUCTIVE}, {SAFETY_PRODUCTION_READONLY}}}; \
                     got {got:?}",
                    got = sc,
                ),
            });
        }
    }
    if let Some(t) = &req.env_type {
        if t.trim().is_empty() {
            return Err(HandlerError::InvalidArgument {
                field: "env_type".to_string(),
                detail: "env-type filter must be a non-empty string".to_string(),
            });
        }
    }
    Ok(())
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &EnvListRequest) -> Result<EnvListResponse, HandlerError> {
    validate(req)?;
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    let mut envs = query::query_envs(
        workdir,
        req.safety_class.as_deref(),
        req.env_type.as_deref(),
    )?;
    envs.sort_by(|a, b| query::env_sort_key(&a.id).cmp(&query::env_sort_key(&b.id)));
    Ok(EnvListResponse { envs })
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
        let req = EnvListRequest {
            safety_class: Some(SAFETY_ISOLATED.to_string()),
            env_type: Some("ephemeral-tempdir".to_string()),
            format: Some(OutputFormat::Json),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: EnvListRequest = serde_json::from_value(v).expect("de");
        assert_eq!(req, back);
    }

    #[test]
    fn validate_rejects_unknown_safety_class() {
        let req = EnvListRequest {
            safety_class: Some("yolo".to_string()),
            ..Default::default()
        };
        let err = validate(&req).expect_err("unknown safety class must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "safety_class"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_accepts_empty_filters() {
        let req = EnvListRequest::default();
        validate(&req).expect("no filters should pass validation");
    }

    #[test]
    fn validate_rejects_blank_env_type() {
        let req = EnvListRequest {
            env_type: Some("   ".to_string()),
            ..Default::default()
        };
        let err = validate(&req).expect_err("blank env_type must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "env_type"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn input_schema_advertises_optional_filters() {
        let s = input_schema();
        let props = s
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("props");
        assert!(props.contains_key("safety_class"));
        assert!(props.contains_key("env_type"));
        assert!(props.contains_key("format"));
    }
}
