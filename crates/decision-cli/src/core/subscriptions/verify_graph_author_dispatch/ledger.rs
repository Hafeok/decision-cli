//! Persisted dedup ledger for the auto-dispatch subscription (FT-050).
//!
//! One `dec:AutoDispatchLedgerEntry` per `(feature, env)` pair. Stored
//! in the orchestration store's `dec:graph/auto-dispatch-ledger` named
//! graph so it survives process restarts (FT-050 §Invariants:
//! "idempotent under restart"). The subscription consults the ledger
//! before emitting; a row newer than `now - dedup_ttl_seconds` short
//! circuits the dispatch.
//!
//! The ledger is *additive only via the StreamWriter chokepoint*: every
//! write goes through [`StreamWriter::commit`] so ADR-005 inStream
//! tagging applies (the ledger entry is a stream-scoped artifact).

use anyhow::{Context, Result};
use chrono::{DateTime, ParseError, Utc};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    auto_dispatch_ledger_entry_class, auto_dispatch_ledger_graph, last_dispatch_at,
    ledger_environment, ledger_feature,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Errors surfaced by the ledger helpers.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// Underlying SPARQL / store failure.
    #[error("ledger query failed: {0}")]
    Query(String),
    /// Could not write a ledger entry through the StreamWriter.
    #[error("ledger commit failed: {0}")]
    Commit(String),
    /// The stored timestamp is not RFC3339-parseable.
    #[error("ledger timestamp invalid for {feature}/{env}: {detail}")]
    BadTimestamp {
        /// Feature short id.
        feature: String,
        /// Env short id.
        env: String,
        /// Parse-error detail.
        detail: String,
    },
}

/// One in-memory ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    /// IRI of the ledger row (`urn:dec:auto-dispatch-ledger:{feature}:{env}`).
    pub iri: NamedNode,
    /// Short feature id literal.
    pub feature: String,
    /// Short env id literal.
    pub env: String,
    /// RFC3339 timestamp of the latest dispatch.
    pub last_dispatch_at: String,
}

impl LedgerEntry {
    /// Parse `last_dispatch_at` as a `DateTime<Utc>`.
    ///
    /// # Errors
    /// Surfaces a [`LedgerError::BadTimestamp`] when the field is not
    /// RFC3339.
    pub fn parsed_timestamp(&self) -> Result<DateTime<Utc>, LedgerError> {
        parse_rfc3339(&self.last_dispatch_at).map_err(|e| LedgerError::BadTimestamp {
            feature: self.feature.clone(),
            env: self.env.clone(),
            detail: e.to_string(),
        })
    }
}

/// Lookup a single ledger row for `(feature, env)`.
///
/// Returns `Ok(None)` when the pair has never been dispatched. The IRI
/// of the row is deterministic in `(feature, env)`; the lookup uses a
/// SPARQL ASK pattern rather than a graph pattern so it is cheap even
/// against a large orchestration store.
pub fn get_entry(
    store: &Store,
    feature: &str,
    env: &str,
) -> Result<Option<LedgerEntry>, LedgerError> {
    let iri = entry_iri(feature, env);
    let mut feature_lit = String::new();
    let mut env_lit = String::new();
    let mut ts_lit = String::new();
    let mut found = false;
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                auto_dispatch_ledger_graph(),
            )),
        )
        .filter_map(Result::ok)
    {
        found = true;
        match q.predicate.as_str() {
            crate::core::vocab::IRI_DEC_LEDGER_FEATURE => {
                feature_lit = term_literal(&q.object);
            }
            crate::core::vocab::IRI_DEC_LEDGER_ENVIRONMENT => {
                env_lit = term_literal(&q.object);
            }
            crate::core::vocab::IRI_DEC_LAST_DISPATCH_AT => {
                ts_lit = term_literal(&q.object);
            }
            _ => {}
        }
    }
    if !found {
        return Ok(None);
    }
    Ok(Some(LedgerEntry {
        iri,
        feature: feature_lit,
        env: env_lit,
        last_dispatch_at: ts_lit,
    }))
}

/// Record (or refresh) a dispatch in the ledger. Atomically removes any
/// prior entry for the `(feature, env)` pair before inserting the new
/// row so the ledger always reflects the *latest* dispatch only.
pub fn record_dispatch(
    writer: &StreamWriter,
    store: &Store,
    feature: &str,
    env: &str,
    now_rfc3339: &str,
) -> Result<LedgerEntry, LedgerError> {
    // 1. Drop any existing row directly from the store. We bypass the
    //    StreamWriter for removal because the writer is insert-only; the
    //    ledger is a "current snapshot" structure rather than an
    //    append-only log.
    let prior_iri = entry_iri(feature, env);
    let to_remove: Vec<Quad> = store
        .quads_for_pattern(
            Some(Subject::NamedNode(prior_iri.clone()).as_ref()),
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                auto_dispatch_ledger_graph(),
            )),
        )
        .filter_map(Result::ok)
        .collect();
    for q in &to_remove {
        store
            .remove(q.as_ref())
            .map_err(|e| LedgerError::Commit(format!("removing prior ledger row: {e}")))?;
    }

    // 2. Insert the fresh row through the StreamWriter chokepoint.
    let entry = LedgerEntry {
        iri: prior_iri.clone(),
        feature: feature.to_string(),
        env: env.to_string(),
        last_dispatch_at: now_rfc3339.to_string(),
    };
    let quads = entry_quads(&entry);
    let mutation = Mutation::insert(quads.iter().cloned())
        .with_cause(format!("FT-050 record auto-dispatch ledger ({feature}/{env})"));
    writer
        .commit(mutation)
        .map_err(|e| LedgerError::Commit(format!("{e:#}")))?;
    Ok(entry)
}

/// Returns `true` iff `(feature, env)` was dispatched within the last
/// `ttl_seconds`. Used by the subscription handler to decide whether to
/// fire a fresh dispatch event.
///
/// When `ttl_seconds == 0`, dedup is disabled — the function always
/// returns `false` (TC-084 AC #4).
pub fn within_ttl(
    store: &Store,
    feature: &str,
    env: &str,
    ttl_seconds: u64,
    now: DateTime<Utc>,
) -> Result<bool, LedgerError> {
    if ttl_seconds == 0 {
        return Ok(false);
    }
    let Some(entry) = get_entry(store, feature, env)? else {
        return Ok(false);
    };
    let prior = entry.parsed_timestamp()?;
    let elapsed = now.signed_duration_since(prior);
    if elapsed.num_seconds() < 0 {
        // Clock skew — be conservative and treat as still within TTL.
        return Ok(true);
    }
    Ok((elapsed.num_seconds() as u64) < ttl_seconds)
}

/// Test helper — directly age the stored ledger timestamp by overwriting
/// it. Bypasses the StreamWriter so tests can simulate "1 hour has
/// passed" without monkeypatching the clock.
pub fn debug_set_timestamp(
    store: &Store,
    feature: &str,
    env: &str,
    new_rfc3339: &str,
) -> Result<()> {
    let iri = entry_iri(feature, env);
    let g = oxigraph::model::GraphNameRef::NamedNode(auto_dispatch_ledger_graph());
    let last_pred = NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_LAST_DISPATCH_AT);
    let prior: Vec<Quad> = store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            Some(last_pred),
            None,
            Some(g),
        )
        .filter_map(Result::ok)
        .collect();
    for q in &prior {
        store
            .remove(q.as_ref())
            .context("debug_set_timestamp: removing prior")?;
    }
    let g_owned: GraphName = auto_dispatch_ledger_graph().into_owned().into();
    let fresh = Quad::new(
        iri,
        last_pred.into_owned(),
        Literal::new_simple_literal(new_rfc3339),
        g_owned,
    );
    store
        .insert(fresh.as_ref())
        .context("debug_set_timestamp: inserting fresh")?;
    Ok(())
}

/// Deterministic IRI for a `(feature, env)` ledger row.
#[must_use]
pub fn entry_iri(feature: &str, env: &str) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "urn:dec:auto-dispatch-ledger:{feature}:{env}"
    ))
}

fn entry_quads(entry: &LedgerEntry) -> Vec<Quad> {
    let g: GraphName = auto_dispatch_ledger_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    vec![
        Quad::new(
            entry.iri.clone(),
            rdf_type,
            auto_dispatch_ledger_entry_class().into_owned(),
            g.clone(),
        ),
        Quad::new(
            entry.iri.clone(),
            ledger_feature().into_owned(),
            Literal::new_simple_literal(&entry.feature),
            g.clone(),
        ),
        Quad::new(
            entry.iri.clone(),
            ledger_environment().into_owned(),
            Literal::new_simple_literal(&entry.env),
            g.clone(),
        ),
        Quad::new(
            entry.iri.clone(),
            last_dispatch_at().into_owned(),
            Literal::new_simple_literal(&entry.last_dispatch_at),
            g,
        ),
    ]
}

fn term_literal(t: &Term) -> String {
    match t {
        Term::Literal(lit) => lit.value().to_string(),
        Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>, ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}
