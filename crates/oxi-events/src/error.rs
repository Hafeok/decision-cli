//! Typed errors for the `oxi-events` writer surface.

use oxigraph::sparql::{EvaluationError, SparqlSyntaxError};
use oxigraph::store::StorageError;
use thiserror::Error;

/// Errors emitted by [`crate::writer::GraphWriter`] and its helpers.
#[derive(Debug, Error)]
pub enum WriterError {
    /// Oxigraph store-level failure (transactions, persistence).
    #[error("oxigraph store error: {0}")]
    Store(#[from] StorageError),

    /// SPARQL query/update evaluation failure.
    #[error("sparql evaluation error: {0}")]
    Sparql(#[from] EvaluationError),

    /// SPARQL query/update syntax failure.
    #[error("sparql syntax error: {0}")]
    SparqlSyntax(#[from] SparqlSyntaxError),

    /// Sequence-counter persistence failed; commit aborted.
    #[error("sequence counter persistence failed: {0}")]
    Sequence(String),

    /// Subscription registry operation failed.
    #[error("subscription registry error: {0}")]
    Registry(#[from] RegistryError),

    /// Internal invariant violated.
    #[error("internal invariant violated: {0}")]
    Internal(String),
}

/// Errors emitted by the FT-005 replay surface ([`crate::replay::replay`]).
///
/// Replay is read-only and stateless; failures fall into three buckets:
/// caller-supplied filter SPARQL that fails to parse, store-side read
/// errors, and store-corruption signals (malformed persisted rows).
#[derive(Debug, Error)]
pub enum ReplayError {
    /// Caller supplied a SPARQL filter fragment that does not parse.
    /// Surfaced at request time, before any store read happens (FT-005
    /// §Error handling).
    #[error("invalid SPARQL filter fragment: {0}")]
    InvalidFilter(String),

    /// Oxigraph store-level failure (read transactions, persistence).
    #[error("oxigraph store error: {0}")]
    Store(#[from] StorageError),

    /// SPARQL evaluation error during replay.
    #[error("sparql evaluation error: {0}")]
    Sparql(#[from] EvaluationError),

    /// Store contained data that violated replay's read invariants
    /// (missing field on an event row, malformed literal, non-monotonic
    /// seq). Indicates store corruption — not a recoverable error.
    #[error("internal invariant violated: {0}")]
    Internal(String),
}

/// Errors surfaced by [`crate::subscription::SubscriptionRegistry`].
///
/// Per FT-002 the registry classifies errors into structured cases so
/// the writer can decide whether to abort a commit or merely surface a
/// per-subscription `Delta::Error` (the latter is the default — a bad
/// subscription must not block the mutation chokepoint).
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Caller passed a SPARQL query that fails to parse. Returned at
    /// registration time (FT-002 §Error handling).
    #[error("invalid SPARQL query for subscription {subscription}: {source}")]
    InvalidQuery {
        /// IRI of the subscription whose query failed to parse.
        subscription: String,
        /// Underlying parser error.
        #[source]
        source: SparqlSyntaxError,
    },

    /// A subscription with this id is already registered.
    #[error("subscription {0} already registered")]
    DuplicateSubscription(String),

    /// No subscription with this id exists.
    #[error("subscription {0} is not registered")]
    SubscriptionNotFound(String),

    /// Internal `RwLock` got poisoned by a panic on another thread.
    #[error("subscription registry lock poisoned")]
    Poisoned,

    /// Oxigraph store error encountered while loading subscriptions.
    #[error("oxigraph store error: {0}")]
    Store(#[from] StorageError),

    /// SPARQL evaluation failed while loading subscriptions.
    #[error("sparql evaluation error: {0}")]
    Sparql(#[from] EvaluationError),

    /// Persisted subscription record was malformed (missing fields,
    /// unknown mode, etc).
    #[error("malformed persisted subscription {subscription}: {reason}")]
    MalformedPersisted {
        /// IRI of the offending subscription record.
        subscription: String,
        /// Why the record could not be parsed.
        reason: String,
    },
}
