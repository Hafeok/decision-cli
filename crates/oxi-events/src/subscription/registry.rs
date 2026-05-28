//! [`SubscriptionRegistry`] — in-process subscription state.

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;

use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use tracing::warn;

use crate::error::RegistryError;

use super::helpers::{collect_bindings, diff_bindings, triggers_compatible, validate_query};
use super::store_io::read_persisted_subscriptions;
use super::types::{
    Binding, Delta, Subscription, SubscriptionMatch, SubscriptionQuery, TriggerType,
};

#[derive(Debug, Default)]
struct RegistryInner {
    subscriptions: Vec<Subscription>,
    /// Cached prior-evaluation bindings keyed by subscription id (FT-002
    /// §State). `ASK` subscriptions are not cached — they fire on every
    /// `true` per slice 1 semantics.
    cache: HashMap<NamedNode, Vec<Binding>>,
}

/// In-process registry of subscriptions evaluated on every commit
/// (FT-002).
#[derive(Debug, Default)]
pub struct SubscriptionRegistry {
    inner: RwLock<RegistryInner>,
}

impl SubscriptionRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of registered subscriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .map(|g| g.subscriptions.len())
            .unwrap_or_default()
    }

    /// Whether the registry has no subscriptions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Register a subscription. Validates SPARQL syntax up-front and
    /// rejects duplicate ids.
    pub fn register(&self, subscription: Subscription) -> Result<(), RegistryError> {
        validate_query(&subscription.id, &subscription.query)?;
        let mut guard = self.inner.write().map_err(|_| RegistryError::Poisoned)?;
        if guard.subscriptions.iter().any(|s| s.id == subscription.id) {
            return Err(RegistryError::DuplicateSubscription(
                subscription.id.as_str().to_string(),
            ));
        }
        guard.subscriptions.push(subscription);
        Ok(())
    }

    /// Remove a subscription by id, returning `true` iff a record was
    /// removed. Cached bindings for that id are dropped.
    pub fn remove(&self, id: &NamedNode) -> Result<bool, RegistryError> {
        let mut guard = self.inner.write().map_err(|_| RegistryError::Poisoned)?;
        let before = guard.subscriptions.len();
        guard.subscriptions.retain(|s| &s.id != id);
        let removed = guard.subscriptions.len() != before;
        if removed {
            guard.cache.remove(id);
        }
        Ok(removed)
    }

    /// Replace an existing subscription. Errors if no record with the
    /// new subscription's id is currently registered. Cached bindings
    /// are dropped (the diff baseline resets).
    pub fn replace(&self, subscription: &Subscription) -> Result<(), RegistryError> {
        validate_query(&subscription.id, &subscription.query)?;
        let mut guard = self.inner.write().map_err(|_| RegistryError::Poisoned)?;
        let Some(slot) = guard
            .subscriptions
            .iter_mut()
            .find(|s| s.id == subscription.id)
        else {
            return Err(RegistryError::SubscriptionNotFound(
                subscription.id.as_str().to_string(),
            ));
        };
        *slot = subscription.clone();
        guard.cache.remove(&subscription.id);
        Ok(())
    }

    /// Snapshot the registry as an owned `Vec`. Used by the writer to
    /// iterate without holding the lock across SPARQL evaluation.
    pub fn snapshot(&self) -> Result<Vec<Subscription>, RegistryError> {
        let guard = self.inner.read().map_err(|_| RegistryError::Poisoned)?;
        Ok(guard.subscriptions.clone())
    }

    /// Cached prior-evaluation bindings for a subscription (for tests
    /// and debugging — production callers should use [`Self::evaluate`]).
    pub fn cached_bindings(&self, id: &NamedNode) -> Result<Option<Vec<Binding>>, RegistryError> {
        let guard = self.inner.read().map_err(|_| RegistryError::Poisoned)?;
        Ok(guard.cache.get(id).cloned())
    }

    /// Rebuild the in-memory registry from the persisted `subscriptions`
    /// named graph (FT-002 §Behaviour step 1). Existing in-memory state
    /// is wiped first so this is safe to call on startup.
    pub fn load_from_store(&self, store: &Store) -> Result<usize, RegistryError> {
        let loaded = read_persisted_subscriptions(store)?;
        let mut guard = self.inner.write().map_err(|_| RegistryError::Poisoned)?;
        guard.subscriptions.clear();
        guard.cache.clear();
        let count = loaded.len();
        guard.subscriptions = loaded;
        Ok(count)
    }

    /// Evaluate every registered subscription against `store` and return
    /// the matches. The signature of this method is the contract FT-001
    /// calls on every commit.
    pub fn evaluate(
        &self,
        store: &Store,
        mutation_triggers: &BTreeSet<TriggerType>,
    ) -> Result<Vec<SubscriptionMatch>, RegistryError> {
        let subs = self.snapshot()?;
        let mut matches = Vec::new();
        let mut cache_updates: Vec<(NamedNode, Vec<Binding>)> = Vec::new();

        for sub in &subs {
            if !triggers_compatible(&sub.triggers, mutation_triggers) {
                continue;
            }
            match &sub.query {
                SubscriptionQuery::Ask(q) => run_ask(store, sub, q.as_str(), &mut matches),
                SubscriptionQuery::Select(q) => {
                    self.run_select(store, sub, q.as_str(), &mut matches, &mut cache_updates)?;
                }
            }
        }

        if !cache_updates.is_empty() {
            let mut guard = self.inner.write().map_err(|_| RegistryError::Poisoned)?;
            for (id, bindings) in cache_updates {
                guard.cache.insert(id, bindings);
            }
        }

        Ok(matches)
    }

    fn run_select(
        &self,
        store: &Store,
        sub: &Subscription,
        query: &str,
        matches: &mut Vec<SubscriptionMatch>,
        cache_updates: &mut Vec<(NamedNode, Vec<Binding>)>,
    ) -> Result<(), RegistryError> {
        let bindings = match collect_bindings(store, query) {
            Ok(b) => b,
            Err(err) => {
                matches.push(select_error_match(sub, &err));
                return Ok(());
            }
        };
        let prior = self.prior_bindings(&sub.id)?;
        let delta = diff_bindings(&prior, &bindings);
        cache_updates.push((sub.id.clone(), bindings));
        if !delta.is_empty() {
            matches.push(SubscriptionMatch {
                subscription_id: sub.id.clone(),
                delta,
                mode: sub.mode,
            });
        }
        Ok(())
    }

    /// Cached prior bindings for `id`, or an empty vec when the
    /// subscription has not yet been evaluated.
    fn prior_bindings(&self, id: &NamedNode) -> Result<Vec<Binding>, RegistryError> {
        let guard = self.inner.read().map_err(|_| RegistryError::Poisoned)?;
        Ok(guard.cache.get(id).cloned().unwrap_or_default())
    }
}

/// Build a `Delta::Error` match for a SELECT subscription whose query
/// failed; logs the failure so operators see the underlying cause.
fn select_error_match(
    sub: &Subscription,
    err: &oxigraph::sparql::EvaluationError,
) -> SubscriptionMatch {
    warn!(
        subscription = %sub.id.as_str(),
        error = %err,
        "SELECT subscription evaluation failed"
    );
    SubscriptionMatch {
        subscription_id: sub.id.clone(),
        delta: Delta::Error(err.to_string()),
        mode: sub.mode,
    }
}

fn run_ask(store: &Store, sub: &Subscription, query: &str, matches: &mut Vec<SubscriptionMatch>) {
    match store.query(query) {
        Ok(QueryResults::Boolean(true)) => {
            matches.push(SubscriptionMatch {
                subscription_id: sub.id.clone(),
                delta: Delta::Fired,
                mode: sub.mode,
            });
        }
        Ok(QueryResults::Boolean(false)) => {}
        Ok(_) => matches.push(ask_non_boolean_match(sub)),
        Err(err) => matches.push(ask_error_match(sub, &err)),
    }
}

/// SubscriptionMatch for an ASK that returned a non-boolean (a
/// vocabulary/store-corruption smell). Records a warning so the
/// telemetry pipeline picks it up.
fn ask_non_boolean_match(sub: &Subscription) -> SubscriptionMatch {
    warn!(
        subscription = %sub.id.as_str(),
        "registered ASK subscription returned non-boolean result"
    );
    SubscriptionMatch {
        subscription_id: sub.id.clone(),
        delta: Delta::Error("ASK subscription returned non-boolean result".to_string()),
        mode: sub.mode,
    }
}

/// SubscriptionMatch for an ASK whose store-side evaluation errored
/// out; logs the cause then surfaces it as `Delta::Error`.
fn ask_error_match(
    sub: &Subscription,
    err: &oxigraph::sparql::EvaluationError,
) -> SubscriptionMatch {
    warn!(
        subscription = %sub.id.as_str(),
        error = %err,
        "ASK subscription evaluation failed"
    );
    SubscriptionMatch {
        subscription_id: sub.id.clone(),
        delta: Delta::Error(err.to_string()),
        mode: sub.mode,
    }
}
