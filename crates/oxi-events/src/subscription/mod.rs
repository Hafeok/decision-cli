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

mod helpers;
mod registry;
mod store_io;
mod types;

pub use registry::SubscriptionRegistry;
pub use types::{
    Binding, DeliveryHandlerRef, Delta, Subscription, SubscriptionMatch, SubscriptionMode,
    SubscriptionQuery, TriggerType,
};
