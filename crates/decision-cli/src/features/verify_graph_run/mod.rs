//! `dec verify graph run` — single-handler implementation (FT-099 / ADR-029).
//!
//! Thin wrapper around `core::verify::runner::run_graph` per FT-099
//! §Behaviour. Both the CLI surface and the MCP twin
//! `dec_verify_graph_run` construct a [`GraphRunRequest`] and route it
//! through [`run`]; surface adapters only translate args + render
//! responses (text / json) and map verdict → exit code.

mod build;
mod render;
mod schema;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use oxigraph::model::NamedNode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};
use crate::core::ontology::verification_graph::{
    from_turtle as graph_from_turtle, VerificationGraph,
};
use crate::core::verify::runner::{run_graph, RunGraphRequest, RunnerError, TriggerKind};
use crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX;

pub use render::{render_json, render_text};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_graph_run";

const RUN_ACTIVITY_PREFIX: &str = "https://decision-cli.dev/ns/activity/verify-graph-run/";

/// Output format selector for the CLI surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable per-step trace (default).
    Text,
    /// Machine-readable JSON.
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl OutputFormat {
    /// Parse the wire value. `sse` falls back to `text` per FT-099
    /// §Error handling (the CLI emits a stderr warning).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" | "sse" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Structured request the single handler consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraphRunRequest {
    /// `VG-NNN[-suffix]` short id of the graph to execute.
    pub graph_id: String,
    /// Pre-seeded capture bindings (e.g. `${health_url}`).
    #[serde(default)]
    pub capture_bindings: HashMap<String, String>,
    /// Skip feedback emission (CI/diagnostic mode).
    #[serde(default)]
    pub no_feedback: bool,
    /// Keep ephemeral env tempdir for debugging.
    #[serde(default)]
    pub keep_tmp: bool,
    /// Working directory the handler runs against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Per-step summary returned to surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSummary {
    /// 0-based position in the graph.
    pub index: usize,
    /// Step kind tag (e.g. `shell-command`).
    pub kind: String,
    /// Step outcome (`pass` / `fail` / `unrunnable`).
    pub outcome: String,
    /// Duration in milliseconds (from started/ended timestamps).
    pub duration_ms: u64,
    /// One-line description suitable for the trace renderer.
    pub description: String,
    /// IRI of the step.
    pub step_id: String,
    /// IRI of the persisted `dec:VerificationStepTrace`.
    pub trace_id: String,
}

/// Structured response — both surfaces consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRunResponse {
    /// IRI of the persisted `dec:VerificationGraphResult`.
    pub result_id: String,
    /// IRI of the graph that was executed.
    pub graph_id: String,
    /// IRI of the environment used.
    pub environment_id: String,
    /// Final per-graph verdict.
    pub verdict: String,
    /// Operator-facing rationale (≥ 20 chars).
    pub rationale: String,
    /// Per-step traces in graph order.
    pub step_outcomes: Vec<StepSummary>,
    /// Emitted `dec:Feedback` IRIs (empty when no failures or
    /// `no_feedback` set).
    pub emitted_feedback: Vec<String>,
    /// Optional session IRI when the harness creates one (not populated in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl GraphRunResponse {
    /// Map the verdict string to the FT-099 exit-code contract.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.verdict.as_str() {
            "approved" => 0,
            "amendment-required" => 2,
            "rejected" => 1,
            _ => 1,
        }
    }
}

/// Parse the structured `Request` envelope into [`GraphRunRequest`].
pub fn parse_request(req: &Request) -> Result<GraphRunRequest, HandlerError> {
    let mut parsed: GraphRunRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_graph_run arguments: {e}"),
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
        Ok(response_for(&outcome))
    });
    ToolDescriptor::new(
        TOOL_NAME,
        "Run a dec:VerificationGraph and persist its VerificationGraphResult (FT-099 / ADR-028).",
        schema::input(),
        handler,
    )
    .with_output_schema(schema::output())
}

/// Build an MCP response payload from the handler's outcome.
#[must_use]
pub fn response_for(outcome: &GraphRunResponse) -> Response {
    let summary = format!(
        "ran graph {graph}: verdict {verdict}",
        graph = outcome.graph_id,
        verdict = outcome.verdict,
    );
    Response::with_summary(
        json!({
            "session_id": outcome.session_id,
            "result_id": outcome.result_id,
            "graph_id": outcome.graph_id,
            "environment_id": outcome.environment_id,
            "verdict": outcome.verdict,
            "rationale": outcome.rationale,
            "step_outcomes": outcome.step_outcomes,
            "emitted_feedback": outcome.emitted_feedback,
        }),
        summary,
    )
}

/// Single handler shared by CLI and MCP.
pub fn run(req: &GraphRunRequest) -> Result<GraphRunResponse, HandlerError> {
    let workdir = req
        .workdir
        .clone()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    validate_graph_id(&req.graph_id)?;
    let graph_iri = format!("{IRI_DEC_VERIFY_GRAPH_PREFIX}{}", req.graph_id);
    let graph_node = NamedNode::new(&graph_iri).map_err(|e| HandlerError::InvalidArgument {
        field: "graph_id".to_string(),
        detail: format!("graph IRI {graph_iri:?} could not be constructed: {e}"),
    })?;
    if req.keep_tmp {
        std::env::set_var("DEC_KEEP_TMP", "1");
    }
    let run_activity = mint_run_activity(&req.graph_id);
    let runner_req = RunGraphRequest {
        graph: graph_node,
        triggered_by: TriggerKind::Manual,
        capture_bindings: req.capture_bindings.clone(),
        run_activity,
        workdir: workdir.clone(),
    };
    let graph = load_graph(&workdir, &req.graph_id);
    match run_graph(&runner_req) {
        Ok(resp) => Ok(build::from_runner(resp, req.no_feedback, graph.as_ref())),
        Err(err) => Err(map_runner_error(err)),
    }
}

fn load_graph(workdir: &std::path::Path, graph_id: &str) -> Option<VerificationGraph> {
    let path = workdir
        .join(".dec")
        .join("verify")
        .join("graph")
        .join(format!("{graph_id}.ttl"));
    if !path.is_file() {
        return None;
    }
    graph_from_turtle(&path).ok()
}

fn validate_graph_id(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("VG-") {
        return Err(HandlerError::InvalidArgument {
            field: "graph_id".to_string(),
            detail: format!("graph id must start with 'VG-', got {id:?}"),
        });
    }
    if id.len() < 4 {
        return Err(HandlerError::InvalidArgument {
            field: "graph_id".to_string(),
            detail: format!("graph id {id:?} is too short (expected VG-NNN[-suffix])"),
        });
    }
    Ok(())
}

fn mint_run_activity(graph_id: &str) -> NamedNode {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let iri = format!("{RUN_ACTIVITY_PREFIX}{graph_id}/ts-{nanos}");
    NamedNode::new_unchecked(iri)
}

fn map_runner_error(err: RunnerError) -> HandlerError {
    match err {
        RunnerError::ArtifactNotFound { kind, id } => HandlerError::ArtifactNotFound { kind, id },
        RunnerError::SafetyViolation { step, op } => HandlerError::Internal {
            detail: format!("safety violation: step <{step}> requires op <{op}>"),
        },
        RunnerError::EnvSetupFailed {
            exit_code,
            stderr_excerpt,
        } => HandlerError::Internal {
            detail: format!("env setup failed (exit {exit_code}): {stderr_excerpt}"),
        },
        RunnerError::ResultWriteFailed { source } => HandlerError::Internal {
            detail: format!("result persistence failed: {source:#}"),
        },
        RunnerError::Internal { detail } => HandlerError::Internal { detail },
    }
}

/// Compose the verify-graph IRI for `short_id` (e.g. `VG-NNN`).
#[must_use]
pub fn graph_iri_for(short_id: &str) -> String {
    if short_id.starts_with("https://") {
        short_id.to_string()
    } else {
        format!("{IRI_DEC_VERIFY_GRAPH_PREFIX}{short_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_parse_accepts_text_json_sse() {
        assert_eq!(OutputFormat::parse("text"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("sse"), Some(OutputFormat::Text));
        assert!(OutputFormat::parse("yaml").is_none());
    }

    #[test]
    fn exit_code_maps_per_ft_099() {
        let mut r = GraphRunResponse {
            result_id: "r".into(),
            graph_id: "g".into(),
            environment_id: "e".into(),
            verdict: "approved".into(),
            rationale: "n/a".into(),
            step_outcomes: Vec::new(),
            emitted_feedback: Vec::new(),
            session_id: None,
        };
        assert_eq!(r.exit_code(), 0);
        r.verdict = "rejected".into();
        assert_eq!(r.exit_code(), 1);
        r.verdict = "amendment-required".into();
        assert_eq!(r.exit_code(), 2);
    }

    #[test]
    fn validate_graph_id_requires_vg_prefix() {
        assert!(validate_graph_id("VG-001").is_ok());
        assert!(validate_graph_id("VG-007-foo").is_ok());
        assert!(validate_graph_id("vg-001").is_err());
        assert!(validate_graph_id("FT-001").is_err());
        assert!(validate_graph_id("VG-").is_err());
    }
}
