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

use std::fmt::Write as _;

use oxigraph::sparql::{Query, QueryResults};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

use crate::error::ReplayError;
use crate::vocab::{
    IRI_OXI_EMITTED_AT, IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS, IRI_OXI_MATCHED_SUBSCRIPTION,
    IRI_OXI_PUBLISHED, IRI_OXI_SEQ, IRI_OXI_STATUS, IRI_PROV_WAS_GENERATED_BY,
};

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
    let sparql = build_query(request);

    // Pre-parse the query so an invalid filter is reported as
    // ReplayError::InvalidFilter rather than a store-side evaluation
    // error. FT-005 §Error handling requires the filter check to happen
    // "at request time" — i.e. before we read the store.
    if let Err(err) = Query::parse(&sparql, None) {
        return Err(match &request.filter {
            Some(_) => ReplayError::InvalidFilter(err.to_string()),
            None => ReplayError::Internal(format!("replay query failed to parse: {err}")),
        });
    }

    let results = store.query(sparql.as_str())?;
    let QueryResults::Solutions(sols) = results else {
        return Err(ReplayError::Internal(
            "replay SELECT returned non-solution results".to_string(),
        ));
    };

    let mut out = Vec::new();
    let mut last_seq: Option<u64> = None;
    for sol in sols {
        let sol = sol?;
        let event = sol
            .get("event")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?event".to_string()))?;
        let seq_term = sol
            .get("seq")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?seq".to_string()))?;
        let mutation = sol
            .get("mutation")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?mutation".to_string()))?;
        let subscription = sol.get("subscription").ok_or_else(|| {
            ReplayError::Internal("replay row missing ?subscription".to_string())
        })?;
        let emitted_at = sol
            .get("emittedAt")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?emittedAt".to_string()))?;
        let published = sol
            .get("published")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?published".to_string()))?;
        let status = sol
            .get("status")
            .ok_or_else(|| ReplayError::Internal("replay row missing ?status".to_string()))?;

        let event_iri = named_node_iri(event, "event")?;
        let seq = parse_u64_literal(seq_term, "seq")?;
        let mutation_iri = named_node_iri(mutation, "mutation")?;
        let subscription_iri = named_node_iri(subscription, "subscription")?;
        let emitted_at_str = literal_value(emitted_at, "emittedAt")?;
        let published_bool = parse_bool_literal(published, "published")?;
        let status_str = literal_value(status, "status")?;

        // Defence-in-depth: SPARQL ORDER BY guarantees ascending, but
        // re-assert the strict-monotonic invariant so a future schema
        // mistake (e.g. duplicate seq) surfaces as a clear error rather
        // than as silently mis-ordered output (FT-005 §Invariants).
        if let Some(prev) = last_seq {
            if seq <= prev {
                return Err(ReplayError::Internal(format!(
                    "replay output not strictly increasing: {prev} followed by {seq}"
                )));
            }
        }
        last_seq = Some(seq);

        out.push(ReplayedEvent {
            event: event_iri,
            seq,
            mutation: mutation_iri,
            subscription: subscription_iri,
            emitted_at: emitted_at_str,
            published: published_bool,
            status: status_str,
        });
    }

    Ok(out)
}

fn build_query(request: &ReplayRequest) -> String {
    let mut q = String::with_capacity(512);
    q.push_str("SELECT ?event ?seq ?mutation ?subscription ?emittedAt ?published ?status FROM <");
    q.push_str(IRI_OXI_GRAPH_EVENTS);
    q.push_str("> WHERE { ?event a <");
    q.push_str(IRI_OXI_EVENT);
    q.push_str("> ; <");
    q.push_str(IRI_OXI_SEQ);
    q.push_str("> ?seq ; <");
    q.push_str(IRI_PROV_WAS_GENERATED_BY);
    q.push_str("> ?mutation ; <");
    q.push_str(IRI_OXI_MATCHED_SUBSCRIPTION);
    q.push_str("> ?subscription ; <");
    q.push_str(IRI_OXI_EMITTED_AT);
    q.push_str("> ?emittedAt ; <");
    q.push_str(IRI_OXI_PUBLISHED);
    q.push_str("> ?published ; <");
    q.push_str(IRI_OXI_STATUS);
    q.push_str("> ?status .");

    // since_seq is inclusive; omit the filter when 0 to keep the query
    // compact and the optimiser happy.
    if request.since_seq > 0 {
        let _ = write!(q, " FILTER(?seq >= {}) ", request.since_seq);
    }
    if let Some(until) = request.until_seq {
        let _ = write!(q, " FILTER(?seq <= {until}) ");
    }
    if let Some(filter) = request.filter.as_ref() {
        q.push_str(" FILTER(");
        q.push_str(filter.as_str());
        q.push_str(") ");
    }
    q.push_str("} ORDER BY ?seq");
    if let Some(limit) = request.limit {
        let _ = write!(q, " LIMIT {limit}");
    }
    q
}

fn named_node_iri(term: &oxigraph::model::Term, name: &str) -> Result<String, ReplayError> {
    match term {
        oxigraph::model::Term::NamedNode(n) => Ok(n.as_str().to_string()),
        _ => Err(ReplayError::Internal(format!(
            "replay row ?{name} is not a named node"
        ))),
    }
}

fn literal_value(term: &oxigraph::model::Term, name: &str) -> Result<String, ReplayError> {
    match term {
        oxigraph::model::Term::Literal(lit) => Ok(lit.value().to_string()),
        _ => Err(ReplayError::Internal(format!(
            "replay row ?{name} is not a literal"
        ))),
    }
}

fn parse_u64_literal(term: &oxigraph::model::Term, name: &str) -> Result<u64, ReplayError> {
    let s = literal_value(term, name)?;
    s.parse::<u64>()
        .map_err(|e| ReplayError::Internal(format!("replay row ?{name} parse u64: {e}")))
}

fn parse_bool_literal(term: &oxigraph::model::Term, name: &str) -> Result<bool, ReplayError> {
    let s = literal_value(term, name)?;
    match s.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(ReplayError::Internal(format!(
            "replay row ?{name} unrecognised boolean literal: {other}"
        ))),
    }
}
