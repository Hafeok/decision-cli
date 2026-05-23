//! Quad builders for FT-062 session linkage with enriched bundle persistence.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::bundle::Bundle;
use crate::core::ontology::role_binding::TriggerSignal;
use crate::core::vocab::{
    bundle_class, bundle_graph, escalated_from_pred, escalated_to_pred, escalation_reason_pred,
    focal_pred, input_tokens_base_pred, input_tokens_cache_hit_pred,
    input_tokens_cache_write_pred, output_tokens_pred, session_capability_pred, stakes_pred,
};

use super::bundle_enrich::{IRI_DEC_SUPERSEDES_BUNDLE, PROV_WAS_DERIVED_FROM};
use super::triggers::capability_iri;
use super::types::{AttemptTokens, DispatchAttempt, SessionId};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const IRI_DEC_SESSION: &str = "https://decision-cli.dev/ns#Session";
const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// Build the quads that persist an enriched bundle artifact plus the
/// `dec:supersedes_bundle` link back to the original.
#[must_use]
pub fn build_enriched_bundle_quads(enriched: &Bundle, original: &Bundle) -> Vec<Quad> {
    let g: GraphName = bundle_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let cls = bundle_class();
    let subject = enriched.iri();
    vec![
        Quad::new(subject.clone(), rdf_type, cls, g.clone()),
        Quad::new(
            subject.clone(),
            focal_pred().into_owned(),
            enriched.focal.clone(),
            g.clone(),
        ),
        Quad::new(
            subject.clone(),
            stakes_pred().into_owned(),
            Literal::new_simple_literal(enriched.stakes.as_str()),
            g.clone(),
        ),
        Quad::new(
            subject.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_SUPERSEDES_BUNDLE).into_owned(),
            original.iri(),
            g.clone(),
        ),
        Quad::new(
            subject,
            NamedNodeRef::new_unchecked(PROV_WAS_DERIVED_FROM).into_owned(),
            original.iri(),
            g,
        ),
    ]
}

/// Build the quad set for a new escalated session.
///
/// `prior_session` + `reason` are `Some` for non-root attempts; the
/// new session gets `dec:escalated_from` and `dec:escalation_reason`,
/// and the prior session simultaneously gets a `dec:escalated_to`
/// pointing here.
#[must_use]
pub fn build_session_linkage_quads(
    attempt: &DispatchAttempt,
    tokens: AttemptTokens,
    prior_session: Option<&SessionId>,
    reason: Option<TriggerSignal>,
) -> Vec<Quad> {
    let g: GraphName = bundle_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let cap_iri = capability_iri(&attempt.capability);
    let mut out = base_session_quads(&attempt.session_id, &cap_iri, tokens, &g, rdf_type);
    if let (Some(prior), Some(reason)) = (prior_session, reason) {
        out.push(Quad::new(
            attempt.session_id.clone(),
            escalated_from_pred().into_owned(),
            prior.clone(),
            g.clone(),
        ));
        out.push(Quad::new(
            attempt.session_id.clone(),
            escalation_reason_pred().into_owned(),
            Literal::new_simple_literal(reason.as_str()),
            g.clone(),
        ));
        out.push(Quad::new(
            prior.clone(),
            escalated_to_pred().into_owned(),
            attempt.session_id.clone(),
            g,
        ));
    }
    out
}

fn base_session_quads(
    session: &SessionId,
    cap_iri: &NamedNode,
    tokens: AttemptTokens,
    g: &GraphName,
    rdf_type: NamedNodeRef<'_>,
) -> Vec<Quad> {
    let mut out = header_quads(session, cap_iri, g, rdf_type);
    out.extend(token_quads(session, tokens, g));
    out
}

fn header_quads(
    session: &SessionId,
    cap_iri: &NamedNode,
    g: &GraphName,
    rdf_type: NamedNodeRef<'_>,
) -> Vec<Quad> {
    vec![
        Quad::new(
            session.clone(),
            rdf_type,
            NamedNode::new_unchecked(IRI_DEC_SESSION),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            session_capability_pred().into_owned(),
            cap_iri.clone(),
            g.clone(),
        ),
    ]
}

fn token_quads(session: &SessionId, tokens: AttemptTokens, g: &GraphName) -> Vec<Quad> {
    vec![
        typed_literal_quad(
            session,
            input_tokens_base_pred(),
            &tokens.input_base.to_string(),
            XSD_INTEGER,
            g,
        ),
        typed_literal_quad(
            session,
            input_tokens_cache_write_pred(),
            &tokens.input_cache_write.to_string(),
            XSD_INTEGER,
            g,
        ),
        typed_literal_quad(
            session,
            input_tokens_cache_hit_pred(),
            &tokens.input_cache_hit.to_string(),
            XSD_INTEGER,
            g,
        ),
        typed_literal_quad(
            session,
            output_tokens_pred(),
            &tokens.output.to_string(),
            XSD_INTEGER,
            g,
        ),
    ]
}

fn typed_literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    datatype: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        g.clone(),
    )
}
