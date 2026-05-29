//! `dec verify bench show` — single-handler implementation (FT-040 / ADR-029).
//!
//! Read-only detail view of a single `dec:VerificationBench`. One
//! handler, two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct an [`BenchShowRequest`] and route
//! it through [`run`].
//!
//! Behaviour mirrors FT-040 §Behaviour: parse the id, locate the
//! on-disk bench file under `.dec/verify/bench/<id>.ttl`, reconstruct the
//! full `VerificationBench` value, and return its document plus
//! the resolved path. Unknown ids surface as
//! [`HandlerError::ArtifactNotFound`]; malformed ids surface as
//! [`HandlerError::InvalidArgument`].

mod render;
mod resolve;
mod schema;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::handler::{Error as HandlerError, Request};
use crate::core::ontology::verification_bench::{SafetyClass, VerificationBench};

pub use render::{render_json, render_text};
pub use schema::{input_schema, tool_descriptor};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_bench_show";

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
pub struct BenchShowRequest {
    /// Identifier of the bench to show (e.g. `BNCH-001-ephemeral-cli`).
    pub id: String,
    /// Output format. Defaults to `Text` for CLI rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Working directory the handler reads against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Full bench document — every property of a `VerificationBench`,
/// optional fields omitted when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvDocument {
    /// `BNCH-NNN[-suffix]` identifier.
    pub id: String,
    /// `dec:benchType` value.
    pub bench_type: String,
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
    /// Project a [`VerificationBench`] into its on-the-wire shape.
    #[must_use]
    pub fn from_env(bench: &VerificationBench) -> Self {
        Self {
            id: bench.id.clone(),
            bench_type: bench.bench_type.clone(),
            safety_class: bench.safety_class.as_str().to_string(),
            endpoint: bench.endpoint.clone(),
            allowed_ops: bench.allowed_ops.clone(),
            setup: bench.setup.clone(),
            teardown: bench.teardown.clone(),
            fixture_source: bench.fixture_source.clone(),
        }
    }

    /// Reconstruct a [`VerificationBench`] from the document. The
    /// id and safety class are validated; unknown safety values surface
    /// as `HandlerError::Internal` (they would have failed SHACL on the
    /// write path, so seeing them here is a graph-corruption signal).
    pub fn to_env(&self) -> Result<VerificationBench, HandlerError> {
        let safety =
            SafetyClass::parse(&self.safety_class).ok_or_else(|| HandlerError::Internal {
                detail: format!(
                    "unknown safety class {got:?} in bench document",
                    got = self.safety_class,
                ),
            })?;
        Ok(VerificationBench {
            id: self.id.clone(),
            bench_type: self.bench_type.clone(),
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
pub struct BenchShowResponse {
    /// Full bench document.
    pub bench: EnvDocument,
    /// Absolute path of the on-disk `.dec/verify/bench/<id>.ttl` file.
    pub path: PathBuf,
}

/// Parse the structured `Request` envelope into [`BenchShowRequest`].
pub fn parse_request(req: &Request) -> Result<BenchShowRequest, HandlerError> {
    let mut parsed: BenchShowRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_bench_show arguments: {e}"),
            }
        })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// MCP tool descriptor — registered by the binary in `cli::mcp`.
#[must_use]
/// Validate the id format ahead of any I/O. FT-040 §Error handling:
/// malformed ids surface as `InvalidArgument { field: "id" }` (exit 2),
/// distinct from missing ids which surface as `ArtifactNotFound` (exit 1).
fn validate_id(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("BNCH-") {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("bench id must start with 'BNCH-', got {id:?}"),
        });
    }
    if id.len() < 5 {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("bench id {id:?} is too short (expected BNCH-NNN[-suffix])"),
        });
    }
    // The tail immediately after "BNCH-" must start with a digit so
    // bare prefixes ("BNCH-foo") never resolve to a file.
    let tail = &id[4..];
    if !tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("bench id {id:?} must match BNCH-NNN[-suffix]"),
        });
    }
    Ok(())
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &BenchShowRequest) -> Result<BenchShowResponse, HandlerError> {
    validate_id(&req.id)?;
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    let bench_dir = workdir.join(".dec").join("verify").join("bench");
    let (path, bench) = resolve::load_env(&bench_dir, &req.id)?;
    Ok(BenchShowResponse {
        bench: EnvDocument::from_env(&bench),
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
        let req = BenchShowRequest {
            id: "BNCH-001".to_string(),
            format: Some(OutputFormat::Json),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: BenchShowRequest = serde_json::from_value(v).expect("de");
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
        let err = validate_id("BNCH-").expect_err("short id must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_rejects_non_numeric_tail() {
        let err = validate_id("BNCH-foo").expect_err("non-numeric tail must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn validate_id_accepts_plain_env_nnn() {
        validate_id("BNCH-007").expect("BNCH-007 should be a valid id format");
    }

    #[test]
    fn validate_id_accepts_env_nnn_with_suffix() {
        validate_id("BNCH-001-ephemeral-cli").expect("suffixed id should be valid");
    }

    #[test]
    fn env_document_round_trips_to_env() {
        let bench = VerificationBench {
            id: "BNCH-007".to_string(),
            bench_type: "ephemeral-tempdir".to_string(),
            setup: Some("echo hi".to_string()),
            teardown: None,
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: None,
        };
        let doc = EnvDocument::from_env(&bench);
        let back = doc.to_env().expect("to_env");
        assert_eq!(bench, back);
    }

    /// FT-053: EnvDocument round-trips with a non-None fixture_source.
    #[test]
    fn env_document_round_trips_with_fixture_source() {
        let bench = VerificationBench {
            id: "BNCH-008".to_string(),
            bench_type: "ephemeral-tempdir".to_string(),
            setup: None,
            teardown: None,
            allowed_ops: vec!["shell".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: Some("tests/fixtures/dec-implement-basic".to_string()),
        };
        let doc = EnvDocument::from_env(&bench);
        let back = doc.to_env().expect("to_env");
        assert_eq!(bench, back);
        assert_eq!(
            doc.fixture_source.as_deref(),
            Some("tests/fixtures/dec-implement-basic"),
        );
    }
}
