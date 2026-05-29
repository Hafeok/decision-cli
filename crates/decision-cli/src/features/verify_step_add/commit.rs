//! Orchestration-store update path for `dec verify step add` (FT-044).
//!
//! Two responsibilities split out of `mod.rs` per ADR-013 §Rule 1:
//!
//!   * [`load_graph`] / [`resolve_path`] — turn a caller-supplied
//!     graph id into an on-disk `.ttl` path and a parsed
//!     [`VerificationGraph`].
//!   * [`load_env_for_graph`] — read the env file the graph references
//!     so the safety check (FT-037) has typed inputs.
//!   * [`commit_graph_update`] — open `StreamWriter`, compose the
//!     `Mutation { removes, inserts }`, commit, persist the dump.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_bench::{
    from_turtle as bench_from_turtle, VerificationBench,
};
use crate::core::ontology::verification_graph::{from_turtle, VerificationGraph};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::{verify_graph_named_graph, IRI_DEC_BENCH_PREFIX};
use crate::core::StreamWriter;

use super::store_diff;

/// Artifact kind surfaced on `ArtifactNotFound` per FT-044 §Error handling.
const ARTIFACT_KIND: &str = "VerificationGraph";

/// Resolve `graph_id` to its on-disk `.ttl` and parse it. Returns
/// `ArtifactNotFound` if no matching file exists.
pub(super) fn load_graph(
    graph_dir: &Path,
    graph_id: &str,
) -> Result<(PathBuf, VerificationGraph), HandlerError> {
    let path = resolve_path(graph_dir, graph_id)?;
    let graph = from_turtle(&path).map_err(|e| HandlerError::Internal {
        detail: format!("parsing graph file {p}: {e}", p = path.display()),
    })?;
    Ok((path, graph))
}

fn resolve_path(graph_dir: &Path, id: &str) -> Result<PathBuf, HandlerError> {
    let exact = graph_dir.join(format!("{id}.ttl"));
    if exact.is_file() {
        return Ok(exact);
    }
    if !graph_dir.exists() {
        return Err(not_found(id));
    }
    let mut matches = scan_matching_ttls(graph_dir, id)?;
    match matches.len() {
        0 => Err(not_found(id)),
        1 => Ok(matches.remove(0)),
        n => Err(HandlerError::Internal {
            detail: format!(
                "ambiguous graph id {id:?}: {n} files match (under {d})",
                d = graph_dir.display()
            ),
        }),
    }
}

/// Read `graph_dir` and return every `.ttl` whose stem is exactly `id`
/// or starts with `{id}-`. Surfaces `Internal` on any IO error so the
/// caller can present a uniform diagnostic.
fn scan_matching_ttls(graph_dir: &Path, id: &str) -> Result<Vec<PathBuf>, HandlerError> {
    let prefix = format!("{id}-");
    let mut matches: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(graph_dir).map_err(|e| HandlerError::Internal {
        detail: format!("reading {d}: {e}", d = graph_dir.display()),
    })? {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("walking {d}: {e}", d = graph_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if stem == id || stem.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    Ok(matches)
}

fn not_found(id: &str) -> HandlerError {
    HandlerError::ArtifactNotFound {
        kind: ARTIFACT_KIND.to_string(),
        id: id.to_string(),
    }
}

/// Load the env the graph references by parsing
/// `.dec/verify/env/<env_id>.ttl`. Surfaces `DanglingRef` when the env
/// file is missing.
pub(super) fn load_env_for_graph(
    workdir: &Path,
    graph: &VerificationGraph,
) -> Result<VerificationBench, HandlerError> {
    let env_iri = graph.environment.as_str();
    let env_id =
        env_iri
            .strip_prefix(IRI_DEC_BENCH_PREFIX)
            .ok_or_else(|| HandlerError::Internal {
                detail: format!("graph references non-canonical env IRI {env_iri:?}"),
            })?;
    // FT-112 renamed env/ → bench/; this loader was missed in original migration.
    let env_dir = workdir.join(".dec").join("verify").join("bench");
    let env_path = env_dir.join(format!("{env_id}.ttl"));
    if !env_path.is_file() {
        return Err(HandlerError::DanglingRef {
            reference: env_id.to_string(),
            kind: "environment".to_string(),
        });
    }
    bench_from_turtle(&env_path).map_err(|e| HandlerError::Internal {
        detail: format!("parsing env file {p}: {e}", p = env_path.display()),
    })
}

/// Commit the updated graph through `StreamWriter`. Removes stale
/// `dec:steps` rdf:List cells from the store projection, then inserts
/// the full set of canonical quads for the new graph; `StreamWriter`
/// re-runs FT-037 safety + FT-036 SHACL as a defensive pass.
pub(super) fn commit_graph_update(
    workdir: &Path,
    new_graph: &VerificationGraph,
) -> Result<(), HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    let (store, writer) = open_writer(workdir, &dump_path)?;
    let mutation = build_step_mutation(&store, new_graph);
    writer.commit(mutation).map_err(map_writer_error)?;
    persist_store(&store, &dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("persisting store: {e}"),
    })?;
    Ok(())
}

/// Load the orchestration dump and open a `StreamWriter` bound to the
/// active scope's stream. Returns both the `Arc<Store>` (so we can
/// project the rdf:List diff against it) and the writer.
fn open_writer(
    workdir: &Path,
    dump_path: &Path,
) -> Result<(Arc<oxigraph::store::Store>, StreamWriter), HandlerError> {
    let scope = ActiveScope::load(workdir).map_err(|e| HandlerError::Internal {
        detail: format!("loading active scope: {e}"),
    })?;
    let store = load_store_from_dump(dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    let store = Arc::new(store);
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| HandlerError::Internal {
        detail: format!("active stream iri {iri}: {e}", iri = scope.stream_iri),
    })?;
    let writer =
        StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| HandlerError::Internal {
            detail: format!("opening stream writer: {e}"),
        })?;
    Ok((store, writer))
}

/// Compose the `Mutation { removes, inserts }` for an appended step.
fn build_step_mutation(store: &oxigraph::store::Store, new_graph: &VerificationGraph) -> Mutation {
    let removes = store_diff::list_quads_to_remove(store, &new_graph.id);
    let inserts = new_graph.to_quads(verify_graph_named_graph());
    let mut mutation = Mutation::insert(inserts);
    mutation.removes = removes;
    mutation
}

/// Map a `StreamWriter::commit` error to a typed handler error.
/// Pre-flight safety check in `run` should have raised typed
/// `HandlerError::SafetyViolation` already; falling back to
/// Internal preserves the diagnostic but signals "the writer
/// caught what the pre-flight missed".
fn map_writer_error(e: anyhow::Error) -> HandlerError {
    let msg = format!("{e:#}");
    if msg.contains("SHACL violation") {
        HandlerError::SchemaViolation { detail: msg }
    } else {
        HandlerError::Internal { detail: msg }
    }
}
