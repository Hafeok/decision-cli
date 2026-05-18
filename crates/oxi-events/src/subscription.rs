//! Subscription registry and evaluator (FT-002).
//!
//! A [`Subscription`] is a first-class graph artifact (ADR-003) carrying:
//!
//! * a stable IRI (`id`),
//! * a SPARQL query — either an `ASK` predicate or a `SELECT` that the
//!   evaluator diffs against a cached prior result set,
//! * a declared set of opaque trigger-type labels (FT-002 §Inputs),
//! * a delivery mode (`Inline` | `Async`), and
//! * an optional handler reference (FT-003/FT-004 deliver; this crate only
//!   carries the reference forward).
//!
//! On commit the registry is given the post-commit store and the trigger
//! set declared by the mutation. For each registered subscription:
//!
//! 1. Skip if the subscription declares triggers and they do not
//!    intersect the mutation's triggers (FT-002 §Behaviour step 2).
//! 2. Evaluate the SPARQL query against the post-commit snapshot
//!    (step 3).
//! 3. For `SELECT` queries, diff the bindings against the cache and emit
//!    a [`SubscriptionMatch`] iff the delta is non-empty (steps 4-5).
//!    `ASK` queries fire on every `true` (slice 1 semantics — TC-009).
//! 4. A SPARQL evaluation error is isolated to that subscription as
//!    [`Delta::Error`] — the commit itself is not blocked (FT-002
//!    §Error handling).
//!
//! The in-memory map mirrors the `subscriptions` named graph; the graph
//! is the source of truth (ADR-003). [`SubscriptionRegistry::load_from_store`]
//! rebuilds the mirror at startup (FT-002 §Behaviour step 1, used by
//! FT-009 `OrchestrationStore::open`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::RwLock;

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::{Query, QueryResults};
use oxigraph::store::Store;
use tracing::warn;

use crate::error::RegistryError;
use crate::vocab::{
    IRI_OXI_GRAPH_SUBSCRIPTIONS, IRI_OXI_SUBSCRIPTION, IRI_OXI_SUB_ASK_QUERY, IRI_OXI_SUB_HANDLER,
    IRI_OXI_SUB_MODE, IRI_OXI_SUB_SELECT_QUERY, IRI_OXI_SUB_TRIGGER, SUB_MODE_ASYNC,
    SUB_MODE_INLINE,
};

/// Opaque trigger-type label. `oxi-events` ascribes no meaning to these
/// strings — consumers map them to their own taxonomy (FT-002
/// §Boundaries).
pub type TriggerType = String;

/// A single solution of a `SELECT` query, keyed by variable name with
/// canonicalised RDF-term values. Kept as a `BTreeMap` so equality and
/// hashing are deterministic across runs.
pub type Binding = BTreeMap<String, String>;

/// Delivery handler reference. The registry never invokes the handler —
/// it just carries the identifier forward for FT-003 / FT-004 to bind to
/// a concrete transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryHandlerRef(pub String);

impl DeliveryHandlerRef {
    /// Build a handler ref from any string-like value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the reference as a `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Delivery mode for a [`Subscription`]. Slice 1 carries the
/// classification through the substrate; FT-003 acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscriptionMode {
    /// Synchronous delivery: the handler runs on the writer's thread.
    #[default]
    Inline,
    /// Asynchronous delivery: the handler is invoked via the outbox.
    Async,
}

impl SubscriptionMode {
    /// Persistence label (`"inline"` or `"async"`).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inline => SUB_MODE_INLINE,
            Self::Async => SUB_MODE_ASYNC,
        }
    }

    /// Parse a persistence label back into a mode.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            SUB_MODE_INLINE => Ok(Self::Inline),
            SUB_MODE_ASYNC => Ok(Self::Async),
            other => Err(format!("unknown subscription mode: {other}")),
        }
    }
}

impl fmt::Display for SubscriptionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// SPARQL query carried by a [`Subscription`].
///
/// * [`SubscriptionQuery::Ask`] — boolean predicate; fires on `true`.
/// * [`SubscriptionQuery::Select`] — solution set; fires on non-empty
///   diff against the registry's cached prior result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionQuery {
    /// ASK predicate body (e.g. `"ASK { ?s ?p ?o }"`).
    Ask(String),
    /// SELECT body (e.g. `"SELECT ?s WHERE { ?s a foo:Bar }"`).
    Select(String),
}

impl SubscriptionQuery {
    /// Borrow the underlying SPARQL string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ask(q) | Self::Select(q) => q,
        }
    }

    /// Returns `true` for [`Self::Ask`].
    #[must_use]
    pub fn is_ask(&self) -> bool {
        matches!(self, Self::Ask(_))
    }
}

/// A subscription registered with the [`SubscriptionRegistry`].
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Stable IRI identifying the subscription.
    pub id: NamedNode,
    /// Human-readable label (recorded on emitted events).
    pub label: Option<String>,
    /// SPARQL query (ASK or SELECT) — see [`SubscriptionQuery`].
    pub query: SubscriptionQuery,
    /// Declared trigger types. Empty means "fires on every commit"
    /// (after the registry's intersection check).
    pub triggers: BTreeSet<TriggerType>,
    /// Delivery mode.
    pub mode: SubscriptionMode,
    /// Optional delivery handler reference.
    pub handler: Option<DeliveryHandlerRef>,
}

impl Subscription {
    /// Build a subscription with a stable id and an `ASK` trigger.
    ///
    /// Preserved for slice 1 callers (TC-009, FT-009 bootstrap). To
    /// declare a SELECT subscription with explicit triggers or async
    /// delivery, chain the `with_*` builders.
    #[must_use]
    pub fn new(id: NamedNode, trigger_ask: impl Into<String>) -> Self {
        Self {
            id,
            label: None,
            query: SubscriptionQuery::Ask(trigger_ask.into()),
            triggers: BTreeSet::new(),
            mode: SubscriptionMode::default(),
            handler: None,
        }
    }

    /// Build a subscription with a `SELECT` query as the matcher.
    #[must_use]
    pub fn select(id: NamedNode, query: impl Into<String>) -> Self {
        Self {
            id,
            label: None,
            query: SubscriptionQuery::Select(query.into()),
            triggers: BTreeSet::new(),
            mode: SubscriptionMode::default(),
            handler: None,
        }
    }

    /// Attach a human-readable label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Declare the trigger-type labels this subscription is interested
    /// in. Empty means "every commit is a candidate".
    #[must_use]
    pub fn with_triggers<I, S>(mut self, triggers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.triggers = triggers.into_iter().map(Into::into).collect();
        self
    }

    /// Set the delivery mode.
    #[must_use]
    pub fn with_mode(mut self, mode: SubscriptionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Attach a delivery handler reference.
    #[must_use]
    pub fn with_handler(mut self, handler: impl Into<DeliveryHandlerRef>) -> Self {
        self.handler = Some(handler.into());
        self
    }

    /// Borrow the SPARQL string for evaluation.
    #[must_use]
    pub fn trigger_ask(&self) -> Option<&str> {
        match &self.query {
            SubscriptionQuery::Ask(q) => Some(q.as_str()),
            SubscriptionQuery::Select(_) => None,
        }
    }
}

impl From<String> for DeliveryHandlerRef {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for DeliveryHandlerRef {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// Delta classification for a [`SubscriptionMatch`]. FT-002 §Outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// An `ASK` subscription evaluated to `true` on this commit.
    Fired,
    /// New bindings appeared since the prior evaluation.
    Added(Vec<Binding>),
    /// Bindings disappeared since the prior evaluation.
    Removed(Vec<Binding>),
    /// Mixed delta — both added and removed bindings.
    Changed {
        /// Bindings that appeared in this evaluation.
        added: Vec<Binding>,
        /// Bindings that vanished in this evaluation.
        removed: Vec<Binding>,
    },
    /// SPARQL evaluation failed for this subscription. The commit is not
    /// aborted (FT-002 §Error handling).
    Error(String),
}

impl Delta {
    /// `true` iff the delta carries no signal (would be filtered out).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Added(v) | Self::Removed(v) => v.is_empty(),
            Self::Changed { added, removed } => added.is_empty() && removed.is_empty(),
            Self::Fired | Self::Error(_) => false,
        }
    }

    /// Short label used on persisted `oxi:Event` records.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fired => "fired",
            Self::Added(_) => "added",
            Self::Removed(_) => "removed",
            Self::Changed { .. } => "changed",
            Self::Error(_) => "error",
        }
    }

    /// `true` iff the delta is an [`Self::Error`].
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// A single subscription firing produced by [`SubscriptionRegistry::evaluate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionMatch {
    /// IRI of the subscription that fired.
    pub subscription_id: NamedNode,
    /// Classification of the firing.
    pub delta: Delta,
    /// Mode declared on the subscription at registration time.
    pub mode: SubscriptionMode,
}

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
    ///
    /// `mutation_triggers` is the set declared by the mutation; a
    /// subscription whose declared trigger set is disjoint is skipped
    /// (a subscription that declares no triggers is always considered).
    /// An empty `mutation_triggers` set means "no trigger metadata" and
    /// is treated as compatible with every subscription (this is what
    /// TC-009 exercises).
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
                SubscriptionQuery::Ask(q) => match store.query(q.as_str()) {
                    Ok(QueryResults::Boolean(true)) => {
                        matches.push(SubscriptionMatch {
                            subscription_id: sub.id.clone(),
                            delta: Delta::Fired,
                            mode: sub.mode,
                        });
                    }
                    Ok(QueryResults::Boolean(false)) => {}
                    Ok(_) => {
                        warn!(
                            subscription = %sub.id.as_str(),
                            "registered ASK subscription returned non-boolean result"
                        );
                        matches.push(SubscriptionMatch {
                            subscription_id: sub.id.clone(),
                            delta: Delta::Error(
                                "ASK subscription returned non-boolean result".to_string(),
                            ),
                            mode: sub.mode,
                        });
                    }
                    Err(err) => {
                        warn!(
                            subscription = %sub.id.as_str(),
                            error = %err,
                            "ASK subscription evaluation failed"
                        );
                        matches.push(SubscriptionMatch {
                            subscription_id: sub.id.clone(),
                            delta: Delta::Error(err.to_string()),
                            mode: sub.mode,
                        });
                    }
                },
                SubscriptionQuery::Select(q) => {
                    let bindings = match collect_bindings(store, q.as_str()) {
                        Ok(b) => b,
                        Err(err) => {
                            warn!(
                                subscription = %sub.id.as_str(),
                                error = %err,
                                "SELECT subscription evaluation failed"
                            );
                            matches.push(SubscriptionMatch {
                                subscription_id: sub.id.clone(),
                                delta: Delta::Error(err.to_string()),
                                mode: sub.mode,
                            });
                            continue;
                        }
                    };
                    let prior = {
                        let guard = self.inner.read().map_err(|_| RegistryError::Poisoned)?;
                        guard.cache.get(&sub.id).cloned().unwrap_or_default()
                    };
                    let delta = diff_bindings(&prior, &bindings);
                    cache_updates.push((sub.id.clone(), bindings));
                    if !delta.is_empty() {
                        matches.push(SubscriptionMatch {
                            subscription_id: sub.id.clone(),
                            delta,
                            mode: sub.mode,
                        });
                    }
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
}

fn triggers_compatible(
    sub_triggers: &BTreeSet<TriggerType>,
    mutation_triggers: &BTreeSet<TriggerType>,
) -> bool {
    if sub_triggers.is_empty() || mutation_triggers.is_empty() {
        return true;
    }
    !sub_triggers.is_disjoint(mutation_triggers)
}

fn validate_query(id: &NamedNode, query: &SubscriptionQuery) -> Result<(), RegistryError> {
    Query::parse(query.as_str(), None).map_err(|source| RegistryError::InvalidQuery {
        subscription: id.as_str().to_string(),
        source,
    })?;
    Ok(())
}

fn collect_bindings(
    store: &Store,
    query: &str,
) -> Result<Vec<Binding>, oxigraph::sparql::EvaluationError> {
    let results = store.query(query)?;
    let mut out: Vec<Binding> = Vec::new();
    let QueryResults::Solutions(sols) = results else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        let mut binding: Binding = BTreeMap::new();
        for (var, term) in sol.iter() {
            binding.insert(var.as_str().to_string(), term_repr(term));
        }
        out.push(binding);
    }
    Ok(out)
}

fn term_repr(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => format!("<{}>", n.as_str()),
        Term::BlankNode(b) => format!("_:{}", b.as_str()),
        Term::Literal(l) => l.to_string(),
        Term::Triple(t) => format!("<<{}>>", t.as_ref()),
    }
}

fn diff_bindings(prior: &[Binding], current: &[Binding]) -> Delta {
    let prior_set: BTreeSet<&Binding> = prior.iter().collect();
    let current_set: BTreeSet<&Binding> = current.iter().collect();
    let added: Vec<Binding> = current_set
        .difference(&prior_set)
        .copied()
        .cloned()
        .collect();
    let removed: Vec<Binding> = prior_set
        .difference(&current_set)
        .copied()
        .cloned()
        .collect();
    match (added.is_empty(), removed.is_empty()) {
        (true, true) => Delta::Added(Vec::new()),
        (false, true) => Delta::Added(added),
        (true, false) => Delta::Removed(removed),
        (false, false) => Delta::Changed { added, removed },
    }
}

fn read_persisted_subscriptions(store: &Store) -> Result<Vec<Subscription>, RegistryError> {
    let graph = IRI_OXI_GRAPH_SUBSCRIPTIONS;
    let cls = IRI_OXI_SUBSCRIPTION;
    let ask_pred = IRI_OXI_SUB_ASK_QUERY;
    let select_pred = IRI_OXI_SUB_SELECT_QUERY;
    let mode_pred = IRI_OXI_SUB_MODE;
    let handler_pred = IRI_OXI_SUB_HANDLER;
    let label_pred = "http://www.w3.org/2000/01/rdf-schema#label";

    let q = format!(
        "SELECT ?sub ?ask ?select ?mode ?handler ?label FROM <{graph}> WHERE {{ \
            ?sub a <{cls}> . \
            OPTIONAL {{ ?sub <{ask_pred}> ?ask }} \
            OPTIONAL {{ ?sub <{select_pred}> ?select }} \
            OPTIONAL {{ ?sub <{mode_pred}> ?mode }} \
            OPTIONAL {{ ?sub <{handler_pred}> ?handler }} \
            OPTIONAL {{ ?sub <{label_pred}> ?label }} \
         }}"
    );

    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        let Some(Term::NamedNode(id)) = sol.get("sub").cloned() else {
            continue;
        };
        let id_str = id.as_str().to_string();
        let query = match (sol.get("ask").cloned(), sol.get("select").cloned()) {
            (Some(Term::Literal(lit)), _) => SubscriptionQuery::Ask(lit.value().to_string()),
            (_, Some(Term::Literal(lit))) => SubscriptionQuery::Select(lit.value().to_string()),
            _ => {
                return Err(RegistryError::MalformedPersisted {
                    subscription: id_str,
                    reason: "missing oxi:askQuery and oxi:selectQuery".to_string(),
                });
            }
        };
        let mode = match sol.get("mode").cloned() {
            Some(Term::Literal(lit)) => SubscriptionMode::parse(lit.value()).map_err(|reason| {
                RegistryError::MalformedPersisted {
                    subscription: id_str.clone(),
                    reason,
                }
            })?,
            _ => SubscriptionMode::default(),
        };
        let handler = match sol.get("handler").cloned() {
            Some(Term::Literal(lit)) => Some(DeliveryHandlerRef::new(lit.value().to_string())),
            Some(Term::NamedNode(n)) => Some(DeliveryHandlerRef::new(n.as_str().to_string())),
            _ => None,
        };
        let label = match sol.get("label").cloned() {
            Some(Term::Literal(lit)) => Some(lit.value().to_string()),
            _ => None,
        };
        let triggers = read_persisted_triggers(store, &id)?;
        out.push(Subscription {
            id,
            label,
            query,
            triggers,
            mode,
            handler,
        });
    }
    Ok(out)
}

fn read_persisted_triggers(
    store: &Store,
    sub_id: &NamedNode,
) -> Result<BTreeSet<TriggerType>, RegistryError> {
    let graph = IRI_OXI_GRAPH_SUBSCRIPTIONS;
    let trigger_pred = IRI_OXI_SUB_TRIGGER;
    let q = format!(
        "SELECT ?t FROM <{graph}> WHERE {{ <{sub}> <{trigger_pred}> ?t }}",
        sub = sub_id.as_str()
    );
    let mut out = BTreeSet::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        if let Some(Term::Literal(lit)) = sol.get("t").cloned() {
            out.insert(lit.value().to_string());
        }
    }
    Ok(out)
}
