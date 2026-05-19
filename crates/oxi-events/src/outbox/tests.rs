//! Unit tests for the outbox publisher.

use std::sync::Arc;

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::error::WriterError;
use crate::vocab::{
    events_graph, IRI_OXI_EMITTED_AT, IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS,
    IRI_OXI_MATCHED_SUBSCRIPTION, IRI_OXI_PUBLISHED, IRI_OXI_SEQ, IRI_PROV_WAS_GENERATED_BY,
};

use super::store_ops::XSD_BOOLEAN;
use super::OutboxPublisher;

fn insert_fake_event(store: &Store, iri: &str, seq: u64) {
    let g: GraphName = events_graph().into_owned().into();
    let event = NamedNode::new(iri).expect("iri");
    let mutation = NamedNode::new("urn:test:mutation:1").expect("mut iri");
    let subscription = NamedNode::new("urn:test:sub:1").expect("sub iri");
    let xsd_int = NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");
    let xsd_bool = NamedNodeRef::new_unchecked(XSD_BOOLEAN);
    let xsd_dt = NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime");
    let triples = vec![
        Quad::new(
            event.clone(),
            NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
                .into_owned(),
            NamedNodeRef::new_unchecked(IRI_OXI_EVENT).into_owned(),
            g.clone(),
        ),
        Quad::new(
            event.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_SEQ).into_owned(),
            Literal::new_typed_literal(seq.to_string(), xsd_int.into_owned()),
            g.clone(),
        ),
        Quad::new(
            event.clone(),
            NamedNodeRef::new_unchecked(IRI_PROV_WAS_GENERATED_BY).into_owned(),
            mutation,
            g.clone(),
        ),
        Quad::new(
            event.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_MATCHED_SUBSCRIPTION).into_owned(),
            subscription,
            g.clone(),
        ),
        Quad::new(
            event.clone(),
            NamedNodeRef::new_unchecked(IRI_OXI_EMITTED_AT).into_owned(),
            Literal::new_typed_literal("2026-05-18T00:00:00Z", xsd_dt.into_owned()),
            g.clone(),
        ),
        Quad::new(
            event,
            NamedNodeRef::new_unchecked(IRI_OXI_PUBLISHED).into_owned(),
            Literal::new_typed_literal("false", xsd_bool.into_owned()),
            g,
        ),
    ];
    store
        .transaction(|mut tx| {
            for q in &triples {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, WriterError>(())
        })
        .expect("seed event");
    let _ = IRI_OXI_GRAPH_EVENTS;
}

#[test]
fn run_once_marks_events_published() {
    let store = Arc::new(Store::new().expect("store"));
    insert_fake_event(&store, "urn:test:event:1", 2);
    let publisher = OutboxPublisher::with_defaults(Arc::clone(&store));
    let mut rx = publisher.subscribe();
    let n = publisher.run_once().expect("run_once");
    assert_eq!(n, 1, "exactly one event should be processed");
    let env = rx.try_recv().expect("envelope delivered");
    assert_eq!(env.seq, 2);
    let again = publisher.run_once().expect("idempotent");
    assert_eq!(again, 0);
}

#[test]
fn run_once_succeeds_with_no_receivers() {
    let store = Arc::new(Store::new().expect("store"));
    insert_fake_event(&store, "urn:test:event:2", 2);
    let publisher = OutboxPublisher::with_defaults(Arc::clone(&store));
    let n = publisher.run_once().expect("run_once");
    assert_eq!(n, 1);
}
