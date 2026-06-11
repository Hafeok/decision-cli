//! Quad builders for `dec:DispatchGroup` mutations (FT-021 / ADR-017).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use dec_ontology::vocab::{
    blocked_by, dispatch_group_class, dispatch_status, dispatched_for, has_action_session,
    has_interpretation_session, orchestration_graph, IRI_DEC_DISPATCH_STATUS,
};

use super::lifecycle::DispatchStatus;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Build the initial quad set for a freshly minted `dec:DispatchGroup`.
///
/// The caller is responsible for committing via [`crate::StreamWriter`];
/// that path adds the `dec:inStream` link required by ADR-005 + the
/// SHACL shape.
#[must_use]
pub fn build_group_creation_quads(
    group_iri: &NamedNode,
    action_session: &NamedNode,
    feature_id: &str,
    initial_status: DispatchStatus,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let group_cls = dispatch_group_class().into_owned();
    let has_action = has_action_session().into_owned();
    let dispatched_pred = dispatched_for().into_owned();
    let status_pred = dispatch_status().into_owned();
    vec![
        Quad::new(group_iri.clone(), rdf_type, group_cls, g.clone()),
        Quad::new(
            group_iri.clone(),
            has_action,
            action_session.clone(),
            g.clone(),
        ),
        Quad::new(
            group_iri.clone(),
            dispatched_pred,
            Literal::new_simple_literal(feature_id),
            g.clone(),
        ),
        Quad::new(
            group_iri.clone(),
            status_pred,
            Literal::new_simple_literal(initial_status.as_str()),
            g,
        ),
    ]
}

/// Build the `(insert, delete)` quad pair for advancing the
/// `dec:dispatchStatus` literal on an existing group.
///
/// The delete quad must be the prior status literal, exactly as
/// persisted; otherwise oxigraph's transaction will skip the deletion
/// and the group will end up with two status literals (which the SHACL
/// `sh:maxCount 1` would then refuse on the next read-side validation).
#[must_use]
pub fn build_status_transition_quads(
    group_iri: &NamedNode,
    previous: DispatchStatus,
    next_status: DispatchStatus,
) -> (Quad, Quad) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let status_pred = NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH_STATUS).into_owned();
    let insert = Quad::new(
        group_iri.clone(),
        status_pred.clone(),
        Literal::new_simple_literal(next_status.as_str()),
        g.clone(),
    );
    let delete = Quad::new(
        group_iri.clone(),
        status_pred,
        Literal::new_simple_literal(previous.as_str()),
        g,
    );
    (insert, delete)
}

/// Build a `dec:hasInterpretationSession` quad linking the group to a
/// minted interpretation session (slice-2 wiring; consumed by FT-022).
#[must_use]
pub fn build_interpretation_session_link_quad(
    group_iri: &NamedNode,
    interpretation_session: &NamedNode,
) -> Quad {
    let g: GraphName = orchestration_graph().into_owned().into();
    Quad::new(
        group_iri.clone(),
        has_interpretation_session().into_owned(),
        interpretation_session.clone(),
        g,
    )
}

/// Build a `dec:blockedBy` quad linking a paused DispatchGroup to a
/// blocking `dec:Feedback` artifact (FT-032 §Behaviour, ADR-025).
#[must_use]
pub fn build_blocked_by_quad(group_iri: &NamedNode, feedback_iri: &NamedNode) -> Quad {
    let g: GraphName = orchestration_graph().into_owned().into();
    Quad::new(
        group_iri.clone(),
        blocked_by().into_owned(),
        feedback_iri.clone(),
        g,
    )
}
