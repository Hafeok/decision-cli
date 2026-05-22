//! Quad builder for the verifier-dispatch subscription seed.
//!
//! Kept in a sibling private module so the parent `mod.rs` stays under the
//! ADR-013 Rule 1 file-length budget. The split is mechanical — only the
//! `seed_quads` helpers live here; runtime behaviour is unchanged.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use super::{PENDING_GROUPS_QUERY, VERIFIER_DISPATCH_HANDLER, VERIFIER_DISPATCH_SUBSCRIPTION_IRI};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Build the quad set that seeds the verifier-dispatch subscription into
/// the `oxi-events:subscriptions` named graph. Used by the init pipeline
/// alongside the slice-1 v0 bootstrap subscriptions (FT-009).
#[must_use]
pub(super) fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(VERIFIER_DISPATCH_SUBSCRIPTION_IRI);
    let mut quads = seed_type_quads(&sub, &subs_graph);
    quads.extend(seed_query_quads(&sub, &subs_graph));
    quads.extend(seed_metadata_quads(sub, &subs_graph));
    quads
}

fn seed_type_quads(sub: &NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    vec![Quad::new(
        sub.clone(),
        rdf_type,
        sub_cls,
        subs_graph.clone(),
    )]
}

fn seed_query_quads(sub: &NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    vec![
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(PENDING_GROUPS_QUERY),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_ASYNC),
            subs_graph.clone(),
        ),
    ]
}

fn seed_metadata_quads(sub: NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(VERIFIER_DISPATCH_HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal("verifier dispatch (action complete, no verifier yet)"),
            subs_graph.clone(),
        ),
    ]
}
