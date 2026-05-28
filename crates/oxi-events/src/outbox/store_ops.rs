//! SPARQL helpers for the outbox publisher's store-side operations.
//!
//! Scans unpublished events from the graph, then flips the
//! `oxi:published` flag once a broadcast has been attempted.

use oxigraph::model::{Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::{QueryResults, QuerySolution};
use oxigraph::store::Store;

use crate::error::WriterError;
use crate::vocab::{
    events_graph, IRI_OXI_EMITTED_AT, IRI_OXI_EVENT, IRI_OXI_MATCHED_SUBSCRIPTION,
    IRI_OXI_PUBLISHED, IRI_OXI_SEQ, IRI_PROV_WAS_GENERATED_BY,
};

use super::EventEnvelope;

pub(super) const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

pub(super) fn scan_unpublished(store: &Store) -> Result<Vec<EventEnvelope>, WriterError> {
    let q = build_unpublished_select();
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        if let Some(envelope) = decode_unpublished_row(&sol)? {
            out.push(envelope);
        }
    }
    Ok(out)
}

fn build_unpublished_select() -> String {
    format!(
        "SELECT ?e ?seq ?mut ?sub ?ts FROM <{events}> WHERE {{ \
            ?e a <{event}> ; \
               <{published}> false ; \
               <{seq}> ?seq ; \
               <{wgb}> ?mut ; \
               <{matched}> ?sub ; \
               <{emitted}> ?ts \
         }} ORDER BY ?seq",
        events = crate::vocab::IRI_OXI_GRAPH_EVENTS,
        event = IRI_OXI_EVENT,
        published = IRI_OXI_PUBLISHED,
        seq = IRI_OXI_SEQ,
        wgb = IRI_PROV_WAS_GENERATED_BY,
        matched = IRI_OXI_MATCHED_SUBSCRIPTION,
        emitted = IRI_OXI_EMITTED_AT,
    )
}

fn decode_unpublished_row(sol: &QuerySolution) -> Result<Option<EventEnvelope>, WriterError> {
    let Some(Term::NamedNode(event)) = sol.get("e").cloned() else {
        return Ok(None);
    };
    let Some(Term::Literal(seq_lit)) = sol.get("seq").cloned() else {
        return Ok(None);
    };
    let Some(Term::NamedNode(mutation)) = sol.get("mut").cloned() else {
        return Ok(None);
    };
    let Some(Term::NamedNode(subscription)) = sol.get("sub").cloned() else {
        return Ok(None);
    };
    let Some(Term::Literal(ts_lit)) = sol.get("ts").cloned() else {
        return Ok(None);
    };
    let seq: u64 = seq_lit
        .value()
        .parse()
        .map_err(|e| WriterError::Internal(format!("outbox: malformed seq literal: {e}")))?;
    Ok(Some(EventEnvelope {
        event: event.as_str().to_string(),
        seq,
        mutation: mutation.as_str().to_string(),
        subscription: subscription.as_str().to_string(),
        emitted_at: ts_lit.value().to_string(),
    }))
}

pub(super) fn mark_published(store: &Store, event_iri: &str) -> Result<(), WriterError> {
    let event = NamedNode::new(event_iri)
        .map_err(|e| WriterError::Internal(format!("outbox: invalid event IRI: {e}")))?;
    let g = events_graph();
    let pred = NamedNodeRef::new_unchecked(IRI_OXI_PUBLISHED);
    let xsd_bool = NamedNodeRef::new_unchecked(XSD_BOOLEAN);
    let old_quad = Quad::new(
        event.clone(),
        pred.into_owned(),
        Literal::new_typed_literal("false", xsd_bool.into_owned()),
        g.into_owned(),
    );
    let new_quad = Quad::new(
        event,
        pred.into_owned(),
        Literal::new_typed_literal("true", xsd_bool.into_owned()),
        g.into_owned(),
    );
    store.transaction(|mut tx| {
        tx.remove(old_quad.as_ref())?;
        tx.insert(new_quad.as_ref())?;
        Ok::<_, WriterError>(())
    })?;
    Ok(())
}
