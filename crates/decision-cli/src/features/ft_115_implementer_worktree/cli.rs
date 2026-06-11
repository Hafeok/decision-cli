//! CLI handlers for worktree diagnostic commands (FT-115).

use std::path::Path;

use anyhow::{Context, Result};
use oxigraph::store::Store;

use super::prune_orphans;

/// Handle `dec _worktree list` — list live worktrees.
pub fn handle_worktree_list(workdir: &Path, store: &Store) -> Result<()> {
    let worktrees_root = workdir.join(".dec/worktrees");

    if !worktrees_root.exists() {
        println!("No worktrees found.");
        return Ok(());
    }

    let entries = std::fs::read_dir(&worktrees_root).context("failed to read .dec/worktrees/")?;

    let mut found_any = false;

    for entry in entries {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let Some(session_short_id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        found_any = true;

        // Try to get baseline commit
        let baseline = get_baseline_from_store(store, session_short_id);
        let status = get_status_from_store(store, session_short_id);

        println!(
            "Session: {}  Path: {}  Baseline: {}  Status: {}",
            session_short_id,
            path.display(),
            baseline.unwrap_or_else(|| "<unknown>".to_string()),
            status.unwrap_or_else(|| "<unknown>".to_string())
        );
    }

    if !found_any {
        println!("No worktrees found.");
    }

    Ok(())
}

/// Handle `dec _worktree prune` — delete orphan worktrees.
pub fn handle_worktree_prune(workdir: &Path, store: &Store) -> Result<()> {
    let outcome = prune_orphans(workdir, store).context("failed to prune orphan worktrees")?;

    if outcome.pruned.is_empty() && outcome.kept.is_empty() {
        println!("No worktrees found.");
    } else {
        if !outcome.pruned.is_empty() {
            println!("Pruned {} orphan worktree(s):", outcome.pruned.len());
            for session_id in &outcome.pruned {
                println!("  - {}", session_id);
            }
        }
        if !outcome.kept.is_empty() {
            println!("Kept {} live worktree(s):", outcome.kept.len());
            for session_id in &outcome.kept {
                println!("  - {}", session_id);
            }
        }
    }

    Ok(())
}

/// Get baseline commit from store for a session.
fn get_baseline_from_store(store: &Store, session_short_id: &str) -> Option<String> {
    use oxigraph::model::{NamedNode, Subject};

    let baseline_pred =
        NamedNode::new_unchecked("https://purl.org/ddd/decision-cli#worktreeBaseline");

    for quad in store.quads_for_pattern(None, Some(baseline_pred.as_ref().into()), None, None) {
        if let Ok(q) = quad {
            if let Subject::NamedNode(session_iri) = q.subject {
                if session_iri.as_str().contains(session_short_id) {
                    if let oxigraph::model::Term::Literal(lit) = q.object {
                        return Some(lit.value().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Get worktree status from store for a session.
fn get_status_from_store(store: &Store, session_short_id: &str) -> Option<String> {
    use oxigraph::model::{NamedNode, Subject};

    let status_pred = NamedNode::new_unchecked("https://purl.org/ddd/decision-cli#worktreeStatus");

    for quad in store.quads_for_pattern(None, Some(status_pred.as_ref().into()), None, None) {
        if let Ok(q) = quad {
            if let Subject::NamedNode(session_iri) = q.subject {
                if session_iri.as_str().contains(session_short_id) {
                    if let oxigraph::model::Term::Literal(lit) = q.object {
                        return Some(lit.value().to_string());
                    }
                }
            }
        }
    }

    None
}
