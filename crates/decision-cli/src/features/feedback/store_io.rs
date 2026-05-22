//! Shared store I/O helpers for the `dec feedback {…}` subcommand family (FT-033).
//!
//! Every write-side feedback command (`close`, `route`, `receive`)
//! follows the same load-mutate-persist dance: open the orchestration
//! N-Quads dump as a read/write `Store`, bind a [`StreamWriter`] to the
//! active stream, run a single lifecycle transition through
//! `core::feedback::transition`, then write the store back atomically.
//!
//! Centralising the shape here keeps each subcommand under ADR-013's
//! 400-line cap and lets the four write commands share one chokepoint
//! for store-state validation. Per the slice-level SDP in `CLAUDE.md`,
//! every helper imports from `core::*` only — no sibling-feature
//! reach.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::scope::ActiveScope;
use crate::core::store::{
    load_store_from_dump, orchestration_dump_path, persist_store as core_persist_store,
};
use crate::core::StreamWriter;

/// Bundle of state every write command needs to mutate a feedback
/// artifact safely.
pub(super) struct WritableStore {
    /// In-memory clone of the persisted orchestration store.
    pub store: Arc<Store>,
    /// Writer bound to the active stream, augments every mutation with
    /// `dec:inStream` and validates feedback transitions.
    pub writer: StreamWriter,
    /// On-disk dump path; callers `persist` back here on success.
    pub dump_path: PathBuf,
    /// Resolved active scope snapshot — exposed so callers can include
    /// the active stream IRI in error messages without re-running the
    /// scope loader.
    pub scope: ActiveScope,
}

impl WritableStore {
    /// Open the working directory's orchestration store for writing.
    /// Returns `Err` if the working dir isn't initialised or the store
    /// is unreadable.
    pub(super) fn open(workdir: &Path) -> Result<Self> {
        let scope = ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
        let dump_path = orchestration_dump_path(workdir);
        if !dump_path.exists() {
            return Err(anyhow!(
                "no orchestration store at {} — run `dec init` first",
                dump_path.display()
            ));
        }
        let store = Arc::new(load_store_from_dump(&dump_path)?);
        let stream_iri = NamedNode::new(&scope.stream_iri)
            .with_context(|| format!("active stream IRI {}", scope.stream_iri))?;
        let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
            .context("binding StreamWriter to active stream")?;
        Ok(Self {
            store,
            writer,
            dump_path,
            scope,
        })
    }

    /// Active stream IRI as a `NamedNode`.
    pub(super) fn active_stream(&self) -> Result<NamedNode> {
        NamedNode::new(&self.scope.stream_iri)
            .with_context(|| format!("parsing active stream IRI {}", self.scope.stream_iri))
    }

    /// Atomically write the in-memory store back to disk. Call once,
    /// after the lifecycle transition succeeds.
    pub(super) fn persist(&self) -> Result<()> {
        core_persist_store(&self.store, &self.dump_path).with_context(|| {
            format!(
                "persisting orchestration store at {}",
                self.dump_path.display()
            )
        })
    }
}

/// Open the orchestration store read-only for `show`-style commands.
/// Cheaper than [`WritableStore::open`] because no writer is bound.
pub(super) fn open_readonly_store(workdir: &Path) -> Result<Store> {
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Err(anyhow!(
            "no orchestration store at {} — run `dec init` first",
            dump_path.display()
        ));
    }
    load_store_from_dump(&dump_path)
}

/// Resolve the active stream IRI from a read-only store.
pub(super) fn active_stream_iri(store: &Store) -> Result<NamedNode> {
    use oxigraph::model::Term;
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               { ?s a dec:ValueStream . } \
               UNION \
               { GRAPH ?g { ?s a dec:ValueStream . } } \
             } LIMIT 1";
    let QueryResults::Solutions(mut sols) = store.query(q).context("locating active stream")?
    else {
        return Err(anyhow!(
            "no dec:ValueStream artifact found — store may be corrupt"
        ));
    };
    let Some(sol) = sols.next() else {
        return Err(anyhow!(
            "no dec:ValueStream artifact found — store may be corrupt"
        ));
    };
    let sol = sol.context("decoding active-stream row")?;
    let Some(Term::NamedNode(node)) = sol.get("s").cloned() else {
        return Err(anyhow!("active stream subject is not an IRI"));
    };
    Ok(node)
}
