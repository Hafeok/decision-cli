//! `dec loop show <FT-NNN>` — chronological audit chain for one feature.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::render::OutputFormat;
use crate::core::feedback::read::list_by_class;
use crate::core::handler::Error as HandlerError;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};

/// Wire request for the show handler.
#[derive(Debug, Clone)]
pub struct LoopShowRequest {
    pub feature_id: String,
    pub workdir: PathBuf,
    pub product_root: Option<PathBuf>,
    pub format: OutputFormat,
}

/// Per-feedback entry in the chronological chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopEntry {
    pub feedback_iri: String,
    pub class: String,
    pub target_role: String,
    pub state: String,
    pub severity: String,
    pub evidence: String,
    pub source_session: String,
    pub source_session_short: String,
    pub source_tc: String,
    pub source_tc_short: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addressing_artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addressing_artifact_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiving_session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiving_session_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routed_at: Option<String>,
}

/// Outcome of the show handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopShowResponse {
    pub feature_id: String,
    pub entries: Vec<LoopEntry>,
}

/// Single handler — both CLI and (future) MCP surfaces consume this.
pub fn run(req: &LoopShowRequest) -> Result<LoopShowResponse, HandlerError> {
    let product_root = req
        .product_root
        .clone()
        .unwrap_or_else(|| req.workdir.clone());

    // 1. Resolve the feature's TC IRIs.
    let tc_shorts =
        resolve_feature_tcs_short(&product_root, &req.feature_id).map_err(|e| {
            HandlerError::Internal {
                detail: format!("loop show: resolving TC list: {e}"),
            }
        })?;
    let tc_iris: std::collections::HashSet<String> =
        tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

    // 2. Open the orchestration store and read every defect feedback.
    let dump = orchestration_dump_path(&req.workdir);
    let store = load_store_from_dump(&dump).map_err(|e| HandlerError::Internal {
        detail: format!(
            "loop show: opening orchestration store at {p}: {e}",
            p = dump.display()
        ),
    })?;
    let defects = list_by_class(&store, "defect").map_err(|e| HandlerError::Internal {
        detail: format!("loop show: reading defect feedback: {e}"),
    })?;

    // 3. Project each in-scope feedback to a LoopEntry.
    let mut entries: Vec<LoopEntry> = Vec::new();
    for fb in defects {
        let Some(source_tc) = fb.source_artifact.as_ref() else {
            continue;
        };
        let source_tc_str = source_tc.as_str().to_string();
        if !tc_iris.contains(&source_tc_str) {
            continue;
        }
        let source_session = fb.source_session.as_str().to_string();
        entries.push(LoopEntry {
            feedback_iri: fb.iri.as_str().to_string(),
            class: fb.class.clone(),
            target_role: fb.target_role.clone(),
            state: fb.lifecycle_state.clone(),
            severity: fb.severity.as_str().to_string(),
            evidence: fb.evidence.clone(),
            source_session_short: super::resolver::short_for_session(&source_session),
            source_session,
            source_tc_short: super::resolver::short_for_tc(&source_tc_str),
            source_tc: source_tc_str,
            addressing_artifact: fb.addressing_artifact.as_ref().map(|n| n.as_str().to_string()),
            addressing_artifact_short: fb
                .addressing_artifact
                .as_ref()
                .map(|n| super::resolver::short_for_artifact(n.as_str())),
            receiving_session: fb.receiving_session.as_ref().map(|n| n.as_str().to_string()),
            receiving_session_short: fb
                .receiving_session
                .as_ref()
                .map(|n| super::resolver::short_for_session(n.as_str())),
            routed_at: fb.routed_at.clone(),
        });
    }

    // 4. Sort chronologically — routed_at when set, else fall back to a
    //    stable feedback-iri tiebreak so the order is deterministic.
    entries.sort_by(|a, b| {
        let key_a = a.routed_at.clone().unwrap_or_default();
        let key_b = b.routed_at.clone().unwrap_or_default();
        match key_a.cmp(&key_b) {
            std::cmp::Ordering::Equal => a.feedback_iri.cmp(&b.feedback_iri),
            other => other,
        }
    });

    Ok(LoopShowResponse {
        feature_id: req.feature_id.clone(),
        entries,
    })
}
