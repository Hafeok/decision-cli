//! Commit pipeline for [`GraphWriter`]: planning, applying the
//! mutation transaction, and emitting events for matched subscriptions.

use std::collections::HashSet;

use chrono::Utc;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::error::WriterError;
use crate::mutation::{EventHandle, Mutation};
use crate::quads::{
    affected_graph_set, counter_quad, event_quads_for, mint_iri, mutation_quads,
};
use crate::subscription::{Delta, SubscriptionMatch};
use crate::vocab::{
    events_graph, meta_graph, IRI_OXI_CURRENT, IRI_OXI_SEQ_COUNTER, IRI_OXI_STATUS, STATUS_FAILED,
    STATUS_OK,
};

pub(super) const EVENT_IRI_PREFIX: &str = "https://decision-cli.dev/oxi-events/ns/event/";
pub(super) const MUTATION_IRI_PREFIX: &str = "https://decision-cli.dev/oxi-events/ns/mutation/";

pub(super) struct CommitPlan {
    pub inserts: Vec<Quad>,
    pub removes: Vec<Quad>,
    pub mutation_id: NamedNode,
    pub mutation_sequence: u64,
    pub affected_graphs: HashSet<GraphName>,
    pub mutation_quads: Vec<Quad>,
    pub counter_old: Vec<Quad>,
    pub counter_new: Quad,
}

pub(super) fn plan_commit(
    store: &Store,
    mutation: Mutation,
) -> Result<CommitPlan, WriterError> {
    let Mutation {
        inserts,
        removes,
        actor,
        cause,
        committed_at,
        triggers: _,
    } = mutation;
    let committed_at = committed_at.unwrap_or_else(|| Utc::now().to_rfc3339());
    let mutation_id = mint_iri(MUTATION_IRI_PREFIX);
    let affected_graphs = affected_graph_set(&inserts, &removes);

    let pre_seq = current_sequence(store)?;
    let mutation_sequence = pre_seq + 1;

    let mutation_quads = mutation_quads(
        &mutation_id,
        mutation_sequence,
        &committed_at,
        actor.as_ref(),
        cause.as_deref(),
    );

    let counter_old = existing_counter_quads(store)?;
    let counter_new = counter_quad(mutation_sequence);

    Ok(CommitPlan {
        inserts,
        removes,
        mutation_id,
        mutation_sequence,
        affected_graphs,
        mutation_quads,
        counter_old,
        counter_new,
    })
}

pub(super) fn apply_mutation_tx(store: &Store, plan: &CommitPlan) -> Result<(), WriterError> {
    store.transaction(|mut tx| {
        for q in &plan.removes {
            tx.remove(q.as_ref())?;
        }
        for q in &plan.inserts {
            tx.insert(q.as_ref())?;
        }
        for q in &plan.mutation_quads {
            tx.insert(q.as_ref())?;
        }
        for q in &plan.counter_old {
            tx.remove(q.as_ref())?;
        }
        tx.insert(plan.counter_new.as_ref())?;
        Ok::<_, WriterError>(())
    })
}

pub(super) fn emit_events(
    store: &Store,
    mutation_id: &NamedNode,
    mutation_sequence: u64,
    matches: &[SubscriptionMatch],
) -> Result<Vec<EventHandle>, WriterError> {
    if matches.is_empty() {
        return Ok(Vec::new());
    }
    let emitted_at = Utc::now().to_rfc3339();
    let mut handles = Vec::with_capacity(matches.len());
    let mut next_seq = mutation_sequence + 1;
    for m in matches {
        let status = if m.delta.is_error() {
            STATUS_FAILED.to_string()
        } else {
            STATUS_OK.to_string()
        };
        handles.push(EventHandle {
            iri: mint_iri(EVENT_IRI_PREFIX),
            sequence: next_seq,
            subscription: m.subscription_id.clone(),
            status,
        });
        next_seq += 1;
    }
    let highest_seq = next_seq - 1;

    let mut event_quads: Vec<Quad> = Vec::new();
    for handle in &handles {
        event_quads.extend(event_quads_for(handle, mutation_id, &emitted_at));
    }

    let counter_old = existing_counter_quads(store)?;
    let counter_new = counter_quad(highest_seq);

    store.transaction(|mut tx| {
        for q in &event_quads {
            tx.insert(q.as_ref())?;
        }
        for q in &counter_old {
            tx.remove(q.as_ref())?;
        }
        tx.insert(counter_new.as_ref())?;
        Ok::<_, WriterError>(())
    })?;

    // Status fix-up: event_quads_for inserts STATUS_OK by default;
    // patch the handful of failed events to STATUS_FAILED so the
    // persisted form matches the in-memory delta classification.
    for (handle, m) in handles.iter().zip(matches.iter()) {
        if matches!(m.delta, Delta::Error(_)) {
            patch_event_status(store, &handle.iri, STATUS_FAILED)?;
        }
    }

    Ok(handles)
}

pub(super) fn current_sequence(store: &Store) -> Result<u64, WriterError> {
    let q = format!(
        "SELECT ?n FROM <{graph}> WHERE {{ <{counter}> <{current}> ?n }}",
        graph = meta_graph().as_str(),
        counter = IRI_OXI_SEQ_COUNTER,
        current = IRI_OXI_CURRENT,
    );
    let results = store.query(q.as_str())?;
    if let QueryResults::Solutions(mut sols) = results {
        if let Some(sol) = sols.next() {
            let sol = sol?;
            if let Some(term) = sol.get("n") {
                return parse_u64_literal(term);
            }
        }
    }
    Ok(0)
}

fn existing_counter_quads(store: &Store) -> Result<Vec<Quad>, WriterError> {
    let subject = NamedNodeRef::new_unchecked(IRI_OXI_SEQ_COUNTER);
    let predicate = NamedNodeRef::new_unchecked(IRI_OXI_CURRENT);
    let graph = meta_graph();
    let mut out = Vec::new();
    for quad in store.quads_for_pattern(
        Some(subject.into()),
        Some(predicate),
        None,
        Some(graph.into()),
    ) {
        out.push(quad?);
    }
    Ok(out)
}

fn patch_event_status(store: &Store, event: &NamedNode, status: &str) -> Result<(), WriterError> {
    let g = events_graph();
    let status_pred = NamedNodeRef::new_unchecked(IRI_OXI_STATUS);
    let old_quad = Quad::new(
        event.clone(),
        status_pred.into_owned(),
        Literal::new_simple_literal(STATUS_OK),
        g.into_owned(),
    );
    let new_quad = Quad::new(
        event.clone(),
        status_pred.into_owned(),
        Literal::new_simple_literal(status),
        g.into_owned(),
    );
    store.transaction(|mut tx| {
        tx.remove(old_quad.as_ref())?;
        tx.insert(new_quad.as_ref())?;
        Ok::<_, WriterError>(())
    })?;
    Ok(())
}

fn parse_u64_literal(term: &Term) -> Result<u64, WriterError> {
    match term {
        Term::Literal(lit) => lit
            .value()
            .parse::<u64>()
            .map_err(|e| WriterError::Sequence(format!("parse counter literal: {e}"))),
        _ => Err(WriterError::Sequence(
            "counter triple object is not a literal".to_string(),
        )),
    }
}
