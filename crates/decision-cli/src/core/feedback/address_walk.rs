//! Generic helper to transition feedback through the ADR-024 lifecycle
//! to the `addressed` terminal-adjacent state. Used by both the
//! verify-graph-author accept path (FT-107) and the code-writer accept
//! path (FT-108).
//!
//! The lifecycle state machine ([`ADR-024`](ADR-024) / FT-027) only
//! permits `addressed` to be reached via
//! `produced → routed → received → addressed`. A dispatch that hands a
//! worker an addressing artifact is itself the routing event AND the
//! receiving event (the worker reads the bundle and acts on it), so
//! this helper walks the three transitions in sequence, attaching the
//! companion fields each state requires.

use std::sync::Arc;

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::{apply as apply_transition, ApplyError};
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{addressing_artifact, orchestration_graph, routed_at};

/// Errors surfaced by [`mark_addressed`] / [`mark_batch_addressed`].
#[derive(Debug, Error)]
pub enum AddressWalkError {
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
/// `session_iri` is the dispatch session that "received" the feedback.
/// `now_rfc3339` is stamped as `dec:routedAt` on the intermediate
/// routed transition.
///
/// Idempotent w.r.t. terminal states: if the feedback is already
/// `addressed`/`closed`/`rejected`/`superseded` the helper returns
/// `Ok(())` without commit.
pub fn mark_addressed(
    store: &Store,
    writer: &StreamWriter,
    feedback_iri: &str,
    addressing_artifact_iri: &NamedNode,
    session_iri: &NamedNode,
    now_rfc3339: &str,
) -> Result<(), AddressWalkError> {
    let fb_node = NamedNode::new(feedback_iri).map_err(|e| AddressWalkError::InvalidIri {
        iri: feedback_iri.to_string(),
        detail: e.to_string(),
    })?;
    let prior = match crate::core::feedback::transition::read_prior_state(store, &fb_node) {
        Ok(state) => state,
        Err(e) => return Err(AddressWalkError::Transition(e)),
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
    let mut cur = prior;
    if matches!(cur, LifecycleState::Produced) {
        // The runner's feedback emitter already stamps `dec:targetRole`
        // when the feedback is born; SHACL requires it on the routed
        // state too, but the existing literal satisfies the cardinality.
        // Adding a second `targetRole` would violate the SHACL shape, so
        // we only attach `routedAt` here.
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

/// Convenience wrapper: open a fresh store + writer, transition every
/// feedback in `feedback_iris`, persist. Returns the number of feedback
/// artifacts actually transitioned to `addressed`.
pub fn mark_batch_addressed(
    workdir: &std::path::Path,
    feedback_iris: &[String],
    addressing_artifact_iri: &NamedNode,
    session_iri: &NamedNode,
    now_rfc3339: &str,
) -> Result<usize, AddressWalkError> {
    if feedback_iris.is_empty() {
        return Ok(0);
    }
    let dump = crate::core::store::orchestration_dump_path(workdir);
    let store = crate::core::store::load_store_from_dump(&dump).map_err(|e| {
        AddressWalkError::Transition(ApplyError::Store(format!("loading store: {e:#}")))
    })?;
    let store = Arc::new(store);
    let scope = crate::core::scope::ActiveScope::load(workdir).map_err(|e| {
        AddressWalkError::Transition(ApplyError::Store(format!("loading active scope: {e}")))
    })?;
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| {
        AddressWalkError::Transition(ApplyError::Store(format!("active stream iri: {e}")))
    })?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| {
        AddressWalkError::Transition(ApplyError::Store(format!("opening writer: {e:#}")))
    })?;
    let mut n = 0;
    for iri in feedback_iris {
        mark_addressed(
            &store,
            &writer,
            iri,
            addressing_artifact_iri,
            session_iri,
            now_rfc3339,
        )?;
        n += 1;
    }
    crate::core::store::persist_store(&store, &dump).map_err(|e| {
        AddressWalkError::Transition(ApplyError::Store(format!("persisting store: {e:#}")))
    })?;
    Ok(n)
}
