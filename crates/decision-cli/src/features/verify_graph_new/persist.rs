//! On-disk persistence for `dec verify graph new` (FT-041).
//!
//! Writes the canonical Turtle produced by
//! [`crate::core::ontology::verification_graph::to_canonical_turtle`]
//! under `.dec/verify/graph/<id>.ttl`. The file is written **after**
//! the `StreamWriter` chokepoint approves the mutation; SHACL failures
//! therefore never leave a partial file on disk (FT-041 §Invariants).

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_graph::{to_canonical_turtle, VerificationGraph};

/// Write the graph's Turtle representation to `.dec/verify/graph/<id>.ttl`.
///
/// Creates the directory if missing. Returns the path written.
pub fn write_graph_file(
    graph_dir: &Path,
    id: &str,
    graph: &VerificationGraph,
) -> Result<PathBuf, HandlerError> {
    fs::create_dir_all(graph_dir).map_err(|e| HandlerError::Internal {
        detail: format!("creating {d}: {e}", d = graph_dir.display()),
    })?;
    let path = graph_dir.join(format!("{id}.ttl"));
    let ttl = to_canonical_turtle(graph);
    fs::write(&path, ttl.as_bytes()).map_err(|e| HandlerError::Internal {
        detail: format!("writing {p}: {e}", p = path.display()),
    })?;
    Ok(path)
}
