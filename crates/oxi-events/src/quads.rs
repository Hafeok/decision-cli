//! Quad-construction helpers used by [`crate::writer::GraphWriter`].
//!
//! Kept in their own module so the writer stays focused on commit
//! orchestration. All triples produced here live in the framework's
//! reserved named graphs (`meta_graph`, `events_graph`).

use std::collections::HashSet;

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use uuid::Uuid;

use crate::mutation::EventHandle;
use crate::subscription::{Subscription, SubscriptionQuery};
use crate::vocab::{
    events_graph, meta_graph, rdf_type, subscriptions_graph, xsd_datetime, xsd_integer,
    IRI_OXI_ACTOR, IRI_OXI_CAUSE, IRI_OXI_COMMITTED_AT, IRI_OXI_CURRENT, IRI_OXI_EMITTED_AT,
    IRI_OXI_EVENT, IRI_OXI_MATCHED_SUBSCRIPTION, IRI_OXI_MUTATION, IRI_OXI_PUBLISHED, IRI_OXI_SEQ,
    IRI_OXI_SEQ_COUNTER, IRI_OXI_STATUS, IRI_OXI_SUBSCRIPTION, IRI_OXI_SUB_ASK_QUERY,
    IRI_OXI_SUB_HANDLER, IRI_OXI_SUB_MODE, IRI_OXI_SUB_SELECT_QUERY, IRI_OXI_SUB_TRIGGER,
    IRI_PROV_WAS_GENERATED_BY, STATUS_OK,
};

const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

pub(crate) fn mint_iri(prefix: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{prefix}{}", Uuid::new_v4()))
}

pub(crate) fn affected_graph_set(inserts: &[Quad], removes: &[Quad]) -> HashSet<GraphName> {
    let mut out = HashSet::new();
    for q in inserts.iter().chain(removes.iter()) {
        out.insert(q.graph_name.clone());
    }
    out
}

pub(crate) fn mutation_quads(
    mutation_id: &NamedNode,
    mutation_sequence: u64,
    committed_at: &str,
    actor: Option<&NamedNode>,
    cause: Option<&str>,
) -> Vec<Quad> {
    let g: GraphName = meta_graph().into_owned().into();
    let mut quads = vec![
        Quad::new(
            mutation_id.clone(),
            rdf_type(),
            NamedNodeRef::new_unchecked(IRI_OXI_MUTATION).into_owned(),
            g.clone(),
        ),
        Quad::new(
            mutation_id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_SEQ).into_owned(),
            Literal::new_typed_literal(mutation_sequence.to_string(), xsd_integer().into_owned()),
            g.clone(),
        ),
        Quad::new(
            mutation_id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_COMMITTED_AT).into_owned(),
            Literal::new_typed_literal(committed_at, xsd_datetime().into_owned()),
            g.clone(),
        ),
    ];
    if let Some(actor) = actor {
        quads.push(Quad::new(
            mutation_id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_ACTOR).into_owned(),
            actor.clone(),
            g.clone(),
        ));
    }
    if let Some(cause) = cause {
        quads.push(Quad::new(
            mutation_id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_CAUSE).into_owned(),
            Literal::new_simple_literal(cause),
            g,
        ));
    }
    quads
}

pub(crate) fn event_quads_for(
    handle: &EventHandle,
    mutation_id: &NamedNode,
    emitted_at: &str,
) -> Vec<Quad> {
    let g: GraphName = events_graph().into_owned().into();
    let event_class = NamedNodeRef::new_unchecked(IRI_OXI_EVENT).into_owned();
    let xsd_bool = NamedNodeRef::new_unchecked(XSD_BOOLEAN).into_owned();
    vec![
        Quad::new(handle.iri.clone(), rdf_type(), event_class, g.clone()),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_SEQ).into_owned(),
            Literal::new_typed_literal(handle.sequence.to_string(), xsd_integer().into_owned()),
            g.clone(),
        ),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_PROV_WAS_GENERATED_BY).into_owned(),
            mutation_id.clone(),
            g.clone(),
        ),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_STATUS).into_owned(),
            Literal::new_simple_literal(STATUS_OK),
            g.clone(),
        ),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_EMITTED_AT).into_owned(),
            Literal::new_typed_literal(emitted_at, xsd_datetime().into_owned()),
            g.clone(),
        ),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_MATCHED_SUBSCRIPTION).into_owned(),
            handle.subscription.clone(),
            g.clone(),
        ),
        Quad::new(
            handle.iri.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_PUBLISHED).into_owned(),
            Literal::new_typed_literal("false", xsd_bool),
            g,
        ),
    ]
}

pub(crate) fn subscription_quads(sub: &Subscription) -> Vec<Quad> {
    let g: GraphName = subscriptions_graph().into_owned().into();
    let mut quads = vec![Quad::new(
        sub.id.clone(),
        rdf_type(),
        NamedNodeRef::new_unchecked(IRI_OXI_SUBSCRIPTION).into_owned(),
        g.clone(),
    )];
    let (query_pred, query_str) = match &sub.query {
        SubscriptionQuery::Ask(q) => (IRI_OXI_SUB_ASK_QUERY, q.as_str()),
        SubscriptionQuery::Select(q) => (IRI_OXI_SUB_SELECT_QUERY, q.as_str()),
    };
    quads.push(Quad::new(
        sub.id.clone(),
        NamedNodeRef::new_unchecked(query_pred).into_owned(),
        Literal::new_simple_literal(query_str),
        g.clone(),
    ));
    quads.push(Quad::new(
        sub.id.clone(),
        NamedNodeRef::new_unchecked(IRI_OXI_SUB_MODE).into_owned(),
        Literal::new_simple_literal(sub.mode.as_str()),
        g.clone(),
    ));
    for trigger in &sub.triggers {
        quads.push(Quad::new(
            sub.id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_SUB_TRIGGER).into_owned(),
            Literal::new_simple_literal(trigger),
            g.clone(),
        ));
    }
    if let Some(handler) = sub.handler.as_ref() {
        quads.push(Quad::new(
            sub.id.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_SUB_HANDLER).into_owned(),
            Literal::new_simple_literal(handler.as_str()),
            g.clone(),
        ));
    }
    if let Some(label) = sub.label.as_ref() {
        quads.push(Quad::new(
            sub.id.clone(),
            NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label"),
            Literal::new_simple_literal(label.clone()),
            g,
        ));
    }
    quads
}

pub(crate) fn counter_quad(value: u64) -> Quad {
    Quad::new(
        NamedNodeRef::new_unchecked(IRI_OXI_SEQ_COUNTER).into_owned(),
        NamedNodeRef::new_unchecked(IRI_OXI_CURRENT).into_owned(),
        Literal::new_typed_literal(value.to_string(), xsd_integer().into_owned()),
        meta_graph().into_owned(),
    )
}
