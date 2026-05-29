//! Quad builders for `dec:VerifyGraphAuthorDispatchEvent` artifacts.
//!
//! Mirrors the shape of `core::subscriptions::verifier_dispatch::quads`
//! per FT-050 §Outputs: "same shape pattern as VerifierDispatchEvent".
//! Pure / no IO; the StreamWriter chokepoint augments these with
//! `dec:inStream` per ADR-005.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    bundle_hash_pred, emitted_at, event_class, feature_ref, orchestration_graph, target_role,
    triggered_by_event_id, verify_graph_author_dispatch_event_class,
    EVENT_CLASS_VERIFY_GRAPH_AUTHOR_DISPATCH, IRI_DEC_BENCH, IRI_DEC_EVENT,
    VERIFY_GRAPH_AUTHOR_TARGET_ROLE,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// All fields the dispatch event carries (FT-050 §Outputs).
#[derive(Debug, Clone)]
pub struct DispatchEventQuadInput<'a> {
    /// IRI of the event artifact.
    pub event_iri: &'a NamedNode,
    /// Short feature id (e.g. `FT-050`).
    pub feature_short: &'a str,
    /// Short env id (e.g. `BNCH-001-ephemeral-cli`).
    pub env_short: &'a str,
    /// SHA-256 hex of the assembled bundle.
    pub bundle_hash: &'a str,
    /// Originating event id (the feature create/update event).
    pub triggered_by_event_id: &'a str,
    /// RFC3339 timestamp.
    pub emitted_at: &'a str,
}

/// Build the canonical quad set for a `dec:VerifyGraphAuthorDispatchEvent`.
#[must_use]
pub fn build_event_quads(input: &DispatchEventQuadInput<'_>) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads = build_event_type_quads(input.event_iri, &g);
    quads.extend(build_event_payload_quads(input, &g));
    quads
}

fn build_event_type_quads(event_iri: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    vec![
        // Abstract dec:Event so ADR-005 scope tagging fires.
        Quad::new(
            event_iri.clone(),
            rdf_type.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_EVENT).into_owned(),
            g.clone(),
        ),
        // Concrete dec:VerifyGraphAuthorDispatchEvent.
        Quad::new(
            event_iri.clone(),
            rdf_type,
            verify_graph_author_dispatch_event_class().into_owned(),
            g.clone(),
        ),
    ]
}

fn build_event_payload_quads(input: &DispatchEventQuadInput<'_>, g: &GraphName) -> Vec<Quad> {
    let mut quads = build_event_role_quads(input, g);
    quads.extend(build_event_provenance_quads(input, g));
    quads
}

fn build_event_role_quads(input: &DispatchEventQuadInput<'_>, g: &GraphName) -> Vec<Quad> {
    let environment_pred = NamedNodeRef::new_unchecked(IRI_DEC_BENCH).into_owned();
    vec![
        Quad::new(
            input.event_iri.clone(),
            event_class().into_owned(),
            Literal::new_simple_literal(EVENT_CLASS_VERIFY_GRAPH_AUTHOR_DISPATCH),
            g.clone(),
        ),
        Quad::new(
            input.event_iri.clone(),
            target_role().into_owned(),
            Literal::new_simple_literal(VERIFY_GRAPH_AUTHOR_TARGET_ROLE),
            g.clone(),
        ),
        Quad::new(
            input.event_iri.clone(),
            feature_ref().into_owned(),
            Literal::new_simple_literal(input.feature_short),
            g.clone(),
        ),
        Quad::new(
            input.event_iri.clone(),
            environment_pred,
            Literal::new_simple_literal(input.env_short),
            g.clone(),
        ),
    ]
}

fn build_event_provenance_quads(input: &DispatchEventQuadInput<'_>, g: &GraphName) -> Vec<Quad> {
    vec![
        Quad::new(
            input.event_iri.clone(),
            bundle_hash_pred().into_owned(),
            Literal::new_simple_literal(input.bundle_hash),
            g.clone(),
        ),
        Quad::new(
            input.event_iri.clone(),
            triggered_by_event_id().into_owned(),
            Literal::new_simple_literal(input.triggered_by_event_id),
            g.clone(),
        ),
        Quad::new(
            input.event_iri.clone(),
            emitted_at().into_owned(),
            Literal::new_simple_literal(input.emitted_at),
            g.clone(),
        ),
    ]
}
