//! Value types carried by the subscription registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use oxigraph::model::NamedNode;

use crate::vocab::{SUB_MODE_ASYNC, SUB_MODE_INLINE};

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

/// A subscription registered with the [`super::SubscriptionRegistry`].
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

/// A single subscription firing produced by
/// [`super::SubscriptionRegistry::evaluate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionMatch {
    /// IRI of the subscription that fired.
    pub subscription_id: NamedNode,
    /// Classification of the firing.
    pub delta: Delta,
    /// Mode declared on the subscription at registration time.
    pub mode: SubscriptionMode,
}
