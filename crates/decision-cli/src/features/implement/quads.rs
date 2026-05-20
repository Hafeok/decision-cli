//! Quad builders for the implementer dispatch lifecycle.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    orchestration_graph, IRI_DEC_ACTION_SESSION, IRI_DEC_DISPATCH, IRI_DEC_SESSION,
};

use super::vocab::{
    DEC_BUNDLE_REF, DEC_CONTENT_HASH, DEC_DISPATCH_PROP, DEC_FEATURE_ID, DEC_MODEL_REF,
    DEC_MODEL_VERSION, DEC_OUTPUT_REF, DEC_ROLE_PROP, DEC_STATUS_PROP, PROV_ACTIVITY, PROV_AT_TIME,
    PROV_ENDED_AT_TIME, PROV_GENERATED_BY, PROV_USED, RDF_TYPE,
};
use super::IMPLEMENTER_ROLE;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_session_quads(
    session: &NamedNode,
    dispatch: &NamedNode,
    bundle: &NamedNode,
    bundle_hash: &str,
    model: &NamedNode,
    model_version: &str,
    feature_id: &str,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads = session_typing_quads(session, bundle, model, &g);
    quads.extend(session_provenance_quads(
        session,
        bundle,
        bundle_hash,
        model,
        model_version,
        &g,
    ));
    quads.extend(session_metadata_quads(
        session, dispatch, feature_id, started_at, &g,
    ));
    quads
}

fn session_typing_quads(
    session: &NamedNode,
    bundle: &NamedNode,
    model: &NamedNode,
    g: &GraphName,
) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let session_class = NamedNodeRef::new_unchecked(IRI_DEC_SESSION).into_owned();
    let action_class = NamedNodeRef::new_unchecked(IRI_DEC_ACTION_SESSION).into_owned();
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY).into_owned();
    let used = NamedNodeRef::new_unchecked(PROV_USED).into_owned();
    vec![
        Quad::new(session.clone(), rdf_type.clone(), session_class, g.clone()),
        // FT-021 / ADR-017: implementer session is also typed as
        // dec:ActionSession so it can be paired by a DispatchGroup.
        Quad::new(session.clone(), rdf_type.clone(), action_class, g.clone()),
        Quad::new(session.clone(), rdf_type, activity, g.clone()),
        Quad::new(session.clone(), used.clone(), bundle.clone(), g.clone()),
        Quad::new(session.clone(), used, model.clone(), g.clone()),
    ]
}

fn session_provenance_quads(
    session: &NamedNode,
    bundle: &NamedNode,
    bundle_hash: &str,
    model: &NamedNode,
    model_version: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let content_hash = NamedNodeRef::new_unchecked(DEC_CONTENT_HASH).into_owned();
    let model_version_pred = NamedNodeRef::new_unchecked(DEC_MODEL_VERSION).into_owned();
    let bundle_ref_pred = NamedNodeRef::new_unchecked(DEC_BUNDLE_REF).into_owned();
    let model_ref_pred = NamedNodeRef::new_unchecked(DEC_MODEL_REF).into_owned();
    vec![
        Quad::new(
            bundle.clone(),
            content_hash,
            Literal::new_simple_literal(bundle_hash),
            g.clone(),
        ),
        Quad::new(
            model.clone(),
            model_version_pred,
            Literal::new_simple_literal(model_version),
            g.clone(),
        ),
        Quad::new(session.clone(), bundle_ref_pred, bundle.clone(), g.clone()),
        Quad::new(session.clone(), model_ref_pred, model.clone(), g.clone()),
    ]
}

fn session_metadata_quads(
    session: &NamedNode,
    dispatch: &NamedNode,
    feature_id: &str,
    started_at: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    let feature_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let dispatch_pred = NamedNodeRef::new_unchecked(DEC_DISPATCH_PROP).into_owned();
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE_PROP).into_owned();
    vec![
        Quad::new(
            session.clone(),
            feature_pred,
            Literal::new_simple_literal(feature_id),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g.clone(),
        ),
        Quad::new(session.clone(), dispatch_pred, dispatch.clone(), g.clone()),
        Quad::new(
            session.clone(),
            role_pred,
            Literal::new_simple_literal(IMPLEMENTER_ROLE),
            g.clone(),
        ),
    ]
}

pub(super) fn build_dispatch_quads(
    dispatch: &NamedNode,
    session: &NamedNode,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let dispatch_class = NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH).into_owned();
    let generated = NamedNodeRef::new_unchecked(PROV_GENERATED_BY).into_owned();
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE_PROP).into_owned();
    vec![
        Quad::new(dispatch.clone(), rdf_type, dispatch_class, g.clone()),
        Quad::new(dispatch.clone(), generated, session.clone(), g.clone()),
        Quad::new(
            dispatch.clone(),
            role_pred,
            Literal::new_simple_literal(IMPLEMENTER_ROLE),
            g.clone(),
        ),
        Quad::new(
            dispatch.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g,
        ),
    ]
}

pub(super) fn build_completion_quads(
    session: &NamedNode,
    code_change: &NamedNode,
    completed_at: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let output_ref = NamedNodeRef::new_unchecked(DEC_OUTPUT_REF).into_owned();
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS_PROP).into_owned();
    let ended_at = NamedNodeRef::new_unchecked(PROV_ENDED_AT_TIME).into_owned();
    vec![
        Quad::new(session.clone(), output_ref, code_change.clone(), g.clone()),
        Quad::new(
            session.clone(),
            status_pred,
            Literal::new_simple_literal("complete"),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            ended_at,
            Literal::new_simple_literal(completed_at),
            g,
        ),
    ]
}

pub(super) fn build_failure_quad(session: &NamedNode, detail: &str) -> Quad {
    let g: GraphName = orchestration_graph().into_owned().into();
    Quad::new(
        session.clone(),
        NamedNodeRef::new_unchecked(DEC_STATUS_PROP).into_owned(),
        Literal::new_simple_literal(format!("failed: {detail}")),
        g,
    )
}
