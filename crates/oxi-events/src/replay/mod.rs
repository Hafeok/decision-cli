//! Replay API (FT-005) — SPARQL-backed event replay over the events graph.
//!
//! Per **ADR-002 (graph-as-state)** there is no separate event log: the
//! events named graph **is** the durable log, so replay is a SPARQL
//! `SELECT` against it, ordered by `oxi:seq`. The API stays inside the
//! [`oxi-events`](crate) SDP boundary (ADR-001): inputs and outputs name
//! only mutations, subscriptions, events, and delivery vocabulary.
//!
//! # Surface
//!
//! [`replay`] is the single entry point. It takes a [`ReplayRequest`]
//! (an inclusive `since_seq` cursor with optional `until_seq`, SPARQL
//! [`SparqlFilterFragment`] and `limit`) and a read handle to an
//! [`oxigraph::store::Store`]. It returns the matching events in
//! seq-ascending order as [`ReplayedEvent`] records.
//!
//! # Invariants (FT-005 §Invariants)
//!
//! * Replay against an unchanged graph is deterministic — the SPARQL
//!   `ORDER BY ?seq` is total because the writer mints monotonic seq
//!   numbers (FT-001 / TC-009).
//! * The output seq sequence is strictly increasing.
//! * Replay never observes events without a corresponding mutation —
//!   the query requires `prov:wasGeneratedBy`, which is set in the same
//!   transaction as the event by the writer (FT-001).
//!
//! # Boundaries
//!
//! * Replay is **read-only**: no quads are inserted, removed, or
//!   touched.
//! * No cross-store federation.
//! * Consumer offsets are the consumer's responsibility (ADR-002): the
//!   caller decides whether to persist the highest seq it has
//!   processed and how to resume.

mod sparql;

use oxigraph::sparql::{Query, QueryResults, QuerySolution};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

use crate::error::ReplayError;

use sparql::{build_query, literal_value, named_node_iri, parse_bool_literal, parse_u64_literal};

/// A SPARQL `FILTER` body fragment, applied to each candidate event row.
///
/// The fragment is the **inside** of a `FILTER( ... )` clause — bare
/// expression syntax such as `?subscription = <urn:my:sub>` or
/// `STRSTARTS(STR(?event), "https://...")`. Variables bound by the
/// replay query are:
///
/// * `?event` — the event IRI,
/// * `?seq` — the sequence number literal,
/// * `?mutation` — the triggering mutation IRI,
/// * `?subscription` — the matched subscription IRI,
/// * `?emittedAt` — the emission timestamp literal,
/// * `?published` — boolean published flag,
/// * `?status` — `"ok"` / `"failed"` simple literal.
///
/// Invalid SPARQL surfaces as [`ReplayError::InvalidFilter`] at request
/// time — the request is rejected before any store read happens (FT-005
/// §Error handling).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparqlFilterFragment(String);

impl SparqlFilterFragment {
    /// Wrap a SPARQL filter body. Validation is deferred to [`replay`].
    #[must_use]
    pub fn new(body: impl Into<String>) -> Self {
        Self(body.into())
    }

    /// Borrow the underlying SPARQL fragment.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SparqlFilterFragment {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SparqlFilterFragment {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// A single replay request. All fields default to "no bound".
///
/// Slice 1 semantics:
///
/// * `since_seq` is **inclusive**: events with `?seq >= since_seq` match.
///   Set to `0` to replay from the beginning.
/// * `until_seq` is **inclusive** when present; absent means open-ended.
/// * `filter` is applied as a SPARQL `FILTER` clause (see
///   [`SparqlFilterFragment`]).
/// * `limit` caps the number of rows returned; absent means unbounded.
///
/// The request is `Clone` and `Serialize` so callers can ship it across
/// process boundaries (e.g. CLI → daemon).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRequest {
    /// Inclusive lower seq bound.
    pub since_seq: u64,
    /// Inclusive upper seq bound (`None` = open-ended).
    pub until_seq: Option<u64>,
    /// Optional SPARQL `FILTER` body fragment.
    pub filter: Option<SparqlFilterFragment>,
    /// Optional cap on the number of rows returned.
    pub limit: Option<usize>,
}

impl ReplayRequest {
    /// Build a request that replays from `since_seq` onward, with no
    /// upper bound, filter, or limit.
    #[must_use]
    pub fn since(since_seq: u64) -> Self {
        Self {
            since_seq,
            ..Self::default()
        }
    }

    /// Bound the request with an inclusive upper seq.
    #[must_use]
    pub fn with_until(mut self, until_seq: u64) -> Self {
        self.until_seq = Some(until_seq);
        self
    }

    /// Attach a SPARQL `FILTER` fragment.
    #[must_use]
    pub fn with_filter(mut self, filter: impl Into<SparqlFilterFragment>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Cap the number of rows returned.
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// An event surfaced by [`replay`].
///
/// Carries every framework-vocabulary field persisted on an `oxi:Event`
/// node so consumers do not need to issue follow-up SPARQL just to read
/// the basics. ADR-001 keeps this struct strictly framework-vocabulary
/// — no DDD concepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayedEvent {
    /// IRI of the persisted `oxi:Event`.
    pub event: String,
    /// Monotonic sequence number minted at commit time.
    pub seq: u64,
    /// IRI of the triggering mutation (`prov:wasGeneratedBy`).
    pub mutation: String,
    /// IRI of the subscription whose match produced this event.
    pub subscription: String,
    /// RFC-3339 emission timestamp.
    #[serde(rename = "emittedAt")]
    pub emitted_at: String,
    /// Whether the outbox has delivered this event.
    pub published: bool,
    /// `"ok"` or `"failed"` (FT-001 §Error handling).
    pub status: String,
}

/// Replay events from `store` according to `request`.
///
/// Returns events in **strictly seq-ascending** order. The call is
/// read-only (FT-005 §Boundaries) and deterministic against an
/// unchanged graph (FT-005 §Invariants).
///
/// # Errors
///
/// * [`ReplayError::InvalidFilter`] if the request carries a filter
///   fragment whose SPARQL fails to parse.
/// * [`ReplayError::Store`] for store-side read errors.
/// * [`ReplayError::Internal`] if a returned solution is malformed
///   (missing field or unparseable literal). Indicates store
///   corruption — not a recoverable error condition.
pub fn replay(store: &Store, request: &ReplayRequest) -> Result<Vec<ReplayedEvent>, ReplayError> {
    let sparql = prepare_replay_query(request)?;
    let sols = execute_replay_query(store, &sparql)?;

    let mut out = Vec::new();
    let mut last_seq: Option<u64> = None;
    for sol in sols {
        let sol = sol?;
        let event = decode_replay_row(&sol)?;
        enforce_strict_monotonic(last_seq, event.seq)?;
        last_seq = Some(event.seq);
        out.push(event);
    }

    Ok(out)
}

fn prepare_replay_query(request: &ReplayRequest) -> Result<String, ReplayError> {
    let sparql = build_query(request);
    if let Err(err) = Query::parse(&sparql, None) {
        return Err(match &request.filter {
            Some(_) => ReplayError::InvalidFilter(err.to_string()),
            None => ReplayError::Internal(format!("replay query failed to parse: {err}")),
        });
    }
    Ok(sparql)
}

fn execute_replay_query(
    store: &Store,
    sparql: &str,
) -> Result<oxigraph::sparql::QuerySolutionIter, ReplayError> {
    let results = store.query(sparql)?;
    match results {
        QueryResults::Solutions(sols) => Ok(sols),
        _ => Err(ReplayError::Internal(
            "replay SELECT returned non-solution results".to_string(),
        )),
    }
}

fn decode_replay_row(sol: &QuerySolution) -> Result<ReplayedEvent, ReplayError> {
    let event = require_field(sol, "event")?;
    let seq_term = require_field(sol, "seq")?;
    let mutation = require_field(sol, "mutation")?;
    let subscription = require_field(sol, "subscription")?;
    let emitted_at = require_field(sol, "emittedAt")?;
    let published = require_field(sol, "published")?;
    let status = require_field(sol, "status")?;
    Ok(ReplayedEvent {
        event: named_node_iri(event, "event")?,
        seq: parse_u64_literal(seq_term, "seq")?,
        mutation: named_node_iri(mutation, "mutation")?,
        subscription: named_node_iri(subscription, "subscription")?,
        emitted_at: literal_value(emitted_at, "emittedAt")?,
        published: parse_bool_literal(published, "published")?,
        status: literal_value(status, "status")?,
    })
}

fn require_field<'a>(
    sol: &'a QuerySolution,
    name: &str,
) -> Result<&'a oxigraph::model::Term, ReplayError> {
    sol.get(name)
        .ok_or_else(|| ReplayError::Internal(format!("replay row missing ?{name}")))
}

fn enforce_strict_monotonic(prev: Option<u64>, seq: u64) -> Result<(), ReplayError> {
    // Defence-in-depth: re-assert strict-monotonic so a future
    // schema mistake surfaces as a clear error.
    if let Some(prev) = prev {
        if seq <= prev {
            return Err(ReplayError::Internal(format!(
                "replay output not strictly increasing: {prev} followed by {seq}"
            )));
        }
    }
    Ok(())
}
