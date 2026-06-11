//! Backfill helpers for pre-FT-058 artifacts touched by catalog bootstrap.
//!
//! - [`migrate_bundle_stakes`] backfills `dec:stakes "routine"` on every
//!   `dec:Bundle` that lacks the field (FT-056 default per ADR-035).
//! - [`migrate_session_token_breakdown`] backfills the three FT-057
//!   token-breakdown fields on every Anthropic `dec:Session` that lacks
//!   them. `input_tokens_base` takes any prior `dec:input_tokens` literal
//!   if present, else 0.
//!
//! Both are idempotent on field presence: a re-run finds nothing to do.

use std::sync::Arc;

use oxigraph::model::{Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    bundle_class, bundle_graph, input_tokens_base_pred, input_tokens_cache_hit_pred,
    input_tokens_cache_write_pred, orchestration_graph, stakes_pred, IRI_DEC_INPUT_TOKENS_BASE,
    IRI_DEC_INPUT_TOKENS_CACHE_HIT, IRI_DEC_INPUT_TOKENS_CACHE_WRITE, IRI_DEC_SESSION,
    IRI_DEC_STAKES, STAKES_ROUTINE,
};

/// Errors produced by the migration helpers.
#[derive(Debug, Error)]
pub enum MigrateError {
    /// Underlying SPARQL or store error.
    #[error("oxigraph error during migration: {0}")]
    Store(String),
}

const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// Walk every `dec:Bundle` lacking `dec:stakes` and insert
/// `dec:stakes "routine"` (FT-056 default). Returns the count of
/// bundles that were updated.
pub fn migrate_bundle_stakes(store: &Arc<Store>) -> Result<usize, MigrateError> {
    let bundles_without_stakes = find_bundles_missing_stakes(store)?;
    if bundles_without_stakes.is_empty() {
        return Ok(0);
    }
    let stakes_obj: Term = Literal::new_simple_literal(STAKES_ROUTINE).into();
    let stakes_pred_node = stakes_pred().into_owned();
    let bundle_graph_node = bundle_graph().into_owned();

    let mut quads = Vec::with_capacity(bundles_without_stakes.len());
    for subject in bundles_without_stakes {
        quads.push(Quad::new(
            subject,
            stakes_pred_node.clone(),
            stakes_obj.clone(),
            bundle_graph_node.clone(),
        ));
    }
    let count = quads.len();
    insert_quads(store, &quads)?;
    Ok(count)
}

/// Walk every `dec:Session` lacking the three token-breakdown fields and
/// insert `input_tokens_base = <prior dec:input_tokens, or 0>`,
/// `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 0`.
pub fn migrate_session_token_breakdown(store: &Arc<Store>) -> Result<usize, MigrateError> {
    let candidates = find_sessions_missing_breakdown(store)?;
    if candidates.is_empty() {
        return Ok(0);
    }
    let mut quads = Vec::with_capacity(candidates.len() * 3);
    let orchestration = orchestration_graph().into_owned();
    for subject in candidates {
        let base = prior_input_tokens(store, &subject).unwrap_or_else(|| "0".to_string());
        quads.push(typed_quad(
            &subject,
            input_tokens_base_pred(),
            &base,
            XSD_INTEGER,
            &orchestration,
        ));
        quads.push(typed_quad(
            &subject,
            input_tokens_cache_write_pred(),
            "0",
            XSD_INTEGER,
            &orchestration,
        ));
        quads.push(typed_quad(
            &subject,
            input_tokens_cache_hit_pred(),
            "0",
            XSD_INTEGER,
            &orchestration,
        ));
    }
    let count = quads.len() / 3;
    insert_quads(store, &quads)?;
    Ok(count)
}

fn find_bundles_missing_stakes(store: &Store) -> Result<Vec<NamedNode>, MigrateError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?b WHERE {{ \
           {{ ?b a <{cls}> . }} \
           UNION \
           {{ GRAPH ?g {{ ?b a <{cls}> }} }} \
           FILTER NOT EXISTS {{ \
             {{ ?b <{stakes}> ?s }} \
             UNION \
             {{ GRAPH ?g2 {{ ?b <{stakes}> ?s }} }} \
           }} \
         }}",
        cls = bundle_class().as_str(),
        stakes = IRI_DEC_STAKES,
    );
    collect_iris(store, &q, "b")
}

fn find_sessions_missing_breakdown(store: &Store) -> Result<Vec<NamedNode>, MigrateError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?s WHERE {{ \
           {{ ?s a <{cls}> . }} \
           UNION \
           {{ GRAPH ?g {{ ?s a <{cls}> }} }} \
           FILTER NOT EXISTS {{ \
             {{ ?s <{base}> ?v }} \
             UNION \
             {{ GRAPH ?g2 {{ ?s <{base}> ?v }} }} \
           }} \
           FILTER NOT EXISTS {{ \
             {{ ?s <{cw}> ?v2 }} \
             UNION \
             {{ GRAPH ?g3 {{ ?s <{cw}> ?v2 }} }} \
           }} \
           FILTER NOT EXISTS {{ \
             {{ ?s <{ch}> ?v3 }} \
             UNION \
             {{ GRAPH ?g4 {{ ?s <{ch}> ?v3 }} }} \
           }} \
         }}",
        cls = IRI_DEC_SESSION,
        base = IRI_DEC_INPUT_TOKENS_BASE,
        cw = IRI_DEC_INPUT_TOKENS_CACHE_WRITE,
        ch = IRI_DEC_INPUT_TOKENS_CACHE_HIT,
    );
    collect_iris(store, &q, "s")
}

fn collect_iris(store: &Store, q: &str, var: &str) -> Result<Vec<NamedNode>, MigrateError> {
    let QueryResults::Solutions(sols) = store
        .query(q)
        .map_err(|e| MigrateError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| MigrateError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get(var) {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn prior_input_tokens(store: &Store, session: &NamedNode) -> Option<String> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?v WHERE {{ \
           {{ <{s}> dec:input_tokens ?v }} \
           UNION \
           {{ GRAPH ?g {{ <{s}> dec:input_tokens ?v }} }} \
         }} LIMIT 1",
        s = session.as_str(),
    );
    let QueryResults::Solutions(mut sols) = store.query(q.as_str()).ok()? else {
        return None;
    };
    let sol = sols.next()?.ok()?;
    match sol.get("v")? {
        Term::Literal(lit) => Some(lit.value().to_string()),
        _ => None,
    }
}

fn typed_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    datatype: &str,
    g: &NamedNode,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        g.clone(),
    )
}

fn insert_quads(store: &Store, quads: &[Quad]) -> Result<(), MigrateError> {
    store
        .transaction(|mut tx| {
            for q in quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| MigrateError::Store(e.to_string()))
}
