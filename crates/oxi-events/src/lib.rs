//! oxi-events — graph-native event substrate for Oxigraph.
//!
//! Stable Dependency Principle (ADR-001): this crate speaks only of
//! mutations, subscriptions, events, and delivery. It MUST NOT depend
//! on `decision-cli` or reference DDD concepts (roles, bundles,
//! sessions, policies, autonomy levels).
//!
//! The slice-1 surface centres on [`GraphWriter`] — the mutation
//! chokepoint over an Oxigraph [`oxigraph::store::Store`]. The writer
//! mints monotonic sequence numbers, drives subscription evaluation
//! via [`SubscriptionRegistry`] (FT-002), and emits events with PROV-O
//! provenance back to the triggering mutation (ADR-002, ADR-004).

#![deny(clippy::unwrap_used)]

pub mod error;
pub mod mutation;
pub mod outbox;
mod quads;
pub mod replay;
pub mod sse;
pub mod subscription;
pub mod vocab;
pub mod writer;

pub use error::{RegistryError, ReplayError, WriterError};
pub use mutation::{CommitResult, EventHandle, Mutation};
pub use outbox::{
    EventEnvelope, OutboxHandle, OutboxPublisher, DEFAULT_OUTBOX_CAPACITY,
    DEFAULT_OUTBOX_POLL_INTERVAL,
};
pub use replay::{replay, ReplayRequest, ReplayedEvent, SparqlFilterFragment};
pub use subscription::{
    Binding, DeliveryHandlerRef, Delta, Subscription, SubscriptionMatch, SubscriptionMode,
    SubscriptionQuery, SubscriptionRegistry, TriggerType,
};
pub use writer::GraphWriter;
