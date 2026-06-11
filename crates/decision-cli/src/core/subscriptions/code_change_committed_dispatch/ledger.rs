//! Dedup ledger for the code-change-committed auto-dispatch (FT-100).
//!
//! Keyed on `(code_change_iri, feature_iri)`.

use chrono::{DateTime, ParseError, Utc};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    code_change_committed_ledger_entry_class, code_change_ledger_graph, last_dispatch_at,
    ledger_code_change_pred, ledger_feature,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Errors surfaced by the ledger helpers.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// SPARQL / store failure.
    #[error("code-change-committed ledger query failed: {0}")]
    Query(String),
    /// Could not write a ledger entry through the StreamWriter.
    #[error("code-change-committed ledger commit failed: {0}")]
    Commit(String),
    /// Stored timestamp not RFC3339.
    #[error(
        "code-change-committed ledger timestamp invalid for {code_change}/{feature}: {detail}"
    )]
    BadTimestamp {
        /// Code-change IRI.
        code_change: String,
        /// Feature short id.
        feature: String,
        /// Parse-error detail.
        detail: String,
    },
}

/// One ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    /// IRI of the ledger row.
    pub iri: NamedNode,
    /// Code-change IRI literal.
    pub code_change: String,
    /// Feature short id literal.
    pub feature: String,
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
            code_change: self.code_change.clone(),
            feature: self.feature.clone(),
            detail: e.to_string(),
        })
    }
}

/// Deterministic IRI for a `(code_change, feature)` row.
#[must_use]
pub fn entry_iri(code_change: &str, feature: &str) -> NamedNode {
    let mut h = Sha256::new();
    h.update(code_change.as_bytes());
    h.update(b"\0");
    h.update(feature.as_bytes());
    let digest = h.finalize();
    let hex: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    NamedNode::new_unchecked(format!("urn:dec:code-change-committed-ledger:{hex}"))
}

/// Lookup a row for `(code_change, feature)`.
pub fn get_entry(
    store: &Store,
    code_change: &str,
    feature: &str,
) -> Result<Option<LedgerEntry>, LedgerError> {
    let iri = entry_iri(code_change, feature);
    let mut acc = LedgerAccumulator::default();
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                code_change_ledger_graph(),
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
    code_change: String,
    feature: String,
    last_dispatch_at: String,
    found: bool,
}

impl LedgerAccumulator {
    fn absorb(&mut self, q: &Quad) {
        self.found = true;
        match q.predicate.as_str() {
            crate::core::vocab::IRI_DEC_LEDGER_CODE_CHANGE => {
                self.code_change = term_literal(&q.object);
            }
            crate::core::vocab::IRI_DEC_LEDGER_FEATURE => {
                self.feature = term_literal(&q.object);
            }
            crate::core::vocab::IRI_DEC_LAST_DISPATCH_AT => {
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
            code_change: self.code_change,
            feature: self.feature,
            last_dispatch_at: self.last_dispatch_at,
        })
    }
}

/// True iff the pair was dispatched within `ttl_seconds`.
pub fn within_ttl(
    store: &Store,
    code_change: &str,
    feature: &str,
    ttl_seconds: u64,
    now: DateTime<Utc>,
) -> Result<bool, LedgerError> {
    if ttl_seconds == 0 {
        return Ok(false);
    }
    let Some(entry) = get_entry(store, code_change, feature)? else {
        return Ok(false);
    };
    let prior = entry.parsed_timestamp()?;
    let elapsed = now.signed_duration_since(prior);
    if elapsed.num_seconds() < 0 {
        return Ok(true);
    }
    Ok((elapsed.num_seconds() as u64) < ttl_seconds)
}

/// Atomically replace any prior row with a fresh one.
pub fn record_dispatch(
    writer: &StreamWriter,
    store: &Store,
    code_change: &str,
    feature: &str,
    now_rfc3339: &str,
) -> Result<LedgerEntry, LedgerError> {
    let iri = entry_iri(code_change, feature);
    remove_prior_entry(store, &iri)?;
    let entry = LedgerEntry {
        iri,
        code_change: code_change.to_string(),
        feature: feature.to_string(),
        last_dispatch_at: now_rfc3339.to_string(),
    };
    let quads = entry_quads(&entry);
    let mutation = Mutation::insert(quads.iter().cloned()).with_cause(format!(
        "FT-100 record code-change-committed ledger ({code_change}/{feature})"
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
                code_change_ledger_graph(),
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
    let g: GraphName = code_change_ledger_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    vec![
        Quad::new(
            entry.iri.clone(),
            rdf_type,
            code_change_committed_ledger_entry_class().into_owned(),
            g.clone(),
        ),
        Quad::new(
            entry.iri.clone(),
            ledger_code_change_pred().into_owned(),
            Literal::new_simple_literal(&entry.code_change),
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
