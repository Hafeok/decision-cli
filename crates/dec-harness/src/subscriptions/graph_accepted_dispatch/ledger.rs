//! Dedup ledger for the graph-accepted auto-dispatch subscription (FT-100).
//!
//! Keyed on `(graph_iri, env_iri)`. One row per pair. Stored in the
//! `dec:graph/graph-accepted-ledger` named graph so it survives restart.

use chrono::{DateTime, ParseError, Utc};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;
use sha2::{Digest, Sha256};
use thiserror::Error;

use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::{
    graph_accepted_ledger_entry_class, graph_accepted_ledger_graph, last_dispatch_at,
    ledger_environment, ledger_graph_pred,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Errors surfaced by the ledger helpers.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// SPARQL / store failure.
    #[error("graph-accepted ledger query failed: {0}")]
    Query(String),
    /// Could not write a ledger entry through the StreamWriter.
    #[error("graph-accepted ledger commit failed: {0}")]
    Commit(String),
    /// Stored timestamp not RFC3339.
    #[error("graph-accepted ledger timestamp invalid for {graph}/{env}: {detail}")]
    BadTimestamp {
        /// Graph IRI.
        graph: String,
        /// Env IRI / short id.
        env: String,
        /// Parse-error detail.
        detail: String,
    },
}

/// One ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    /// IRI of the ledger row.
    pub iri: NamedNode,
    /// Graph IRI literal.
    pub graph: String,
    /// Env IRI literal.
    pub env: String,
    /// RFC3339 timestamp of the latest dispatch.
    pub last_dispatch_at: String,
}

impl LedgerEntry {
    /// Parse `last_dispatch_at` as a `DateTime<Utc>`.
    ///
    /// # Errors
    /// Returns [`LedgerError::BadTimestamp`] when the field is not RFC3339.
    pub fn parsed_timestamp(&self) -> Result<DateTime<Utc>, LedgerError> {
        parse_rfc3339(&self.last_dispatch_at).map_err(|e| LedgerError::BadTimestamp {
            graph: self.graph.clone(),
            env: self.env.clone(),
            detail: e.to_string(),
        })
    }
}

/// Deterministic IRI for a `(graph, env)` ledger row. Uses a SHA-256
/// over the concatenated keys so the IRI does not embed arbitrarily
/// long URLs.
#[must_use]
pub fn entry_iri(graph: &str, env: &str) -> NamedNode {
    let mut h = Sha256::new();
    h.update(graph.as_bytes());
    h.update(b"\0");
    h.update(env.as_bytes());
    let digest = h.finalize();
    let hex: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    NamedNode::new_unchecked(format!("urn:dec:graph-accepted-ledger:{hex}"))
}

/// Look up a row for `(graph, env)`. Returns `Ok(None)` when none exists.
pub fn get_entry(
    store: &Store,
    graph: &str,
    env: &str,
) -> Result<Option<LedgerEntry>, LedgerError> {
    let iri = entry_iri(graph, env);
    let mut acc = LedgerAccumulator::default();
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                graph_accepted_ledger_graph(),
            )),
        )
        .filter_map(Result::ok)
    {
        acc.absorb(&q);
    }
    Ok(acc.into_entry(iri))
}

#[derive(Default)]
struct LedgerAccumulator {
    graph: String,
    env: String,
    last_dispatch_at: String,
    found: bool,
}

impl LedgerAccumulator {
    fn absorb(&mut self, q: &Quad) {
        self.found = true;
        match q.predicate.as_str() {
            dec_ontology::vocab::IRI_DEC_LEDGER_GRAPH => {
                self.graph = term_literal(&q.object);
            }
            dec_ontology::vocab::IRI_DEC_LEDGER_ENVIRONMENT => {
                self.env = term_literal(&q.object);
            }
            dec_ontology::vocab::IRI_DEC_LAST_DISPATCH_AT => {
                self.last_dispatch_at = term_literal(&q.object);
            }
            _ => {}
        }
    }

    fn into_entry(self, iri: NamedNode) -> Option<LedgerEntry> {
        if !self.found {
            return None;
        }
        Some(LedgerEntry {
            iri,
            graph: self.graph,
            env: self.env,
            last_dispatch_at: self.last_dispatch_at,
        })
    }
}

/// True iff the pair was dispatched within `ttl_seconds`. `ttl_seconds == 0`
/// disables dedup.
pub fn within_ttl(
    store: &Store,
    graph: &str,
    env: &str,
    ttl_seconds: u64,
    now: DateTime<Utc>,
) -> Result<bool, LedgerError> {
    if ttl_seconds == 0 {
        return Ok(false);
    }
    let Some(entry) = get_entry(store, graph, env)? else {
        return Ok(false);
    };
    let prior = entry.parsed_timestamp()?;
    let elapsed = now.signed_duration_since(prior);
    if elapsed.num_seconds() < 0 {
        // Clock skew — conservative: still within TTL.
        return Ok(true);
    }
    Ok((elapsed.num_seconds() as u64) < ttl_seconds)
}

/// Atomically replace any prior row for `(graph, env)` with a fresh
/// entry timestamped `now_rfc3339`.
pub fn record_dispatch(
    writer: &StreamWriter,
    store: &Store,
    graph: &str,
    env: &str,
    now_rfc3339: &str,
) -> Result<LedgerEntry, LedgerError> {
    let iri = entry_iri(graph, env);
    remove_prior_entry(store, &iri)?;
    let entry = LedgerEntry {
        iri,
        graph: graph.to_string(),
        env: env.to_string(),
        last_dispatch_at: now_rfc3339.to_string(),
    };
    let quads = entry_quads(&entry);
    let mutation = Mutation::insert(quads.iter().cloned()).with_cause(format!(
        "FT-100 record graph-accepted ledger ({graph}/{env})"
    ));
    writer
        .commit(mutation)
        .map_err(|e| LedgerError::Commit(format!("{e:#}")))?;
    Ok(entry)
}

fn remove_prior_entry(store: &Store, iri: &NamedNode) -> Result<(), LedgerError> {
    let to_remove: Vec<Quad> = store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                graph_accepted_ledger_graph(),
            )),
        )
        .filter_map(Result::ok)
        .collect();
    for q in &to_remove {
        store
            .remove(q.as_ref())
            .map_err(|e| LedgerError::Commit(format!("removing prior row: {e}")))?;
    }
    Ok(())
}

fn entry_quads(entry: &LedgerEntry) -> Vec<Quad> {
    let g: GraphName = graph_accepted_ledger_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    vec![
        Quad::new(
            entry.iri.clone(),
            rdf_type,
            graph_accepted_ledger_entry_class().into_owned(),
            g.clone(),
        ),
        Quad::new(
            entry.iri.clone(),
            ledger_graph_pred().into_owned(),
            Literal::new_simple_literal(&entry.graph),
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
