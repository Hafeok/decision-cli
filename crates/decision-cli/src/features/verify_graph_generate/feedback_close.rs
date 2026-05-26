//! FT-107 — lifecycle transition helper used by the accept handler to
//! mark each defect feedback the worker addressed as `addressed`.
//!
//! The lifecycle state machine ([`ADR-024`](ADR-024) / FT-027) only
//! permits `addressed` to be reached via `produced → routed → received →
//! addressed`. The verify-graph-author dispatch is itself the routing
//! event (the orchestrator hands the worker the bundle that carries the
//! feedback) AND the receiving event (the worker reads and acts on it).
//! Rather than introducing a fast-track edge, this helper walks the
//! three transitions in order, attaching the companion fields each
//! state requires.

use std::sync::Arc;

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::{apply as apply_transition, ApplyError};
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    addressing_artifact, orchestration_graph, routed_at, target_role,
};

/// Errors surfaced by [`mark_addressed`].
#[derive(Debug, Error)]
pub enum FeedbackCloseError {
    /// The feedback IRI string failed to parse as a NamedNode.
    #[error("invalid feedback IRI {iri:?}: {detail}")]
    InvalidIri {
        /// Wire-string the caller passed.
        iri: String,
        /// Parse error.
        detail: String,
    },
    /// Lifecycle transition refused (e.g. the feedback was already
    /// terminal, or the prior-state was missing).
    #[error(transparent)]
    Transition(#[from] ApplyError),
}

/// Transition `feedback_iri` from whatever current state to `addressed`,
/// attaching `addressing_artifact_iri` as the `dec:addressingArtifact`.
///
/// `target_role_id` and `now_rfc3339` are used as the companion fields
/// for the intermediate `routed` state. `session_iri` is the session
/// IRI that "received" the feedback (the dispatch activity).
///
/// Idempotent w.r.t. terminal states: if the feedback is already
/// `addressed`/`closed`/`rejected`/`superseded` the helper returns
/// `Ok(())` without commit.
pub fn mark_addressed(
    store: &Store,
    writer: &StreamWriter,
    feedback_iri: &str,
    addressing_artifact_iri: &NamedNode,
    target_role_id: &str,
    session_iri: &NamedNode,
    now_rfc3339: &str,
) -> Result<(), FeedbackCloseError> {
    let fb_node = NamedNode::new(feedback_iri).map_err(|e| FeedbackCloseError::InvalidIri {
        iri: feedback_iri.to_string(),
        detail: e.to_string(),
    })?;
    let prior = match crate::core::feedback::transition::read_prior_state(store, &fb_node) {
        Ok(state) => state,
        Err(e) => return Err(FeedbackCloseError::Transition(e)),
    };
    if matches!(
        prior,
        LifecycleState::Addressed
            | LifecycleState::Closed
            | LifecycleState::Rejected
            | LifecycleState::Superseded
    ) {
        return Ok(());
    }
    let graph = orchestration_graph();
    let g: GraphName = graph.into_owned().into();
    // Walk produced → routed → received → addressed, skipping states we
    // are already past.
    let mut cur = prior;
    if matches!(cur, LifecycleState::Produced) {
        // The runner's feedback emitter (core::verify::runner::feedback)
        // already stamps `dec:targetRole` at the class default — see
        // FeedbackClass::default_target_role. SHACL requires both
        // `dec:routedAt` AND `dec:targetRole` on the routed state, so the
        // prior literal already satisfies the second half; we only attach
        // `routedAt` here and let the existing targetRole stand. (Adding
        // a second targetRole would violate the cardinality-one SHACL
        // shape.) `target_role_id` is therefore advisory in the routed
        // step and the parameter is retained on the signature for future
        // explicit-override paths.
        let _ = target_role();
        let _ = target_role_id;
        let routed_evidence = vec![Quad::new(
            fb_node.clone(),
            routed_at().into_owned(),
            Literal::new_simple_literal(now_rfc3339),
            g.clone(),
        )];
        apply_transition(
            store,
            writer,
            &fb_node,
            LifecycleState::Routed,
            routed_evidence,
            graph,
        )?;
        cur = LifecycleState::Routed;
    }
    if matches!(cur, LifecycleState::Routed) {
        let received_evidence = vec![Quad::new(
            fb_node.clone(),
            crate::core::vocab::receiving_session().into_owned(),
            session_iri.clone(),
            g.clone(),
        )];
        apply_transition(
            store,
            writer,
            &fb_node,
            LifecycleState::Received,
            received_evidence,
            graph,
        )?;
        cur = LifecycleState::Received;
    }
    if matches!(cur, LifecycleState::Received) {
        let addressed_evidence = vec![Quad::new(
            fb_node.clone(),
            addressing_artifact().into_owned(),
            addressing_artifact_iri.clone(),
            g.clone(),
        )];
        apply_transition(
            store,
            writer,
            &fb_node,
            LifecycleState::Addressed,
            addressed_evidence,
            graph,
        )?;
    }
    Ok(())
}

/// Convenience wrapper for the accept path: open a fresh store + writer,
/// transition every feedback in `feedback_iris`, persist. Returns the
/// number of feedback artifacts actually transitioned to `addressed`.
pub fn mark_batch_addressed(
    workdir: &std::path::Path,
    feedback_iris: &[String],
    addressing_artifact_iri: &NamedNode,
    target_role_id: &str,
    session_iri: &NamedNode,
    now_rfc3339: &str,
) -> Result<usize, FeedbackCloseError> {
    if feedback_iris.is_empty() {
        return Ok(0);
    }
    let dump = crate::core::store::orchestration_dump_path(workdir);
    let store = crate::core::store::load_store_from_dump(&dump).map_err(|e| {
        FeedbackCloseError::Transition(ApplyError::Store(format!("loading store: {e:#}")))
    })?;
    let store = Arc::new(store);
    let scope = crate::core::scope::ActiveScope::load(workdir).map_err(|e| {
        FeedbackCloseError::Transition(ApplyError::Store(format!("loading active scope: {e}")))
    })?;
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| {
        FeedbackCloseError::Transition(ApplyError::Store(format!("active stream iri: {e}")))
    })?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| {
        FeedbackCloseError::Transition(ApplyError::Store(format!("opening writer: {e:#}")))
    })?;
    let mut n = 0;
    for iri in feedback_iris {
        mark_addressed(
            &store,
            &writer,
            iri,
            addressing_artifact_iri,
            target_role_id,
            session_iri,
            now_rfc3339,
        )?;
        n += 1;
    }
    crate::core::store::persist_store(&store, &dump).map_err(|e| {
        FeedbackCloseError::Transition(ApplyError::Store(format!("persisting store: {e:#}")))
    })?;
    Ok(n)
}
