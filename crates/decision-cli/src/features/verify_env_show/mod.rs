//! `dec verify env show` — single-handler implementation (FT-040 / ADR-029).
//!
//! Read-only detail view of a single `dec:VerificationEnvironment`. One
//! handler, two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct an [`EnvShowRequest`] and route
//! it through [`run`].
//!
//! Behaviour mirrors FT-040 §Behaviour: parse the id, locate the
//! on-disk env file under `.dec/verify/env/<id>.ttl`, reconstruct the
//! full `VerificationEnvironment` value, and return its document plus
//! the resolved path. Unknown ids surface as
//! [`HandlerError::ArtifactNotFound`]; malformed ids surface as
//! [`HandlerError::InvalidArgument`].

mod render;
mod resolve;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};
use crate::core::ontology::verification_env::{SafetyClass, VerificationEnvironment};

pub use render::{render_json, render_text};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_env_show";

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
pub struct EnvShowRequest {
    /// Identifier of the env to show (e.g. `ENV-001-ephemeral-cli`).
    pub id: String,
    /// Output format. Defaults to `Text` for CLI rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Working directory the handler reads against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Full env document — every property of a `VerificationEnvironment`,
/// optional fields omitted when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvDocument {
    /// `ENV-NNN[-suffix]` identifier.
    pub id: String,
    /// `dec:envType` value.
    pub env_type: String,
    /// `dec:safetyClass` value.
    pub safety_class: String,
    /// `dec:endpoint` value (omitted when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Ordered list of allowed-ops tokens.
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
}

impl EnvDocument {
    /// Project a [`VerificationEnvironment`] into its on-the-wire shape.
    #[must_use]
    pub fn from_env(env: &VerificationEnvironment) -> Self {
        Self {
            id: env.id.clone(),
            env_type: env.env_type.clone(),
            safety_class: env.safety_class.as_str().to_string(),
            endpoint: env.endpoint.clone(),
            allowed_ops: env.allowed_ops.clone(),
            setup: env.setup.clone(),
            teardown: env.teardown.clone(),
            fixture_source: env.fixture_source.clone(),
        }
    }

    /// Reconstruct a [`VerificationEnvironment`] from the document. The
    /// id and safety class are validated; unknown safety values surface
    /// as `HandlerError::Internal` (they would have failed SHACL on the
    /// write path, so seeing them here is a graph-corruption signal).
    pub fn to_env(&self) -> Result<VerificationEnvironment, HandlerError> {
        let safety =
            SafetyClass::parse(&self.safety_class).ok_or_else(|| HandlerError::Internal {
                detail: format!(
                    "unknown safety class {got:?} in env document",
                    got = self.safety_class,
                ),
            })?;
        Ok(VerificationEnvironment {
            id: self.id.clone(),
            env_type: self.env_type.clone(),
            setup: self.setup.clone(),
            teardown: self.teardown.clone(),
            allowed_ops: self.allowed_ops.clone(),
            safety_class: safety,
            endpoint: self.endpoint.clone(),
            fixture_source: self.fixture_source.clone(),
        })
    }
}

/// Structured response — surfaced verbatim by MCP, rendered as text or
/// JSON by the CLI per `req.format`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvShowResponse {
    /// Full env document.
    pub env: EnvDocument,
    /// Absolute path of the on-disk `.dec/verify/env/<id>.ttl` file.
    pub path: PathBuf,
}

/// Parse the structured `Request` envelope into [`EnvShowRequest`].
pub fn parse_request(req: &Request) -> Result<EnvShowRequest, HandlerError> {
    let mut parsed: EnvShowRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_env_show arguments: {e}"),
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
        "Show a single dec:VerificationEnvironment artifact (FT-040 / ADR-028).",
        input_schema(),
        tool_handler(),
    )
    .with_output_schema(output_schema())
}

/// MCP handler closure — runs the single handler and renders the response.
fn tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
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

/// JSON Schema for the MCP tool's structured output.
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

/// JSON Schema fragment describing the [`EnvDocument`] shape.
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

/// Validate the id format ahead of any I/O. FT-040 §Error handling:
/// malformed ids surface as `InvalidArgument { field: "id" }` (exit 2),
/// distinct from missing ids which surface as `ArtifactNotFound` (exit 1).
fn validate_id(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("ENV-") {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("env id must start with 'ENV-', got {id:?}"),
        });
    }
    if id.len() < 5 {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("env id {id:?} is too short (expected ENV-NNN[-suffix])"),
        });
    }
    // The tail immediately after "ENV-" must start with a digit so
    // bare prefixes ("ENV-foo") never resolve to a file.
    let tail = &id[4..];
    if !tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("env id {id:?} must match ENV-NNN[-suffix]"),
        });
    }
    Ok(())
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &EnvShowRequest) -> Result<EnvShowResponse, HandlerError> {
    validate_id(&req.id)?;
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    let env_dir = workdir.join(".dec").join("verify").join("env");
    let (path, env) = resolve::load_env(&env_dir, &req.id)?;
    Ok(EnvShowResponse {
        env: EnvDocument::from_env(&env),
        path: path.canonicalize().unwrap_or(path),
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
        let req = EnvShowRequest {
            id: "ENV-001".to_string(),
            format: Some(OutputFormat::Json),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: EnvShowRequest = serde_json::from_value(v).expect("de");
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
        let err = validate_id("ENV-").expect_err("short id must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_rejects_non_numeric_tail() {
        let err = validate_id("ENV-foo").expect_err("non-numeric tail must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_accepts_plain_env_nnn() {
        validate_id("ENV-007").expect("ENV-007 should be a valid id format");
    }

    #[test]
    fn validate_id_accepts_env_nnn_with_suffix() {
        validate_id("ENV-001-ephemeral-cli").expect("suffixed id should be valid");
    }

    #[test]
    fn env_document_round_trips_to_env() {
        let env = VerificationEnvironment {
            id: "ENV-007".to_string(),
            env_type: "ephemeral-tempdir".to_string(),
            setup: Some("echo hi".to_string()),
            teardown: None,
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: None,
        };
        let doc = EnvDocument::from_env(&env);
        let back = doc.to_env().expect("to_env");
        assert_eq!(env, back);
    }

    /// FT-053: EnvDocument round-trips with a non-None fixture_source.
    #[test]
    fn env_document_round_trips_with_fixture_source() {
        let env = VerificationEnvironment {
            id: "ENV-008".to_string(),
            env_type: "ephemeral-tempdir".to_string(),
            setup: None,
            teardown: None,
            allowed_ops: vec!["shell".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: Some("tests/fixtures/dec-implement-basic".to_string()),
        };
        let doc = EnvDocument::from_env(&env);
        let back = doc.to_env().expect("to_env");
        assert_eq!(env, back);
        assert_eq!(
            doc.fixture_source.as_deref(),
            Some("tests/fixtures/dec-implement-basic"),
        );
    }
}
