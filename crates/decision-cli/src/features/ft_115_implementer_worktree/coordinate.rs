//! Orchestration store coordination for worktree state (FT-115).

use std::path::Path;

use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Subject};
use oxigraph::store::Store;

use super::{abort_worktree, WorktreeError, WorktreePath};

/// Namespace for worktree predicates.
const DEC_NS: &str = "https://purl.org/ddd/decision-cli#";

/// Record worktree creation in the orchestration store.
///
/// Inserts:
/// - `<session> dec:worktreePath <abs_path>`
/// - `<session> dec:worktreeBaseline <baseline_commit>`
/// - `<session> dec:worktreeStatus "live"`
pub fn record_worktree_created(
    writer: &oxi_events::GraphWriter,
    store: &Store,
    dump_path: &Path,
    session_iri: &NamedNode,
    worktree: &WorktreePath,
) -> Result<(), WorktreeError> {
    let mut mutation = Mutation::default();

    let worktree_path_pred = NamedNode::new_unchecked(format!("{}worktreePath", DEC_NS));
    let worktree_baseline_pred = NamedNode::new_unchecked(format!("{}worktreeBaseline", DEC_NS));
    let worktree_status_pred = NamedNode::new_unchecked(format!("{}worktreeStatus", DEC_NS));

    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        worktree_path_pred,
        Literal::new_simple_literal(worktree.path.display().to_string()),
        GraphName::DefaultGraph,
    ));

    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        worktree_baseline_pred,
        Literal::new_simple_literal(&worktree.baseline_commit),
        GraphName::DefaultGraph,
    ));

    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        worktree_status_pred,
        Literal::new_simple_literal("live"),
        GraphName::DefaultGraph,
    ));

    writer
        .commit(mutation.with_cause("worktree created"))
        .map_err(|e| {
            WorktreeError::StoreError(format!("failed to commit worktree creation: {}", e))
        })?;

    // Persist to disk
    persist_store(store, dump_path)?;

    Ok(())
}

/// Record worktree merge in the orchestration store.
///
/// Inserts:
/// - `<session> dec:mergedInto <commit_sha>`
/// - `<session> dec:worktreeStatus "merged"` (replaces "live")
pub fn record_worktree_merged(
    writer: &oxi_events::GraphWriter,
    store: &Store,
    dump_path: &Path,
    session_iri: &NamedNode,
    merged_commit_sha: &str,
) -> Result<(), WorktreeError> {
    let mut mutation = Mutation::default();

    let worktree_status_pred = NamedNode::new_unchecked(format!("{}worktreeStatus", DEC_NS));
    let merged_into_pred = NamedNode::new_unchecked(format!("{}mergedInto", DEC_NS));

    // Remove old status="live"
    for quad in store.quads_for_pattern(
        Some(session_iri.as_ref().into()),
        Some(worktree_status_pred.as_ref().into()),
        None,
        None,
    ) {
        if let Ok(q) = quad {
            mutation.removes.push(q);
        }
    }

    // Insert new status="merged"
    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        worktree_status_pred,
        Literal::new_simple_literal("merged"),
        GraphName::DefaultGraph,
    ));

    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        merged_into_pred,
        Literal::new_simple_literal(merged_commit_sha),
        GraphName::DefaultGraph,
    ));

    writer
        .commit(mutation.with_cause("worktree merged"))
        .map_err(|e| {
            WorktreeError::StoreError(format!("failed to commit worktree merge: {}", e))
        })?;

    persist_store(store, dump_path)?;

    Ok(())
}

/// Record worktree abort in the orchestration store.
///
/// Inserts:
/// - `<session> dec:worktreeStatus "aborted"`
/// - `<session> dec:worktreeAbortReason <reason>`
pub fn record_worktree_aborted(
    writer: &oxi_events::GraphWriter,
    store: &Store,
    dump_path: &Path,
    session_iri: &NamedNode,
    reason: &str,
) -> Result<(), WorktreeError> {
    let mut mutation = Mutation::default();

    let worktree_status_pred = NamedNode::new_unchecked(format!("{}worktreeStatus", DEC_NS));
    let abort_reason_pred = NamedNode::new_unchecked(format!("{}worktreeAbortReason", DEC_NS));

    // Remove old status="live"
    for quad in store.quads_for_pattern(
        Some(session_iri.as_ref().into()),
        Some(worktree_status_pred.as_ref().into()),
        None,
        None,
    ) {
        if let Ok(q) = quad {
            mutation.removes.push(q);
        }
    }

    // Insert new status="aborted"
    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        worktree_status_pred,
        Literal::new_simple_literal("aborted"),
        GraphName::DefaultGraph,
    ));

    mutation.inserts.push(Quad::new(
        session_iri.clone(),
        abort_reason_pred,
        Literal::new_simple_literal(reason),
        GraphName::DefaultGraph,
    ));

    writer
        .commit(mutation.with_cause("worktree aborted"))
        .map_err(|e| {
            WorktreeError::StoreError(format!("failed to commit worktree abort: {}", e))
        })?;

    persist_store(store, dump_path)?;

    Ok(())
}

/// Outcome of orphan worktree pruning.
#[derive(Debug, Clone)]
pub struct PruneOutcome {
    pub pruned: Vec<String>,
    pub kept: Vec<String>,
}

/// Prune orphan worktrees (worktrees without a live session in the store).
///
/// A worktree is considered orphan if:
/// - Its directory exists in `.dec/worktrees/`
/// - No session in the store has `dec:worktreeStatus "live"` for that session ID
///
/// Terminal sessions (status="merged" or "aborted") whose worktrees still exist
/// are also pruned.
pub fn prune_orphans(workdir: &Path, store: &Store) -> Result<PruneOutcome, WorktreeError> {
    let worktrees_root = workdir.join(".dec/worktrees");
    let mut pruned = Vec::new();
    let mut kept = Vec::new();

    if !worktrees_root.exists() {
        return Ok(PruneOutcome { pruned, kept });
    }

    // Enumerate worktree directories
    let entries = std::fs::read_dir(&worktrees_root).map_err(|e| WorktreeError::Io(e))?;

    for entry in entries {
        let entry = entry.map_err(|e| WorktreeError::Io(e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let Some(session_short_id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Check if this session has worktreeStatus="live" in the store
        if is_worktree_live(store, session_short_id) {
            kept.push(session_short_id.to_string());
        } else {
            // Orphan — prune it
            let worktree_path = WorktreePath {
                path: path.clone(),
                session_short_id: session_short_id.to_string(),
                baseline_commit: String::new(), // not needed for abort
            };

            match abort_worktree(workdir, &worktree_path) {
                Ok(_) => {
                    pruned.push(session_short_id.to_string());
                }
                Err(e) => {
                    eprintln!(
                        "warning: failed to prune orphan worktree {}: {}",
                        session_short_id, e
                    );
                    kept.push(session_short_id.to_string());
                }
            }
        }
    }

    Ok(PruneOutcome { pruned, kept })
}

/// Check if a session has `dec:worktreeStatus "live"` in the store.
fn is_worktree_live(store: &Store, session_short_id: &str) -> bool {
    let worktree_status_pred = NamedNode::new_unchecked(format!("{}worktreeStatus", DEC_NS));

    // We need to find the session IRI that matches this short ID.
    // Session IRIs are like urn:dec:session:<uuid>.
    // We'll query for all sessions with worktreeStatus and check if any have "live".

    for quad in store.quads_for_pattern(
        None,
        Some(worktree_status_pred.as_ref().into()),
        Some((&Literal::new_simple_literal("live")).into()),
        None,
    ) {
        if let Ok(q) = quad {
            // Check if this session's IRI contains the short ID
            if let Subject::NamedNode(session_iri) = q.subject {
                let session_iri_str = session_iri.as_str();
                if session_iri_str.contains(session_short_id) {
                    return true;
                }
            }
        }
    }

    false
}

/// Persist the store to disk.
fn persist_store(store: &Store, dump_path: &Path) -> Result<(), WorktreeError> {
    use std::fs::File;
    use std::io::BufWriter;

    let file = File::create(dump_path)
        .map_err(|e| WorktreeError::StoreError(format!("failed to create dump file: {}", e)))?;
    let writer = BufWriter::new(file);

    store
        .dump_graph_to_writer(
            &GraphName::DefaultGraph,
            oxigraph::io::RdfFormat::NQuads,
            writer,
        )
        .map_err(|e| WorktreeError::StoreError(format!("failed to dump graph: {}", e)))?;

    Ok(())
}
