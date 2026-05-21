//! Subscription-seed assembly for the feedback-routing handler (FT-029).
//!
//! Split out of `handler.rs` to keep that module under ADR-013 Rule 1's
//! 400-line hard cap. The only public surface is [`seed_quads`], which
//! the init pipeline calls alongside the slice-1 v0 bootstrap
//! subscriptions and the FT-022 verifier-dispatch seed.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use super::handler::{
    FEEDBACK_ROUTING_HANDLER, FEEDBACK_ROUTING_SUBSCRIPTION_IRI, PENDING_FEEDBACK_QUERY,
};

/// Build the quad set that seeds the feedback-routing subscription into
/// the `oxi-events:subscriptions` named graph.
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(FEEDBACK_ROUTING_SUBSCRIPTION_IRI);
    let rdf_type =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(sub.clone(), rdf_type, sub_cls, subs_graph.clone()),
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(PENDING_FEEDBACK_QUERY),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_INLINE),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(FEEDBACK_ROUTING_HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal("feedback routing (produced → routed)"),
            subs_graph,
        ),
    ]
}
