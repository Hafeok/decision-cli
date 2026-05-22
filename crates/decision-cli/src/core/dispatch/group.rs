//! In-memory `DispatchGroup` shape + commit helpers (FT-021 / ADR-017).

use anyhow::Context;
use oxi_events::Mutation;
use oxigraph::model::{Literal, NamedNode, Subject, Term};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{IRI_DEC_DISPATCH_GROUP, IRI_DEC_DISPATCH_STATUS};

use super::lifecycle::{next as next_status, DispatchEvent, DispatchStatus, LifecycleError};
use super::quads::{
    build_group_creation_quads, build_interpretation_session_link_quad,
    build_status_transition_quads,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// In-memory view over a `dec:DispatchGroup` artifact.
///
/// Owners of a `DispatchGroup` use the typed helpers
/// ([`Self::transition`], [`Self::attach_interpretation_session`]) so
/// the lifecycle machinery and the persisted store stay in step.
#[derive(Debug, Clone)]
pub struct DispatchGroup {
    /// Stable group IRI in the orchestration graph.
    pub iri: NamedNode,
    /// Feature id (e.g. `"FT-021"`) this group is acting on.
    pub feature_id: String,
    /// Action session paired by this group.
    pub action_session: NamedNode,
    /// Interpretation session, present once the verifier dispatch is minted.
    pub interpretation_session: Option<NamedNode>,
    /// Current lifecycle state.
    pub status: DispatchStatus,
}

/// Errors produced when committing DispatchGroup mutations.
#[derive(Debug, Error)]
pub enum GroupError {
    /// State machine refused the requested transition.
    #[error("lifecycle error: {0}")]
    Lifecycle(#[from] LifecycleError),

    /// Underlying store / writer failure.
    #[error("dispatch group commit failed: {0}")]
    Commit(String),
}

impl DispatchGroup {
    /// Mint a new `DispatchGroup` in the orchestration store and return
    /// the in-memory handle.
    ///
    /// Initial status is [`DispatchStatus::AwaitingAction`]. The group's
    /// `dec:inStream` link is added by [`StreamWriter::commit`].
    pub fn mint(
        writer: &StreamWriter,
        group_iri: NamedNode,
        action_session: NamedNode,
        feature_id: impl Into<String>,
    ) -> Result<Self, GroupError> {
        let feature_id = feature_id.into();
        let status = DispatchStatus::AwaitingAction;
        let quads = build_group_creation_quads(&group_iri, &action_session, &feature_id, status);
        let mut mutation = Mutation::insert(quads.iter().cloned());
        mutation = mutation.with_cause(format!("FT-021 mint DispatchGroup {feature_id}"));
        writer
            .commit(mutation)
            .map_err(|e| GroupError::Commit(format!("{e:#}")))?;
        Ok(Self {
            iri: group_iri,
            feature_id,
            action_session,
            interpretation_session: None,
            status,
        })
    }

    /// Drive the lifecycle by one event and persist the resulting
    /// `dec:dispatchStatus` transition.
    pub fn transition(
        &mut self,
        writer: &StreamWriter,
        event: DispatchEvent,
    ) -> Result<DispatchStatus, GroupError> {
        let next = next_status(self.status, event)?;
        if next == self.status {
            return Ok(self.status);
        }
        let (insert, delete) = build_status_transition_quads(&self.iri, self.status, next);
        let mut mutation = Mutation::default();
        mutation.removes.push(delete);
        mutation.inserts.push(insert);
        mutation = mutation.with_cause(format!(
            "FT-021 DispatchGroup {feature} transition {from} -> {to}",
            feature = self.feature_id,
            from = self.status.as_str(),
            to = next.as_str(),
        ));
        writer
            .commit(mutation)
            .map_err(|e| GroupError::Commit(format!("{e:#}")))?;
        self.status = next;
        Ok(next)
    }

    /// Record the paired `dec:InterpretationSession` once the verifier
    /// dispatch is minted. Idempotent for the same session IRI.
    pub fn attach_interpretation_session(
        &mut self,
        writer: &StreamWriter,
        interpretation_session: NamedNode,
    ) -> Result<(), GroupError> {
        if self.interpretation_session.as_ref() == Some(&interpretation_session) {
            return Ok(());
        }
        let quad = build_interpretation_session_link_quad(&self.iri, &interpretation_session);
        let mut mutation = Mutation::default();
        mutation.inserts.push(quad);
        mutation = mutation.with_cause(format!(
            "FT-021 DispatchGroup {feature} attach interpretation session",
            feature = self.feature_id,
        ));
        writer
            .commit(mutation)
            .map_err(|e| GroupError::Commit(format!("{e:#}")))?;
        self.interpretation_session = Some(interpretation_session);
        Ok(())
    }

    /// Read a `DispatchGroup` from the orchestration store, if present.
    /// Returns `None` when the IRI is unknown.
    pub fn read(store: &Store, group_iri: &NamedNode) -> Result<Option<Self>, anyhow::Error> {
        let mut acc = GroupAccumulator::default();
        for q in store.quads_for_pattern(
            Some(Subject::NamedNode(group_iri.clone()).as_ref()),
            None,
            None,
            None,
        ) {
            let q = q.context("scanning DispatchGroup quads")?;
            acc.absorb(&q);
        }
        if !acc.typed {
            return Ok(None);
        }
        Ok(Some(Self {
            iri: group_iri.clone(),
            feature_id: acc.feature_id.unwrap_or_default(),
            action_session: acc.action_session.unwrap_or_else(|| group_iri.clone()),
            interpretation_session: acc.interpretation_session,
            status: acc.status.unwrap_or(DispatchStatus::AwaitingAction),
        }))
    }

    /// Convenience: literal IRI string.
    #[must_use]
    pub fn iri_str(&self) -> &str {
        self.iri.as_str()
    }
}

/// Helper kept here so callers in `features/*` can match the wire shape
/// without importing oxigraph term types directly.
#[must_use]
pub fn status_literal(status: DispatchStatus) -> Literal {
    Literal::new_simple_literal(status.as_str())
}

const IRI_HAS_ACTION: &str = "https://decision-cli.dev/ns#hasActionSession";
const IRI_HAS_INTERP: &str = "https://decision-cli.dev/ns#hasInterpretationSession";
const IRI_DISPATCHED_FOR: &str = "https://decision-cli.dev/ns#dispatchedFor";

#[derive(Default)]
struct GroupAccumulator {
    typed: bool,
    status: Option<DispatchStatus>,
    feature_id: Option<String>,
    action_session: Option<NamedNode>,
    interpretation_session: Option<NamedNode>,
}

impl GroupAccumulator {
    fn absorb(&mut self, q: &oxigraph::model::Quad) {
        match q.predicate.as_str() {
            RDF_TYPE => self.absorb_type(&q.object),
            IRI_DEC_DISPATCH_STATUS => self.absorb_status(&q.object),
            IRI_DISPATCHED_FOR => self.absorb_feature_id(&q.object),
            IRI_HAS_ACTION => self.absorb_action(&q.object),
            IRI_HAS_INTERP => self.absorb_interpretation(&q.object),
            _ => {}
        }
    }

    fn absorb_type(&mut self, object: &Term) {
        if let Term::NamedNode(n) = object {
            if n.as_str() == IRI_DEC_DISPATCH_GROUP {
                self.typed = true;
            }
        }
    }

    fn absorb_status(&mut self, object: &Term) {
        if let Term::Literal(lit) = object {
            self.status = DispatchStatus::parse(lit.value());
        }
    }

    fn absorb_feature_id(&mut self, object: &Term) {
        if let Term::Literal(lit) = object {
            self.feature_id = Some(lit.value().to_string());
        }
    }

    fn absorb_action(&mut self, object: &Term) {
        if let Term::NamedNode(n) = object {
            self.action_session = Some(n.clone());
        }
    }

    fn absorb_interpretation(&mut self, object: &Term) {
        if let Term::NamedNode(n) = object {
            self.interpretation_session = Some(n.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use oxigraph::store::Store;

    use super::*;
    use crate::core::stream_writer::StreamWriter;
    use crate::core::vocab::IRI_DEC_DISPATCH_STATUS;

    const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";

    fn writer() -> (Arc<Store>, StreamWriter) {
        let store = Arc::new(Store::new().expect("in-memory store"));
        let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
        let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
        (store, w)
    }

    #[test]
    fn mint_and_transition_through_complete() {
        let (store, w) = writer();
        let group_iri = NamedNode::new("urn:dec:group:1").expect("group iri");
        let action = NamedNode::new("urn:dec:session/action/1").expect("action iri");
        let mut g = DispatchGroup::mint(&w, group_iri.clone(), action, "FT-021").expect("mint");
        assert_eq!(g.status, DispatchStatus::AwaitingAction);

        g.transition(&w, DispatchEvent::ActionCompleted)
            .expect("ok");
        assert_eq!(g.status, DispatchStatus::AwaitingInterpretation);

        g.transition(&w, DispatchEvent::VerdictApproved)
            .expect("approved");
        assert_eq!(g.status, DispatchStatus::Complete);

        // Persisted status literal is exactly `complete`, no leftover
        // `awaiting-*` literal (SHACL maxCount 1).
        let mut found: Vec<String> = Vec::new();
        for q in store.quads_for_pattern(
            Some(Subject::NamedNode(group_iri.clone()).as_ref()),
            None,
            None,
            None,
        ) {
            let q = q.expect("scan");
            if q.predicate.as_str() == IRI_DEC_DISPATCH_STATUS {
                if let Term::Literal(lit) = q.object {
                    found.push(lit.value().to_string());
                }
            }
        }
        assert_eq!(found, vec!["complete".to_string()]);
    }

    #[test]
    fn rejected_lands_on_interpretation_rejected() {
        let (_, w) = writer();
        let group_iri = NamedNode::new("urn:dec:group:2").expect("group iri");
        let action = NamedNode::new("urn:dec:session/action/2").expect("action iri");
        let mut g = DispatchGroup::mint(&w, group_iri, action, "FT-021").expect("mint");
        g.transition(&w, DispatchEvent::ActionCompleted)
            .expect("ok");
        g.transition(&w, DispatchEvent::VerdictRejected)
            .expect("reject");
        assert_eq!(g.status, DispatchStatus::InterpretationRejected);
        // Terminal: another transition refused.
        let err = g
            .transition(&w, DispatchEvent::VerdictApproved)
            .expect_err("terminal");
        assert!(matches!(err, GroupError::Lifecycle(_)));
    }
}
