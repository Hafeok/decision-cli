//! Atomic file rewrite for `dec verify step add` (FT-044).
//!
//! The on-disk Turtle is rewritten only after SHACL and safety both pass
//! (FT-044 §Invariants). Write-temp + rename gives crash-safety: a
//! mid-flight crash leaves the previous file intact (TC-066 AC #6).

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_graph::{to_canonical_turtle, VerificationGraph};

/// Atomically rewrite `<graph_dir>/<id>.ttl` to the canonical Turtle for
/// `graph`. Writes through `.tmp` then renames; the previous file is
/// never partially overwritten in place.
pub(super) fn rewrite_graph_file(
    graph_dir: &Path,
    id: &str,
    graph: &VerificationGraph,
) -> Result<PathBuf, HandlerError> {
    fs::create_dir_all(graph_dir).map_err(|e| HandlerError::Internal {
        detail: format!("creating {d}: {e}", d = graph_dir.display()),
    })?;
    let final_path = graph_dir.join(format!("{id}.ttl"));
    let tmp_path = graph_dir.join(format!("{id}.ttl.tmp"));
    let ttl = to_canonical_turtle(graph);
    fs::write(&tmp_path, ttl.as_bytes()).map_err(|e| HandlerError::Internal {
        detail: format!("writing {p}: {e}", p = tmp_path.display()),
    })?;
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        // Best-effort cleanup of the temp file.
        let _ = fs::remove_file(&tmp_path);
        HandlerError::Internal {
            detail: format!(
                "renaming {tmp} -> {final_p}: {e}",
                tmp = tmp_path.display(),
                final_p = final_path.display()
            ),
        }
    })?;
    Ok(final_path)
}
