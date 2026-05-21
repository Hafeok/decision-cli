//! `dec feedback receive <iri>` (FT-033).
//!
//! Manual `routed → received` transition. Phase A surface for the
//! "human as target role" pattern: when the human operator is the
//! pending spec-author / architect / slice-curator (no role worker
//! exists yet), `dec feedback receive` is the acknowledgement that
//! they're working on the feedback.
//!
//! The command refuses anything that is not in `routed` — the lifecycle
//! state machine in FT-027 owns the actual transition validation;
//! this module wraps it in a CLI-friendly outcome.

use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::Utc;
use oxigraph::model::{NamedNode, Quad};

use crate::core::feedback::transition::{apply as apply_transition, read_prior_state};
use crate::core::feedback::{get, LifecycleState};
use crate::core::vocab::{orchestration_graph, receiving_session};

use super::format::{json_escape, json_opt};
use super::store_io::WritableStore;

/// Successful outcome of `dec feedback receive`.
#[derive(Debug, Clone)]
pub struct ReceiveOutcome {
    /// Feedback IRI now in `received`.
    pub feedback_iri: String,
    /// Identity recorded via the synthetic receiving-session URN
    /// (Phase A puts the human's id straight into the IRI fragment).
    pub receiving_session: String,
}

/// Errors from `dec feedback receive`.
#[derive(Debug, thiserror::Error)]
pub enum ReceiveError {
    /// Feedback IRI doesn't parse.
    #[error("invalid feedback IRI: {0}")]
    InvalidIri(String),
    /// No feedback artifact at the given IRI.
    #[error("feedback not found: <{0}>")]
    NotFound(String),
    /// Feedback isn't in `routed` — receive is invalid.
    #[error(
        "cannot receive feedback <{feedback}>: state is {state}, expected `routed` \
         (receive transitions routed → received)"
    )]
    WrongState {
        /// Feedback IRI.
        feedback: String,
        /// Current state.
        state: String,
    },
    /// Wrapped lifecycle / writer / scope failure.
    #[error("{0}")]
    Other(String),
}

/// Run `dec feedback receive`. Returns the structured outcome on success.
pub fn receive(
    workdir: &Path,
    feedback_iri: &str,
    actor: &str,
) -> Result<ReceiveOutcome, ReceiveError> {
    let ws = WritableStore::open(workdir).map_err(|e| ReceiveError::Other(format!("{e:#}")))?;
    let fb_node = NamedNode::new(feedback_iri)
        .map_err(|_| ReceiveError::InvalidIri(feedback_iri.to_string()))?;
    ensure_feedback_loadable(&ws, &fb_node)?;
    ensure_routed_state(&ws, &fb_node, feedback_iri)?;
    let receiving_session_iri = mint_human_session_iri(feedback_iri, actor);
    let receiving_node = parse_receiving_session(&receiving_session_iri)?;
    let evidence = build_receive_evidence(&fb_node, receiving_node);
    apply_receive_transition(&ws, &fb_node, evidence)?;
    ws.persist()
        .map_err(|e| ReceiveError::Other(format!("persisting store: {e:#}")))?;
    Ok(ReceiveOutcome {
        feedback_iri: feedback_iri.to_string(),
        receiving_session: receiving_session_iri,
    })
}

fn ensure_feedback_loadable(ws: &WritableStore, fb_node: &NamedNode) -> Result<(), ReceiveError> {
    get(&ws.store, fb_node)
        .map(|_| ())
        .map_err(|e| match e {
            crate::core::feedback::FeedbackReadError::NotFound { iri } => {
                ReceiveError::NotFound(iri)
            }
            other => ReceiveError::Other(format!("{other}")),
        })
}

fn ensure_routed_state(
    ws: &WritableStore,
    fb_node: &NamedNode,
    feedback_iri: &str,
) -> Result<(), ReceiveError> {
    let prior =
        read_prior_state(&ws.store, fb_node).map_err(|e| ReceiveError::Other(format!("{e}")))?;
    if prior != LifecycleState::Routed {
        return Err(ReceiveError::WrongState {
            feedback: feedback_iri.to_string(),
            state: prior.as_str().to_string(),
        });
    }
    Ok(())
}

fn parse_receiving_session(iri: &str) -> Result<NamedNode, ReceiveError> {
    NamedNode::new(iri)
        .map_err(|e| ReceiveError::Other(format!("minting receiving-session IRI {iri}: {e}")))
}

fn build_receive_evidence(fb_node: &NamedNode, receiving_node: NamedNode) -> Vec<Quad> {
    vec![Quad::new(
        fb_node.clone(),
        receiving_session().into_owned(),
        receiving_node,
        orchestration_graph().into_owned(),
    )]
}

fn apply_receive_transition(
    ws: &WritableStore,
    fb_node: &NamedNode,
    evidence: Vec<Quad>,
) -> Result<(), ReceiveError> {
    apply_transition(
        &ws.store,
        &ws.writer,
        fb_node,
        LifecycleState::Received,
        evidence,
        orchestration_graph(),
    )
    .map_err(|e| ReceiveError::Other(format!("applying receive transition: {e}")))
}

/// Build a deterministic-ish IRI for a human-driven receive event so
/// the audit trail records who acked what, when. Phase A uses an
/// RFC3339 timestamp + actor + feedback-IRI hash; no schema cost vs.
/// the worker-minted session IRIs.
fn mint_human_session_iri(feedback_iri: &str, actor: &str) -> String {
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
    let safe_actor: String = actor
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let safe_actor = safe_actor.trim_matches('-').to_string();
    let hash_short = feedback_iri_hash(feedback_iri);
    format!(
        "https://decision-cli.dev/ns/session/human-receive-{ts}-{actor}-{hash}",
        ts = ts,
        actor = if safe_actor.is_empty() {
            "anonymous"
        } else {
            safe_actor.as_str()
        },
        hash = hash_short,
    )
}

fn feedback_iri_hash(iri: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(iri.as_bytes());
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    hex
}

/// Render the outcome as human text.
#[must_use]
pub fn format_receive(outcome: &ReceiveOutcome) -> String {
    format!(
        "received: {fb}\n  receiving_session: {sess}\n",
        fb = outcome.feedback_iri,
        sess = outcome.receiving_session,
    )
}

/// Render the outcome as JSON.
#[must_use]
pub fn format_receive_json(outcome: &ReceiveOutcome) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"feedback\": \"{}\",\n",
        json_escape(&outcome.feedback_iri)
    ));
    out.push_str(&format!(
        "  \"receiving_session\": {}\n",
        json_opt(Some(&outcome.receiving_session))
    ));
    out.push_str("}\n");
    out
}

/// `anyhow`-wrapped wrapper for the CLI adapter.
pub fn receive_anyhow(
    workdir: &Path,
    feedback_iri: &str,
    actor: &str,
) -> Result<ReceiveOutcome> {
    receive(workdir, feedback_iri, actor).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_human_session_iri_includes_actor_and_hash() {
        let iri = mint_human_session_iri("urn:f:1", "alice");
        assert!(iri.starts_with("https://decision-cli.dev/ns/session/human-receive-"));
        assert!(iri.contains("-alice-"));
        // Hash short is 12 hex chars.
        assert!(iri.split('-').last().unwrap().len() == 12);
    }

    #[test]
    fn mint_handles_anonymous_actor() {
        let iri = mint_human_session_iri("urn:f:1", "");
        assert!(iri.contains("-anonymous-"));
    }

    #[test]
    fn format_receive_emits_kv_lines() {
        let outcome = ReceiveOutcome {
            feedback_iri: "urn:f:1".to_string(),
            receiving_session: "urn:s:received-1".to_string(),
        };
        let out = format_receive(&outcome);
        assert!(out.contains("received: urn:f:1"));
        assert!(out.contains("receiving_session: urn:s:received-1"));
    }

    #[test]
    fn format_receive_json_emits_record() {
        let outcome = ReceiveOutcome {
            feedback_iri: "urn:f:1".to_string(),
            receiving_session: "urn:s:received-1".to_string(),
        };
        let out = format_receive_json(&outcome);
        assert!(out.contains("\"feedback\": \"urn:f:1\""));
        assert!(out.contains("\"receiving_session\": \"urn:s:received-1\""));
    }
}
