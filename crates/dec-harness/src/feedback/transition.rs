//! Read-validate-write helper for feedback lifecycle transitions (FT-027).
//!
//! Callers in FT-029 (routing handler), FT-031 (worker harness), and
//! FT-033 (CLI close) all need the same dance: read the prior lifecycle
//! state from the orchestration store, call
//! [`super::lifecycle::validate_transition`], and emit a mutation that
//! removes the prior literal and inserts the new one (plus any
//! state-specific companion fields). This module wraps that dance so
//! every consumer hits the same chokepoint.

use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;
use thiserror::Error;

use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::{lifecycle_state, IRI_DEC_LIFECYCLE_STATE};

use super::lifecycle::{validate_transition, LifecycleState, TransitionError};

/// Evidence quads attached to the target state (e.g. `dec:routedAt`,
/// `dec:addressingArtifact`). These are inserted alongside the new
/// `dec:lifecycleState` literal in the same mutation.
pub type Evidence = Vec<Quad>;

/// Errors surfaced by [`apply`].
#[derive(Debug, Error)]
pub enum ApplyError {
    /// The feedback IRI has no `dec:lifecycleState` literal in the store.
    #[error("no prior dec:lifecycleState found for <{iri}>")]
    NoPriorState {
        /// The feedback IRI.
        iri: String,
    },
    /// The stored prior-state literal does not match any known lifecycle state.
    #[error("stored dec:lifecycleState for <{iri}> is unknown: {value:?}")]
    UnknownStoredState {
        /// The feedback IRI.
        iri: String,
        /// The unrecognised wire-string value.
        value: String,
    },
    /// The validator refused the transition.
    #[error(transparent)]
    Transition(#[from] TransitionError),
    /// Underlying store / writer failure.
    #[error("store error: {0}")]
    Store(String),
}

/// Apply a lifecycle transition for `feedback_iri` by reading the prior
/// state, validating it against [`validate_transition`], and committing
/// a mutation through `writer` that swaps the literal and attaches
/// `evidence` quads.
///
/// Callers are responsible for supplying the companion fields required
/// by the target state (e.g. `dec:routedAt` + `dec:targetRole` for
/// `routed`, `dec:addressingArtifact` for `addressed`). The SHACL shape
/// in `shapes.ttl` enforces those companion-field constraints at write
/// time, so a missing field produces a write-side error rather than
/// silently committing a malformed state.
pub fn apply(
    store: &Store,
    writer: &StreamWriter,
    feedback_iri: &NamedNode,
    new_state: LifecycleState,
    evidence: Evidence,
    graph: NamedNodeRef<'_>,
) -> Result<(), ApplyError> {
    let prior = read_prior_state(store, feedback_iri)?;
    validate_transition(prior, new_state)?;
    let g: GraphName = graph.into_owned().into();
    let removes = collect_prior_state_quads(store, feedback_iri);
    let inserts = build_state_inserts(feedback_iri, new_state, evidence, g);
    let mutation = Mutation {
        inserts,
        removes,
        ..Mutation::default()
    };
    writer
        .commit(mutation)
        .map_err(|e| ApplyError::Store(format!("{e:#}")))?;
    Ok(())
}

// Remove every prior dec:lifecycleState literal for this subject, in
// whatever graph it lives. The writer's middleware re-tags scoped
// artifacts; the prior-state literal is the only thing we need to
// explicitly retract.
fn collect_prior_state_quads(store: &Store, feedback_iri: &NamedNode) -> Vec<Quad> {
    let lifecycle_pred = lifecycle_state();
    store
        .quads_for_pattern(
            Some(Subject::NamedNode(feedback_iri.clone()).as_ref()),
            Some(lifecycle_pred),
            None,
            None,
        )
        .filter_map(Result::ok)
        .collect()
}

fn build_state_inserts(
    feedback_iri: &NamedNode,
    new_state: LifecycleState,
    evidence: Evidence,
    g: GraphName,
) -> Vec<Quad> {
    let lifecycle_pred = lifecycle_state();
    let mut inserts: Vec<Quad> = Vec::with_capacity(1 + evidence.len());
    inserts.push(Quad::new(
        feedback_iri.clone(),
        lifecycle_pred.into_owned(),
        Literal::new_simple_literal(new_state.as_str()),
        g,
    ));
    inserts.extend(evidence);
    inserts
}

/// Read the current `dec:lifecycleState` literal for `feedback_iri` from
/// the store. Returns `NoPriorState` if no literal is present, or
/// `UnknownStoredState` if the stored value is not a recognised state.
pub fn read_prior_state(
    store: &Store,
    feedback_iri: &NamedNode,
) -> Result<LifecycleState, ApplyError> {
    let lifecycle_pred = lifecycle_state();
    let mut found: Option<String> = None;
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(feedback_iri.clone()).as_ref()),
            Some(lifecycle_pred),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = quad.object {
            found = Some(lit.value().to_string());
            break;
        }
    }
    let raw = found.ok_or_else(|| ApplyError::NoPriorState {
        iri: feedback_iri.as_str().to_string(),
    })?;
    LifecycleState::parse(&raw).ok_or(ApplyError::UnknownStoredState {
        iri: feedback_iri.as_str().to_string(),
        value: raw,
    })
}

/// Predicate IRI for `dec:lifecycleState` — re-exported for callers that
/// build their own mutations rather than going through [`apply`].
pub const IRI_DEC_LIFECYCLE_STATE_PRED: &str = IRI_DEC_LIFECYCLE_STATE;
