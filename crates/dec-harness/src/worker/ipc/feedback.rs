//! Worker emit_feedback record ingest chokepoint (FT-031 / ADR-022).
//!
//! Workers emit feedback by writing one sentinel-prefixed JSON line per
//! emission on their stdout (see `workers/_shared/src/_shared/feedback.py`).
//! The harness scans those lines after the worker exits, parses each
//! into a typed [`FeedbackEmission`], then commits a `dec:Feedback`
//! artifact in state `produced` per emission through `StreamWriter`.
//!
//! Workers never touch the graph (ADR-008). This module is the single
//! chokepoint for the ingest path; bypassing it would re-introduce the
//! worker / graph coupling the contract forbids.

use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::feedback::artifact::{Feedback, Severity};
use crate::feedback::class::{Disposition, FeedbackClass};
use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::orchestration_graph;

/// Sentinel prefix workers write on stdout in front of every feedback
/// JSON record. Mirrors the constant in the Python SDK
/// (`workers/_shared/src/_shared/feedback.py`).
pub const FEEDBACK_RECORD_SENTINEL: &str = "__DEC_FEEDBACK__";

/// A single worker feedback emission, mirroring the Python SDK's
/// `FeedbackEmission` Pydantic model. Wire format is JSON; the [`parse_records`]
/// scanner produces these from a worker's captured stdout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackEmission {
    /// ADR-023 controlled-vocabulary class tag (`gap`, `contradiction`, …).
    pub feedback_class: String,
    /// Severity hint (`low`, `medium`, `high`).
    pub severity: String,
    /// Free-form citation into the bundle (≥ 20 chars per the Python SDK).
    pub evidence: String,
    /// Optional suggested fix for the target role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
    /// Per-emission target role override (defaults to the class default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_role_override: Option<String>,
    /// Per-emission blocking override (`None` ⇒ class default applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking: Option<bool>,
    /// Rationale recorded when blocking differs from the class default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_rationale: Option<String>,
    /// Session IRI the worker wrote into the record. Empty string ⇒ the
    /// harness substitutes the active session at write time (FT-031).
    #[serde(default)]
    pub source_session: String,
}

/// Errors returned when scanning a worker's stdout for feedback records.
#[derive(Debug, Error)]
pub enum ParseRecordError {
    /// A sentinel-prefixed line was found but the JSON payload failed to
    /// deserialise into a [`FeedbackEmission`].
    #[error("malformed feedback record at line {line_no}: {source}")]
    Malformed {
        /// 1-based line number in the input stream.
        line_no: usize,
        /// Underlying serde error.
        source: serde_json::Error,
    },
}

/// Errors surfaced when writing a parsed emission through `StreamWriter`.
#[derive(Debug, Error)]
pub enum FeedbackApplyError {
    /// The emission carried an empty `source_session`; the caller did
    /// not provide a default — the resulting `Feedback` cannot satisfy
    /// SHACL `dec:sourceSession minCount 1`.
    #[error("emission has no source_session; no active session was provided")]
    NoActiveSession,
    /// Failed to mint a `NamedNode` for one of the artifact's IRIs.
    #[error("invalid IRI while constructing feedback artifact: {0}")]
    InvalidIri(String),
    /// Underlying [`StreamWriter::commit`] failed (SHACL rejection,
    /// transactional error, …).
    #[error("StreamWriter rejected feedback mutation: {0}")]
    Commit(String),
}

/// Scan a worker's stdout (or any line-delimited text) for emit_feedback
/// records. Returns parsed records plus a parallel `Vec<ParseRecordError>`
/// so the harness can record parse failures in session telemetry without
/// dropping the good records.
///
/// Lines that do not start with [`FEEDBACK_RECORD_SENTINEL`] are ignored
/// — workers print their normal output (logs, `WorkerResponse` JSON,
/// progress messages) on the same stream.
#[must_use]
pub fn parse_records(stream: &str) -> (Vec<FeedbackEmission>, Vec<ParseRecordError>) {
    let mut ok: Vec<FeedbackEmission> = Vec::new();
    let mut errs: Vec<ParseRecordError> = Vec::new();
    for (idx, raw) in stream.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if !line.starts_with(FEEDBACK_RECORD_SENTINEL) {
            continue;
        }
        let payload = line[FEEDBACK_RECORD_SENTINEL.len()..].trim_start();
        match serde_json::from_str::<FeedbackEmission>(payload) {
            Ok(emission) => ok.push(emission),
            Err(e) => errs.push(ParseRecordError::Malformed {
                line_no: idx + 1,
                source: e,
            }),
        }
    }
    (ok, errs)
}

/// Construct a [`Feedback`] artifact in state `produced` from a parsed
/// emission. Populates the source session from `active_session` when
/// the worker left the field blank, then commits the artifact through
/// `writer`.
///
/// The returned IRI is the freshly-minted `dec:Feedback` IRI so callers
/// can correlate the artifact with downstream events (e.g. FT-032
/// `paused-for-feedback` transitions).
pub fn apply(
    writer: &StreamWriter,
    emission: &FeedbackEmission,
    active_session: Option<&NamedNode>,
) -> Result<NamedNode, FeedbackApplyError> {
    let session = resolve_source_session(emission, active_session)?;
    let in_stream = writer.active_stream().clone();
    let iri = mint_feedback_iri()?;
    let feedback = build_feedback(emission, iri.clone(), session, in_stream);
    let quads = feedback.to_quads(orchestration_graph());
    let mutation = Mutation {
        inserts: quads,
        ..Mutation::default()
    };
    writer
        .commit(mutation)
        .map_err(|e| FeedbackApplyError::Commit(format!("{e:#}")))?;
    Ok(iri)
}

fn build_feedback(
    emission: &FeedbackEmission,
    iri: NamedNode,
    session: NamedNode,
    in_stream: NamedNode,
) -> Feedback {
    let target_role = resolve_target_role(emission);
    let severity = resolve_severity(&emission.severity);
    let (disposition_override, disposition_rationale) = resolve_disposition(emission);
    Feedback {
        iri,
        class: emission.feedback_class.clone(),
        severity,
        target_role,
        evidence: emission.evidence.clone(),
        recommendation: emission.recommendation.clone(),
        lifecycle_state: "produced".to_string(),
        source_session: session,
        source_artifact: None,
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override,
        disposition_rationale,
        in_stream,
    }
}

fn resolve_source_session(
    emission: &FeedbackEmission,
    active_session: Option<&NamedNode>,
) -> Result<NamedNode, FeedbackApplyError> {
    if !emission.source_session.is_empty() {
        return NamedNode::new(emission.source_session.as_str())
            .map_err(|e| FeedbackApplyError::InvalidIri(format!("source_session: {e}")));
    }
    active_session
        .cloned()
        .ok_or(FeedbackApplyError::NoActiveSession)
}

fn resolve_target_role(emission: &FeedbackEmission) -> String {
    if let Some(override_role) = emission.target_role_override.as_ref() {
        if !override_role.is_empty() {
            return override_role.clone();
        }
    }
    FeedbackClass::from_iri_value(&emission.feedback_class)
        .map(|c| c.default_target_role().to_string())
        // Unknown class — surface as a feedback artifact anyway (per
        // ADR-022 error handling: harness writes, SHACL catches), with
        // the empty target role triggering the SHACL minCount-1 rule.
        .unwrap_or_default()
}

fn resolve_severity(wire: &str) -> Severity {
    // Map the Python SDK's low/medium/high vocabulary onto the Rust enum.
    match wire {
        "low" => Severity::Info,
        "high" => Severity::Error,
        // "medium" is the SDK default; unknown values fall back to it.
        _ => Severity::Warning,
    }
}

fn resolve_disposition(emission: &FeedbackEmission) -> (Option<String>, Option<String>) {
    let Some(blocking) = emission.blocking else {
        return (None, emission.disposition_rationale.clone());
    };
    let explicit = if blocking {
        Disposition::Blocking
    } else {
        Disposition::NonBlocking
    };
    let class_default =
        FeedbackClass::from_iri_value(&emission.feedback_class).map(|c| c.default_disposition());
    if class_default == Some(explicit) {
        // The worker requested the class default — no override recorded.
        return (None, None);
    }
    (
        Some(explicit.as_str().to_string()),
        emission.disposition_rationale.clone(),
    )
}

fn mint_feedback_iri() -> Result<NamedNode, FeedbackApplyError> {
    let id = Uuid::new_v4();
    let raw = format!("urn:dec:feedback:{id}");
    NamedNode::new(raw.as_str()).map_err(|e| FeedbackApplyError::InvalidIri(format!("{e}")))
}

// Tests live in `feedback_tests.rs` so this file stays under ADR-013
// Rule 1's 400-line hard cap. The compile-only path attribute keeps
// them in the same compilation unit without forcing a re-export.
#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
