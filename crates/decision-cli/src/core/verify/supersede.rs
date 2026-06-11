//! VerificationGraph supersession — ADR-024-style lifecycle for the
//! graph artifact. When the verify-graph-author (or an operator)
//! decides a graph's design is fundamentally wrong, they mint a
//! replacement and write a `dec:supersededBy <new>` triple on the old
//! one. The enumerator + planner inspector both filter superseded
//! graphs out of their queries, so subsequent verify runs only see
//! the live successor.
//!
//! No on-disk Turtle is touched — the per-graph .ttl file under
//! `.dec/verify/graph/` is the static definition; lifecycle state
//! lives in the orchestration store (same pattern feedback uses).

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad};

use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{verify_graph_named_graph, IRI_DEC_SUPERSEDED_BY};

/// Mark `old_graph_iri` as superseded by `new_graph_iri`. Writes a
/// single `<old> dec:supersededBy <new>` quad in the
/// `verify_graph_named_graph()` named graph. Idempotent: re-writing
/// the same supersession is a no-op (oxigraph dedups quads at the
/// store level).
pub fn supersede_graph(
    workdir: &std::path::Path,
    old_graph_iri: &str,
    new_graph_iri: &str,
) -> Result<()> {
    let old = NamedNode::new(old_graph_iri).context("old graph IRI")?;
    let new = NamedNode::new(new_graph_iri).context("new graph IRI")?;
    if old == new {
        return Err(anyhow!("supersededBy cannot point at the graph itself"));
    }

    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .with_context(|| format!("loading orchestration store at {}", dump.display()))?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).context("loading active scope")?;
    let stream_iri = NamedNode::new(&scope.stream_iri).context("active stream iri")?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("opening writer for graph supersession")?;

    let graph_named: GraphName = verify_graph_named_graph().into_owned().into();
    let quad = Quad::new(
        old,
        NamedNodeRef::new_unchecked(IRI_DEC_SUPERSEDED_BY).into_owned(),
        new,
        graph_named,
    );
    writer
        .commit(Mutation::insert(vec![quad]))
        .with_context(|| format!("writing supersededBy triple for {old_graph_iri}"))?;

    persist_store(&store, &dump).context("persisting store after graph supersession")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_self_supersession() {
        let err = supersede_graph(std::path::Path::new("/nonexistent"), "urn:vg:1", "urn:vg:1")
            .unwrap_err();
        assert!(format!("{err}").contains("cannot point at the graph itself"));
    }
}
