//! Mutation request / commit result types for [`crate::writer::GraphWriter`].
//!
//! Framework vocabulary only — see ADR-001.

use std::collections::{BTreeSet, HashSet};

use oxigraph::model::{GraphName, NamedNode, Quad};

use crate::subscription::TriggerType;

/// A request to mutate the store inside a single transaction.
///
/// Inserts and removes are applied atomically; on failure no event is
/// emitted and no partial state lands in the store.
#[derive(Debug, Clone, Default)]
pub struct Mutation {
    /// Quads to insert. Targets are taken from each quad's `graph_name`.
    pub inserts: Vec<Quad>,
    /// Quads to remove.
    pub removes: Vec<Quad>,
    /// Optional actor IRI (recorded on the mutation node).
    pub actor: Option<NamedNode>,
    /// Free-form cause label (e.g. a CLI verb), recorded as `oxi:cause`.
    pub cause: Option<String>,
    /// Optional caller-supplied commit timestamp (RFC-3339). Defaults to
    /// `now()` when absent.
    pub committed_at: Option<String>,
    /// Trigger-type labels declared by the caller (FT-002). Empty means
    /// "every subscription is considered"; a non-empty set is intersected
    /// against each subscription's declared trigger set to short-circuit
    /// evaluation. Trigger semantics are opaque to `oxi-events` — the
    /// consumer assigns meaning.
    pub triggers: BTreeSet<TriggerType>,
}

impl Mutation {
    /// Build a mutation that only inserts quads.
    #[must_use]
    pub fn insert(quads: impl IntoIterator<Item = Quad>) -> Self {
        Self {
            inserts: quads.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Attach an actor IRI.
    #[must_use]
    pub fn with_actor(mut self, actor: NamedNode) -> Self {
        self.actor = Some(actor);
        self
    }

    /// Attach a cause label.
    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    /// Attach a set of trigger-type labels (FT-002).
    #[must_use]
    pub fn with_triggers<I, S>(mut self, triggers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.triggers = triggers.into_iter().map(Into::into).collect();
        self
    }
}

/// Handle to an event emitted as part of a commit.
#[derive(Debug, Clone)]
pub struct EventHandle {
    /// Canonical IRI of the event entity.
    pub iri: NamedNode,
    /// Monotonic, contiguous sequence number assigned at commit.
    pub sequence: u64,
    /// Subscription whose match produced this event.
    pub subscription: NamedNode,
    /// `ok` / `failed` per FT-001 error-handling rules.
    pub status: String,
}

/// Outcome of a successful commit.
#[derive(Debug, Clone)]
pub struct CommitResult {
    /// IRI of the mutation node (typed `oxi:Mutation`).
    pub mutation_id: NamedNode,
    /// Sequence number minted for the mutation itself (one per mutation).
    pub mutation_sequence: u64,
    /// Named graphs touched by the mutation's inserts/removes.
    pub affected_graphs: HashSet<GraphName>,
    /// Events emitted by subscription evaluation, in seq order.
    pub events: Vec<EventHandle>,
}
