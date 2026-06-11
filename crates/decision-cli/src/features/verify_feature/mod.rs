//! `dec verify feature` — single-handler implementation (FT-099 / ADR-029).
//!
//! Roll-up entry: enumerate every `VerificationGraph` whose `dec:verifies`
//! or `dec:providesEvidenceFor` chain reaches the supplied feature, run
//! each `(graph, env)` tuple through `core::verify::runner::run_graph`
//! sequentially (v1), then compose the results through FT-097's
//! `aggregate_verdict`. Returns per-graph + per-TC + aggregate outcome
//! suitable for both CLI text/JSON rendering and MCP structured output.

mod enumerate;
mod orchestrate;
mod render;
mod schema;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};
use crate::core::verify::coverage::feature_resolver::{feature_iri_for, resolve_feature_tc_iris};

pub use enumerate::GraphTuple;
pub use render::{render_json, render_text};

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_feature";

/// Output format selector for the CLI surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable per-graph + per-TC + aggregate block (default).
    Text,
    /// Machine-readable JSON document.
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl OutputFormat {
    /// Parse the wire value.
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
pub struct FeatureVerifyRequest {
    /// `FT-NNN` short id (or full IRI) of the feature.
    pub feature_id: String,
    /// Optional environment filter (`BNCH-NNN[-suffix]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
    /// Skip Feedback emission for failing evidence-bearing steps.
    #[serde(default)]
    pub no_feedback: bool,
    /// Consider stale VGRs (v1 always re-runs).
    #[serde(default)]
    pub include_stale: bool,
    /// Enumerate-only mode (no execution, no artifacts written).
    #[serde(default)]
    pub dry_run: bool,
    /// Working directory the handler runs against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// One per-graph entry in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerGraphRow {
    /// Graph short id (`VG-NNN`).
    pub vg: String,
    /// Environment short id.
    pub env: String,
    /// Verdict — `None` for dry-run / would-reuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// IRI of the persisted (or reused) VGR — `None` for dry-run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    /// Status: `ran`, `reused`, `would-run`, `would-reuse`, `error`.
    pub status: String,
    /// Optional one-line note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One per-TC verdict row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerTcRow {
    /// TC short id (`TC-NNN`).
    pub tc: String,
    /// Aggregate verdict for that TC.
    pub verdict: String,
    /// One-line rationale.
    pub rationale: String,
    /// Contributing VGR IRIs.
    pub from_results: Vec<String>,
}

/// Structured response — surfaces consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVerifyResponse {
    /// Optional session IRI (not populated in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Feature short id (`FT-NNN`).
    pub feature_id: String,
    /// Enumerated `(graph, env)` runs.
    pub per_graph: Vec<PerGraphRow>,
    /// Per-TC verdicts. Empty for dry-run.
    pub per_tc: Vec<PerTcRow>,
    /// TCs with no covering verification graph (FT-099 §exit 3).
    pub coverage_gaps: Vec<String>,
    /// Aggregate verdict block. `None` for dry-run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateBlock>,
    /// True when the response was produced via the dry-run enumerator.
    #[serde(default)]
    pub dry_run: bool,
    /// Enumerated `(would_run, would_reuse)` shape for dry-run callers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enumeration: Option<EnumerationBlock>,
}

/// Aggregate verdict envelope returned to surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateBlock {
    /// `approved` / `rejected` / `amendment-required`.
    pub verdict: String,
    /// One-line operator-facing rationale.
    pub rationale: String,
}

/// Dry-run enumeration shape per FT-099 §Behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumerationBlock {
    /// `(graph, env)` tuples that would execute.
    pub would_run: Vec<EnumerationEntry>,
    /// Tuples that would be reused from a fresh VGR.
    pub would_reuse: Vec<EnumerationEntry>,
}

/// One dry-run enumeration row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumerationEntry {
    /// Graph short id.
    pub vg: String,
    /// Environment short id.
    pub env: String,
    /// Reused VGR IRI when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vgr: Option<String>,
}

impl FeatureVerifyResponse {
    /// Map the aggregate verdict + coverage-gap state to the FT-099
    /// exit-code contract: 0 / 1 / 2 / 3 (gap dominates 0).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if !self.coverage_gaps.is_empty() {
            return 3;
        }
        if self.dry_run {
            return 0;
        }
        match self.aggregate.as_ref().map(|a| a.verdict.as_str()) {
            Some("approved") => 0,
            Some("rejected") => 1,
            Some("amendment-required") => 2,
            _ => 1,
        }
    }
}

/// Parse the structured `Request` envelope into [`FeatureVerifyRequest`].
pub fn parse_request(req: &Request) -> Result<FeatureVerifyRequest, HandlerError> {
    let mut parsed: FeatureVerifyRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_feature arguments: {e}"),
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
        "Verify a feature by running every covering VerificationGraph and aggregating verdicts (FT-099 / ADR-028).",
        schema::input(),
        handler,
    )
    .with_output_schema(schema::output())
}

/// Build an MCP response payload from the handler outcome.
#[must_use]
pub fn response_for(outcome: &FeatureVerifyResponse) -> Response {
    let verdict = outcome
        .aggregate
        .as_ref()
        .map(|a| a.verdict.clone())
        .unwrap_or_else(|| "dry-run".to_string());
    let summary = format!(
        "verified feature {ft}: {verdict}",
        ft = outcome.feature_id,
        verdict = verdict
    );
    let value = serde_json::to_value(outcome).unwrap_or(Value::Null);
    Response::with_summary(value, summary)
}

/// Single handler.
pub fn run(req: &FeatureVerifyRequest) -> Result<FeatureVerifyResponse, HandlerError> {
    let workdir = req
        .workdir
        .clone()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    validate_feature_id(&req.feature_id)?;
    let tcs = resolve_feature_tc_iris(&workdir, &req.feature_id).map_err(|e| match e {
        crate::core::verify::CoverageError::ArtifactNotFound { kind, id } => {
            HandlerError::ArtifactNotFound { kind, id }
        }
        crate::core::verify::CoverageError::StoreUnreachable { detail } => {
            HandlerError::Internal { detail }
        }
    })?;
    let feature_iri = feature_iri_for(&req.feature_id);
    let tuples = enumerate::enumerate_runnable_tuples(
        &workdir,
        &feature_iri,
        &tcs,
        req.environment_id.as_deref(),
    )?;

    if req.dry_run {
        return Ok(dry_run_response(req, &tuples));
    }

    let outcome =
        orchestrate::execute_and_aggregate(&workdir, &req.feature_id, &feature_iri, &tcs, &tuples)?;
    Ok(FeatureVerifyResponse {
        session_id: None,
        feature_id: req.feature_id.clone(),
        per_graph: outcome.per_graph,
        per_tc: outcome.per_tc,
        coverage_gaps: outcome.coverage_gaps,
        aggregate: Some(outcome.aggregate),
        dry_run: false,
        enumeration: None,
    })
}

fn dry_run_response(req: &FeatureVerifyRequest, tuples: &[GraphTuple]) -> FeatureVerifyResponse {
    let would_run: Vec<EnumerationEntry> = tuples
        .iter()
        .map(|t| EnumerationEntry {
            vg: t.graph_short.clone(),
            env: t.env_short.clone(),
            vgr: None,
        })
        .collect();
    FeatureVerifyResponse {
        session_id: None,
        feature_id: req.feature_id.clone(),
        per_graph: Vec::new(),
        per_tc: Vec::new(),
        coverage_gaps: Vec::new(),
        aggregate: None,
        dry_run: true,
        enumeration: Some(EnumerationBlock {
            would_run,
            would_reuse: Vec::new(),
        }),
    }
}

fn validate_feature_id(id: &str) -> Result<(), HandlerError> {
    if id.starts_with("https://") {
        return Ok(());
    }
    if !id.starts_with("FT-") || id.len() < 4 {
        return Err(HandlerError::InvalidArgument {
            field: "feature_id".to_string(),
            detail: format!("feature id must match 'FT-NNN'; got {id:?}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_parse_text_json() {
        assert_eq!(OutputFormat::parse("text"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert!(OutputFormat::parse("yaml").is_none());
    }

    #[test]
    fn exit_code_gap_dominates_approval() {
        let resp = FeatureVerifyResponse {
            session_id: None,
            feature_id: "FT-001".into(),
            per_graph: Vec::new(),
            per_tc: Vec::new(),
            coverage_gaps: vec!["TC-002".into()],
            aggregate: Some(AggregateBlock {
                verdict: "approved".into(),
                rationale: "ok".into(),
            }),
            dry_run: false,
            enumeration: None,
        };
        assert_eq!(resp.exit_code(), 3);
    }

    #[test]
    fn exit_code_maps_aggregate_verdicts() {
        let make = |v: &str| FeatureVerifyResponse {
            session_id: None,
            feature_id: "FT-001".into(),
            per_graph: Vec::new(),
            per_tc: Vec::new(),
            coverage_gaps: Vec::new(),
            aggregate: Some(AggregateBlock {
                verdict: v.into(),
                rationale: "x".into(),
            }),
            dry_run: false,
            enumeration: None,
        };
        assert_eq!(make("approved").exit_code(), 0);
        assert_eq!(make("rejected").exit_code(), 1);
        assert_eq!(make("amendment-required").exit_code(), 2);
    }

    #[test]
    fn exit_code_dry_run_is_zero() {
        let resp = FeatureVerifyResponse {
            session_id: None,
            feature_id: "FT-001".into(),
            per_graph: Vec::new(),
            per_tc: Vec::new(),
            coverage_gaps: Vec::new(),
            aggregate: None,
            dry_run: true,
            enumeration: Some(EnumerationBlock {
                would_run: Vec::new(),
                would_reuse: Vec::new(),
            }),
        };
        assert_eq!(resp.exit_code(), 0);
    }

    #[test]
    fn validate_feature_id_requires_ft_prefix() {
        assert!(validate_feature_id("FT-001").is_ok());
        assert!(validate_feature_id("foo").is_err());
    }
}
