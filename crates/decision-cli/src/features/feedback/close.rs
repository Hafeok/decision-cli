//! `dec feedback close <iri> --addressing <artifact-iri>` (FT-033).
//!
//! Closes a feedback artifact by transitioning `addressed → closed`
//! after validating the addressing artifact:
//!
//!   1. exists in the orchestration store,
//!   2. belongs to the same active stream (ADR-005), and
//!   3. is of a type the routing table's `addressing_roles` allowlist
//!      sanctions for the feedback's class (ADR-026).
//!
//! After a successful transition, this module calls
//! [`crate::core::dispatch::resume_check`] on every paused dispatch
//! whose `dec:blockedBy` includes this feedback so FT-032's pause/
//! resume machinery picks the unblocked path.
//!
//! Private helpers (validation, evidence-quad builders, actor-IRI
//! minting, resume-check cascade) live in `close_helpers.rs` to keep
//! this file under ADR-013 Rule 1's 400-line hard cap.

use std::path::Path;

use anyhow::{anyhow, Result};
use oxigraph::model::{NamedNode, Quad};

use crate::core::feedback::class::FeedbackClass;
use crate::core::feedback::transition::{apply as apply_transition, read_prior_state};
use crate::core::feedback::{get, LifecycleState};
use crate::core::vocab::orchestration_graph;

use super::close_helpers::{
    build_close_evidence, mint_actor_iri, read_artifact_metadata, run_resume_checks,
    validate_artifact_type_allowed,
};
use super::format::{json_escape, json_opt};
use super::store_io::WritableStore;

/// Structured outcome of a close operation; surfaces enough state for
/// both the human renderer and JSON path.
#[derive(Debug, Clone)]
pub struct CloseOutcome {
    /// IRI of the closed feedback artifact.
    pub feedback_iri: String,
    /// IRI of the addressing artifact recorded via `dec:addressingArtifact`.
    pub addressing_iri: String,
    /// Operator-supplied identity tag (CLI `--actor` or env). Surfaces
    /// in human output; the stored `dec:closedBy` IRI is derived from
    /// this value.
    pub closed_by: String,
    /// Stable IRI persisted as the object of `dec:closedBy`. Either
    /// the operator's value (if it parses as an IRI) or a synthetic
    /// `human-close-…` URN.
    pub closed_by_iri: String,
    /// IRIs of paused DispatchGroups whose resume_check was triggered.
    /// Includes the new outcome from the check (`Resumed`, `Blocked`,
    /// `StillBlocked`, `NotPaused`) so operators see the cascade.
    pub resumed_groups: Vec<ResumedGroup>,
}

/// One paused DispatchGroup that this close attempt unblocked (or kept
/// blocked, in the case of additional non-terminal feedback).
#[derive(Debug, Clone)]
pub struct ResumedGroup {
    /// DispatchGroup IRI.
    pub group_iri: String,
    /// Short wire string mirroring `ResumeOutcome::*`.
    pub outcome: String,
}

/// Errors from `dec feedback close`.
#[derive(Debug, thiserror::Error)]
pub enum CloseError {
    /// IRI doesn't parse.
    #[error("invalid feedback IRI: {0}")]
    InvalidFeedbackIri(String),
    /// Addressing-artifact IRI doesn't parse.
    #[error("invalid addressing-artifact IRI: {0}")]
    InvalidAddressingIri(String),
    /// Feedback artifact not present in the active stream.
    #[error("feedback not found: <{0}>")]
    NotFound(String),
    /// Feedback's class literal isn't one of the six ADR-023 values.
    #[error("feedback <{feedback}> has unknown class {class:?}")]
    UnknownClass {
        /// Offending feedback IRI.
        feedback: String,
        /// Class literal observed on the feedback.
        class: String,
    },
    /// Feedback isn't in the `addressed` state — close is invalid.
    #[error(
        "cannot close feedback <{feedback}>: state is {state}, expected `addressed` \
         (close transitions addressed → closed)"
    )]
    WrongState {
        /// IRI of the offending feedback.
        feedback: String,
        /// State observed at read time.
        state: String,
    },
    /// Addressing artifact does not exist in the store.
    #[error("addressing artifact <{0}> not found in the active store")]
    AddressingArtifactMissing(String),
    /// Addressing artifact belongs to a different stream.
    #[error(
        "addressing artifact <{artifact}> is scoped to <{found}>, \
         not the active stream <{active}>"
    )]
    ForeignStream {
        /// Addressing artifact IRI.
        artifact: String,
        /// Stream the artifact actually links to.
        found: String,
        /// Active stream IRI.
        active: String,
    },
    /// Addressing-artifact type not allowed by the routing table.
    #[error(
        "addressing artifact type <{artifact_type}> is not in the routing-table allowlist \
         for class `{class}` (allowed: {allowed})"
    )]
    IneligibleAddressingArtifact {
        /// Feedback class.
        class: String,
        /// Type IRI observed on the addressing artifact.
        artifact_type: String,
        /// Comma-joined display of the routing-table allowlist.
        allowed: String,
    },
    /// Wrapped lifecycle / writer / scope failure.
    #[error("{0}")]
    Other(String),
}

/// Apply a `dec feedback close` operation. Returns a structured outcome
/// describing the addressed feedback and any dispatch resumes that ran.
pub fn close(
    workdir: &Path,
    feedback_iri: &str,
    addressing_iri: &str,
    closed_by_identity: &str,
) -> Result<CloseOutcome, CloseError> {
    let (ws, fb_node, addr_node) = open_and_parse_inputs(workdir, feedback_iri, addressing_iri)?;
    let fb = load_feedback(&ws, &fb_node)?;
    ensure_addressed_state(&ws, &fb_node, feedback_iri)?;
    let class = parse_feedback_class(&fb, feedback_iri)?;
    validate_addressing_artifact(&ws, &addr_node, addressing_iri, class, &fb.class)?;
    let closed_by_node = mint_actor_iri(closed_by_identity, feedback_iri);
    apply_close_transition(&ws, &fb_node, &addr_node, &closed_by_node)?;
    let resumed_groups = run_resume_checks(&ws, &fb_node)?;
    ws.persist()
        .map_err(|e| CloseError::Other(format!("persisting store: {e:#}")))?;
    Ok(build_close_outcome(
        feedback_iri,
        addressing_iri,
        closed_by_identity,
        &closed_by_node,
        resumed_groups,
    ))
}

fn open_and_parse_inputs(
    workdir: &Path,
    feedback_iri: &str,
    addressing_iri: &str,
) -> Result<(WritableStore, NamedNode, NamedNode), CloseError> {
    let ws = WritableStore::open(workdir).map_err(|e| CloseError::Other(format!("{e:#}")))?;
    let fb_node = NamedNode::new(feedback_iri)
        .map_err(|_| CloseError::InvalidFeedbackIri(feedback_iri.to_string()))?;
    let addr_node = NamedNode::new(addressing_iri)
        .map_err(|_| CloseError::InvalidAddressingIri(addressing_iri.to_string()))?;
    Ok((ws, fb_node, addr_node))
}

fn load_feedback(
    ws: &WritableStore,
    fb_node: &NamedNode,
) -> Result<crate::core::feedback::Feedback, CloseError> {
    get(&ws.store, fb_node).map_err(|e| match e {
        crate::core::feedback::FeedbackReadError::NotFound { iri } => CloseError::NotFound(iri),
        other => CloseError::Other(format!("{other}")),
    })
}

fn ensure_addressed_state(
    ws: &WritableStore,
    fb_node: &NamedNode,
    feedback_iri: &str,
) -> Result<(), CloseError> {
    let prior =
        read_prior_state(&ws.store, fb_node).map_err(|e| CloseError::Other(format!("{e}")))?;
    if prior != LifecycleState::Addressed {
        return Err(CloseError::WrongState {
            feedback: feedback_iri.to_string(),
            state: prior.as_str().to_string(),
        });
    }
    Ok(())
}

fn parse_feedback_class(
    fb: &crate::core::feedback::Feedback,
    feedback_iri: &str,
) -> Result<FeedbackClass, CloseError> {
    FeedbackClass::from_iri_value(&fb.class).ok_or_else(|| CloseError::UnknownClass {
        feedback: feedback_iri.to_string(),
        class: fb.class.clone(),
    })
}

fn validate_addressing_artifact(
    ws: &WritableStore,
    addr_node: &NamedNode,
    addressing_iri: &str,
    class: FeedbackClass,
    class_literal: &str,
) -> Result<(), CloseError> {
    let active_stream = ws
        .active_stream()
        .map_err(|e| CloseError::Other(format!("{e:#}")))?;
    let (artifact_stream, artifact_types) = read_artifact_metadata(&ws.store, addr_node)
        .ok_or_else(|| CloseError::AddressingArtifactMissing(addressing_iri.to_string()))?;
    if let Some(stream) = artifact_stream.as_deref() {
        if stream != active_stream.as_str() {
            return Err(CloseError::ForeignStream {
                artifact: addressing_iri.to_string(),
                found: stream.to_string(),
                active: active_stream.as_str().to_string(),
            });
        }
    }
    validate_artifact_type_allowed(class, &artifact_types, class_literal)?;
    Ok(())
}

fn apply_close_transition(
    ws: &WritableStore,
    fb_node: &NamedNode,
    addr_node: &NamedNode,
    closed_by_node: &NamedNode,
) -> Result<(), CloseError> {
    let g = orchestration_graph().into_owned().into();
    let evidence: Vec<Quad> = build_close_evidence(fb_node, addr_node, closed_by_node, &g);
    apply_transition(
        &ws.store,
        &ws.writer,
        fb_node,
        LifecycleState::Closed,
        evidence,
        orchestration_graph(),
    )
    .map_err(|e| CloseError::Other(format!("applying close transition: {e}")))
}

fn build_close_outcome(
    feedback_iri: &str,
    addressing_iri: &str,
    closed_by_identity: &str,
    closed_by_node: &NamedNode,
    resumed_groups: Vec<ResumedGroup>,
) -> CloseOutcome {
    CloseOutcome {
        feedback_iri: feedback_iri.to_string(),
        addressing_iri: addressing_iri.to_string(),
        closed_by: closed_by_identity.to_string(),
        closed_by_iri: closed_by_node.as_str().to_string(),
        resumed_groups,
    }
}

/// Render a [`CloseOutcome`] as human-readable text.
#[must_use]
pub fn format_close(outcome: &CloseOutcome) -> String {
    let mut out = String::new();
    out.push_str(&format!("closed: {}\n", outcome.feedback_iri));
    out.push_str(&format!("addressing: {}\n", outcome.addressing_iri));
    out.push_str(&format!(
        "closed_by: {} (iri: {})\n",
        outcome.closed_by, outcome.closed_by_iri
    ));
    if outcome.resumed_groups.is_empty() {
        out.push_str("resumed_dispatches: (none)\n");
    } else {
        out.push_str("resumed_dispatches:\n");
        for r in &outcome.resumed_groups {
            out.push_str(&format!("  - {} → {}\n", r.group_iri, r.outcome));
        }
    }
    out
}

/// Render a [`CloseOutcome`] as JSON.
#[must_use]
pub fn format_close_json(outcome: &CloseOutcome) -> String {
    let mut out = String::from("{\n");
    append_scalar_fields(&mut out, outcome);
    append_resumed_dispatches(&mut out, &outcome.resumed_groups);
    out.push_str("}\n");
    out
}

fn append_scalar_fields(out: &mut String, outcome: &CloseOutcome) {
    out.push_str(&format!(
        "  \"feedback\": \"{}\",\n",
        json_escape(&outcome.feedback_iri)
    ));
    out.push_str(&format!(
        "  \"addressing_artifact\": \"{}\",\n",
        json_escape(&outcome.addressing_iri)
    ));
    out.push_str(&format!(
        "  \"closed_by\": {},\n",
        json_opt(Some(&outcome.closed_by))
    ));
    out.push_str(&format!(
        "  \"closed_by_iri\": {},\n",
        json_opt(Some(&outcome.closed_by_iri))
    ));
}

fn append_resumed_dispatches(out: &mut String, groups: &[ResumedGroup]) {
    out.push_str("  \"resumed_dispatches\": [");
    if groups.is_empty() {
        out.push_str("]\n");
        return;
    }
    out.push('\n');
    for (i, r) in groups.iter().enumerate() {
        out.push_str("    {");
        out.push_str(&format!("\"group\": \"{}\", ", json_escape(&r.group_iri)));
        out.push_str(&format!("\"outcome\": \"{}\"", json_escape(&r.outcome)));
        out.push('}');
        if i + 1 < groups.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
}

/// Convenience: convert a [`CloseError`] into an `anyhow` so the
/// CLI adapter can format it through the shared exit-code path.
pub fn close_anyhow(
    workdir: &Path,
    feedback_iri: &str,
    addressing_iri: &str,
    closed_by_identity: &str,
) -> Result<CloseOutcome> {
    close(workdir, feedback_iri, addressing_iri, closed_by_identity).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_close_renders_resumed_groups() {
        let outcome = CloseOutcome {
            feedback_iri: "urn:f:1".to_string(),
            addressing_iri: "urn:a:1".to_string(),
            closed_by: "user".to_string(),
            closed_by_iri: "urn:user:close:1".to_string(),
            resumed_groups: vec![ResumedGroup {
                group_iri: "urn:g:1".to_string(),
                outcome: "resumed".to_string(),
            }],
        };
        let out = format_close(&outcome);
        assert!(out.contains("closed: urn:f:1"));
        assert!(out.contains("urn:g:1"));
        assert!(out.contains("resumed"));
        assert!(out.contains("urn:user:close:1"));
    }

    #[test]
    fn format_close_json_emits_required_fields() {
        let outcome = CloseOutcome {
            feedback_iri: "urn:f:1".to_string(),
            addressing_iri: "urn:a:1".to_string(),
            closed_by: "user".to_string(),
            closed_by_iri: "urn:user:close:1".to_string(),
            resumed_groups: Vec::new(),
        };
        let out = format_close_json(&outcome);
        assert!(out.contains("\"feedback\": \"urn:f:1\""));
        assert!(out.contains("\"addressing_artifact\": \"urn:a:1\""));
        assert!(out.contains("\"closed_by\": \"user\""));
        assert!(out.contains("\"closed_by_iri\": \"urn:user:close:1\""));
        assert!(out.contains("\"resumed_dispatches\": []"));
    }
}
