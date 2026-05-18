//! [`GraphWriter`] — the mutation chokepoint over an Oxigraph store.
//!
//! All graph writes from the orchestrator route through this writer.
//! On commit the writer:
//!
//! 1. Applies the mutation inside a transaction (mutation node + seq + user quads).
//! 2. Invokes [`SubscriptionRegistry::evaluate`] against the post-commit
//!    store to obtain the set of [`SubscriptionMatch`](crate::subscription::SubscriptionMatch)
//!    records — this is the FT-002 evaluator contract.
//! 3. Persists an `oxi:Event` per match in a second transaction, with
//!    `prov:wasGeneratedBy` linking back to the mutation (ADR-004).
//! 4. Returns a [`CommitResult`].
//!
//! Sequence numbers are minted as a single contiguous run across both
//! the mutation node and the events it produces (mutation = `pre+1`,
//! events = `pre+2..=pre+1+n`). The counter is persisted in the meta
//! named graph so it survives restart (ADR-002: graph is the state).

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::Utc;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use tracing::{debug, warn};

use crate::error::WriterError;
use crate::mutation::{CommitResult, EventHandle, Mutation};
use crate::outbox::OutboxPublisher;
use crate::quads::{
    affected_graph_set, counter_quad, event_quads_for, mint_iri, mutation_quads, subscription_quads,
};
use crate::subscription::{Delta, Subscription, SubscriptionMatch, SubscriptionRegistry};
use crate::vocab::{
    events_graph, meta_graph, IRI_OXI_CURRENT, IRI_OXI_SEQ_COUNTER, IRI_OXI_STATUS, STATUS_FAILED,
    STATUS_OK,
};

const EVENT_IRI_PREFIX: &str = "https://decision-cli.dev/oxi-events/ns/event/";
const MUTATION_IRI_PREFIX: &str = "https://decision-cli.dev/oxi-events/ns/mutation/";

/// Mutation chokepoint over an Oxigraph [`Store`].
pub struct GraphWriter {
    store: Arc<Store>,
    registry: Arc<SubscriptionRegistry>,
    commit_lock: Mutex<()>,
    outbox: OnceLock<Arc<OutboxPublisher>>,
}

impl GraphWriter {
    /// Open a writer over `store`, recovering the seq counter from the
    /// meta named graph and rehydrating any persisted subscriptions
    /// (FT-002 §Behaviour step 1).
    pub fn open(store: Arc<Store>) -> Result<Self, WriterError> {
        let registry = Arc::new(SubscriptionRegistry::new());
        registry.load_from_store(&store)?;
        let writer = Self {
            store,
            registry,
            commit_lock: Mutex::new(()),
            outbox: OnceLock::new(),
        };
        let _ = writer.current_sequence()?;
        Ok(writer)
    }

    /// Attach an [`OutboxPublisher`] so successful commits nudge the
    /// background sweep loop without waiting for the poll tick.
    ///
    /// Returns `Err` if an outbox is already attached.
    pub fn attach_outbox(&self, outbox: Arc<OutboxPublisher>) -> Result<(), WriterError> {
        self.outbox
            .set(outbox)
            .map_err(|_| WriterError::Internal("outbox already attached".to_string()))
    }

    /// Borrow the store handle (read-path only; ADR-001 keeps writes here).
    #[must_use]
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// Borrow the subscription registry.
    #[must_use]
    pub fn registry(&self) -> &Arc<SubscriptionRegistry> {
        &self.registry
    }

    /// Register a subscription and persist its declaration as a
    /// first-class graph artifact (ADR-003).
    pub fn register_subscription(&self, sub: &Subscription) -> Result<(), WriterError> {
        self.registry.register(sub.clone())?;
        let quads = subscription_quads(sub);
        self.store.transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, WriterError>(())
        })?;
        Ok(())
    }

    /// Remove a subscription by id (in-memory and on-disk).
    pub fn remove_subscription(&self, id: &NamedNode) -> Result<bool, WriterError> {
        let removed = self.registry.remove(id)?;
        if !removed {
            return Ok(false);
        }
        let g = crate::vocab::subscriptions_graph();
        let quads: Vec<Quad> = self
            .store
            .quads_for_pattern(Some(id.as_ref().into()), None, None, Some(g.into()))
            .collect::<Result<Vec<_>, _>>()?;
        if !quads.is_empty() {
            self.store.transaction(|mut tx| {
                for q in &quads {
                    tx.remove(q.as_ref())?;
                }
                Ok::<_, WriterError>(())
            })?;
        }
        Ok(true)
    }

    /// Commit a mutation atomically; mint seq numbers and fire subscriptions.
    pub fn commit(&self, mutation: Mutation) -> Result<CommitResult, WriterError> {
        let _commit_guard = self
            .commit_lock
            .lock()
            .map_err(|_| WriterError::Internal("commit lock poisoned".to_string()))?;

        let mutation_triggers = mutation.triggers.clone();
        let plan = self.plan_commit(mutation)?;
        self.apply_mutation_tx(&plan)?;

        // FT-002 §Behaviour steps 2-5: evaluate subscriptions against the
        // post-commit store. Errors in this layer are isolated to the
        // affected subscription (Delta::Error) and never block the commit.
        let matches = match self.registry.evaluate(&self.store, &mutation_triggers) {
            Ok(m) => m,
            Err(err) => {
                warn!(error = %err, "subscription evaluator failed wholesale; commit completes without events");
                Vec::new()
            }
        };

        let events = self.emit_events(&plan.mutation_id, plan.mutation_sequence, &matches)?;

        debug!(
            mutation = %plan.mutation_id.as_str(),
            seq = plan.mutation_sequence,
            events = events.len(),
            "commit succeeded"
        );

        if !events.is_empty() {
            if let Some(outbox) = self.outbox.get() {
                outbox.notify();
            }
        }

        Ok(CommitResult {
            mutation_id: plan.mutation_id,
            mutation_sequence: plan.mutation_sequence,
            affected_graphs: plan.affected_graphs,
            events,
        })
    }

    fn plan_commit(&self, mutation: Mutation) -> Result<CommitPlan, WriterError> {
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

        let pre_seq = self.current_sequence()?;
        let mutation_sequence = pre_seq + 1;

        let mutation_quads = mutation_quads(
            &mutation_id,
            mutation_sequence,
            &committed_at,
            actor.as_ref(),
            cause.as_deref(),
        );

        let counter_old = self.existing_counter_quads()?;
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

    fn apply_mutation_tx(&self, plan: &CommitPlan) -> Result<(), WriterError> {
        self.store.transaction(|mut tx| {
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

    fn emit_events(
        &self,
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

        let counter_old = self.existing_counter_quads()?;
        let counter_new = counter_quad(highest_seq);

        self.store.transaction(|mut tx| {
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
                self.patch_event_status(&handle.iri, STATUS_FAILED)?;
            }
        }

        Ok(handles)
    }

    /// Current value of the persisted sequence counter (0 when unset).
    pub fn current_sequence(&self) -> Result<u64, WriterError> {
        let q = format!(
            "SELECT ?n FROM <{graph}> WHERE {{ <{counter}> <{current}> ?n }}",
            graph = meta_graph().as_str(),
            counter = IRI_OXI_SEQ_COUNTER,
            current = IRI_OXI_CURRENT,
        );
        let results = self.store.query(q.as_str())?;
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

    fn existing_counter_quads(&self) -> Result<Vec<Quad>, WriterError> {
        let subject = NamedNodeRef::new_unchecked(IRI_OXI_SEQ_COUNTER);
        let predicate = NamedNodeRef::new_unchecked(IRI_OXI_CURRENT);
        let graph = meta_graph();
        let mut out = Vec::new();
        for quad in self.store.quads_for_pattern(
            Some(subject.into()),
            Some(predicate),
            None,
            Some(graph.into()),
        ) {
            out.push(quad?);
        }
        Ok(out)
    }

    fn patch_event_status(&self, event: &NamedNode, status: &str) -> Result<(), WriterError> {
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
        self.store.transaction(|mut tx| {
            tx.remove(old_quad.as_ref())?;
            tx.insert(new_quad.as_ref())?;
            Ok::<_, WriterError>(())
        })?;
        Ok(())
    }
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

struct CommitPlan {
    inserts: Vec<Quad>,
    removes: Vec<Quad>,
    mutation_id: NamedNode,
    mutation_sequence: u64,
    affected_graphs: HashSet<GraphName>,
    mutation_quads: Vec<Quad>,
    counter_old: Vec<Quad>,
    counter_new: Quad,
}
