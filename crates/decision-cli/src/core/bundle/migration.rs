//! Idempotent backfill of `dec:stakes` on existing bundles (FT-058 plumbing
//! lives here per FT-056 §State).
//!
//! Bundles that pre-date FT-056 may exist in the orchestration store
//! without a `dec:stakes` literal. The bootstrap migration walks every
//! `dec:Bundle` subject lacking `dec:stakes` and inserts
//! `dec:stakes "routine"`. Re-running the migration on a graph where
//! every bundle already has stakes is a no-op.

use std::sync::Arc;

use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    bundle_graph, stakes_pred, IRI_DEC_BUNDLE, IRI_DEC_STAKES, STAKES_ROUTINE,
};

/// Outcome of [`migrate_bundle_stakes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationOutcome {
    /// Number of bundle subjects that received a backfill literal.
    pub backfilled: usize,
}

impl MigrationOutcome {
    /// True iff the migration touched the graph.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.backfilled > 0
    }
}

/// Failure modes for the migration.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// SPARQL query against the store failed.
    #[error("oxigraph store error: {0}")]
    Store(String),
    /// Underlying writer rejected the mutation (e.g. SHACL violation).
    #[error("writer rejected backfill: {0}")]
    Writer(String),
}

/// Walk every `dec:Bundle` subject in `store` lacking a `dec:stakes`
/// literal and insert `dec:stakes "routine"` per FT-056 §State.
///
/// Idempotent: re-running on a graph where every bundle already has
/// stakes is a no-op (`backfilled == 0`).
///
/// The mutation is routed through [`StreamWriter::commit`], so the same
/// SHACL pass that gates new bundle writes also gates the backfill.
/// Empty stores skip the writer entirely (avoids the bootstrap
/// stream-presence precondition for callers that only have a store).
pub fn migrate_bundle_stakes(
    store: &Arc<Store>,
    writer: Option<&StreamWriter>,
) -> Result<MigrationOutcome, MigrationError> {
    let needing = find_bundles_without_stakes(store)?;
    if needing.is_empty() {
        return Ok(MigrationOutcome { backfilled: 0 });
    }
    let inserts = build_backfill_quads(&needing);
    match writer {
        Some(w) => {
            w.commit(Mutation::insert(inserts))
                .map_err(|e| MigrationError::Writer(format!("{e:#}")))?;
        }
        None => {
            // Direct-store fallback for callers without a configured stream
            // (e.g. early test fixtures). Mirrors the StreamWriter bootstrap
            // pattern used elsewhere — transactional insert through the
            // store's own API.
            store
                .transaction(|mut tx| {
                    for q in &inserts {
                        tx.insert(q.as_ref())?;
                    }
                    Ok::<(), oxigraph::store::StorageError>(())
                })
                .map_err(|e| MigrationError::Writer(e.to_string()))?;
        }
    }
    Ok(MigrationOutcome {
        backfilled: needing.len(),
    })
}

fn find_bundles_without_stakes(store: &Store) -> Result<Vec<NamedNode>, MigrationError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?b WHERE {{ \
           {{ ?b a <{cls}> . FILTER NOT EXISTS {{ ?b <{stakes}> ?s }} }} \
           UNION \
           {{ GRAPH ?g {{ ?b a <{cls}> . FILTER NOT EXISTS {{ ?b <{stakes}> ?s }} }} }} \
         }}",
        cls = IRI_DEC_BUNDLE,
        stakes = IRI_DEC_STAKES,
    );
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| MigrationError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| MigrationError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("b") {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn build_backfill_quads(subjects: &[NamedNode]) -> Vec<Quad> {
    let graph: GraphName = bundle_graph().into_owned().into();
    let pred = stakes_pred().into_owned();
    let lit = Literal::new_simple_literal(STAKES_ROUTINE);
    subjects
        .iter()
        .map(|s| {
            Quad::new(
                Subject::NamedNode(s.clone()),
                pred.clone(),
                Term::Literal(lit.clone()),
                graph.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vocab::IRI_DEC_BUNDLE;
    use oxigraph::model::NamedNodeRef;
    use oxigraph::store::Store as OxStore;

    fn rdf_type() -> NamedNodeRef<'static> {
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    }

    fn bundle_class_node() -> NamedNode {
        NamedNode::new_unchecked(IRI_DEC_BUNDLE)
    }

    fn insert_pre_ft056_bundle(store: &Store, iri: &str) {
        let graph: GraphName = bundle_graph().into_owned().into();
        let subj = NamedNode::new(iri).expect("subject iri");
        let q = Quad::new(subj, rdf_type(), bundle_class_node(), graph);
        store
            .transaction(|mut tx| tx.insert(q.as_ref()).map(|_| ()))
            .expect("seed bundle without stakes");
    }

    #[test]
    fn backfills_pre_existing_bundles_with_routine() {
        let store = Arc::new(OxStore::new().expect("in-memory store"));
        insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/abc");
        insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/def");
        insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/ghi");

        let outcome = migrate_bundle_stakes(&store, None).expect("migrate succeeds");
        assert_eq!(outcome.backfilled, 3);

        // Every bundle now has dec:stakes "routine".
        let q = format!(
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT (COUNT(?b) AS ?n) WHERE {{ \
               {{ ?b a <{cls}> ; <{stakes}> ?s }} \
               UNION \
               {{ GRAPH ?g {{ ?b a <{cls}> ; <{stakes}> ?s }} }} \
             }}",
            cls = IRI_DEC_BUNDLE,
            stakes = IRI_DEC_STAKES,
        );
        let QueryResults::Solutions(mut sols) = store.query(&q).expect("count") else {
            panic!("expected solutions");
        };
        let sol = sols.next().expect("one row").expect("ok");
        let n = match sol.get("n") {
            Some(Term::Literal(lit)) => lit.value().parse::<usize>().expect("number"),
            _ => panic!("expected literal"),
        };
        assert_eq!(n, 3);
    }

    #[test]
    fn second_run_is_noop() {
        let store = Arc::new(OxStore::new().expect("in-memory store"));
        insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/xyz");
        let first = migrate_bundle_stakes(&store, None).expect("first run");
        assert_eq!(first.backfilled, 1);
        let second = migrate_bundle_stakes(&store, None).expect("second run");
        assert_eq!(second.backfilled, 0);
        assert!(!second.changed());
    }

    #[test]
    fn empty_graph_is_noop() {
        let store = Arc::new(OxStore::new().expect("in-memory store"));
        let outcome = migrate_bundle_stakes(&store, None).expect("noop");
        assert_eq!(outcome.backfilled, 0);
    }
}
