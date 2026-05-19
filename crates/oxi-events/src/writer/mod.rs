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

mod commit;

use std::sync::{Arc, Mutex, OnceLock};

use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;
use tracing::{debug, warn};

use crate::error::WriterError;
use crate::mutation::{CommitResult, Mutation};
use crate::outbox::OutboxPublisher;
use crate::quads::subscription_quads;
use crate::subscription::{Subscription, SubscriptionRegistry};

use commit::{apply_mutation_tx, current_sequence, emit_events, plan_commit};

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
        let plan = plan_commit(&self.store, mutation)?;
        apply_mutation_tx(&self.store, &plan)?;

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

        let events = emit_events(
            &self.store,
            &plan.mutation_id,
            plan.mutation_sequence,
            &matches,
        )?;

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

    /// Current value of the persisted sequence counter (0 when unset).
    pub fn current_sequence(&self) -> Result<u64, WriterError> {
        current_sequence(&self.store)
    }
}
