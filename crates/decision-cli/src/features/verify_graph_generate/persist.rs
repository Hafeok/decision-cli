//! Atomic-per-graph persistence of a `New` proposal (FT-049 §Persistence path).
//!
//! Per ADR-030 §Persistence path and TC-079 AC #4, persistence reuses
//! the slice-2.5 writers — there is no parallel write path:
//!
//!   1. Call [`crate::verify_graph_new::run`] once to create the empty
//!      graph + on-disk Turtle.
//!   2. For each `ProposedStep` in the proposal, call
//!      [`crate::verify_step_add::run`] (extended in this slice with
//!      `provides_evidence_for`).
//!   3. If any step-add fails (SHACL, safety, IO), roll back the partial
//!      graph: delete the on-disk file, drop the projection in the
//!      orchestration store. The graph never exists in a half-written
//!      state on disk after this function returns (TC-081 AC #1-#3).
//!
//! Thread-local writer-call instrumentation lets tests count how many
//! times each writer was invoked (TC-079 AC #4 — "writer-instrumentation
//! hook").

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxigraph::model::NamedNode;

use crate::core::handler::Error as HandlerError;
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::verify_graph_named_graph;
use crate::core::StreamWriter;
use crate::verify_graph_new::{self, GraphNewRequest};
use crate::verify_step_add::{self, StepAddRequest};

use super::proposal::ProposedStep;

/// Counts of the slice-2.5 writer handler invocations during a single
/// persist call. The recorder is thread-local so concurrent tests do
/// not interfere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriterCallLog {
    /// How many times `verify_graph_new::run` was called.
    pub graph_new_calls: u32,
    /// How many times `verify_step_add::run` was called.
    pub step_add_calls: u32,
}

thread_local! {
    static WRITER_CALL_LOG: RefCell<WriterCallLog> = const {
        RefCell::new(WriterCallLog {
            graph_new_calls: 0,
            step_add_calls: 0,
        })
    };
}

/// Reset the thread-local writer call log (call before each test).
pub fn reset_writer_call_log() {
    WRITER_CALL_LOG.with(|cell| {
        *cell.borrow_mut() = WriterCallLog::default();
    });
}

/// Snapshot the thread-local writer call log.
#[must_use]
pub fn writer_call_log() -> WriterCallLog {
    WRITER_CALL_LOG.with(|cell| *cell.borrow())
}

fn record_graph_new() {
    WRITER_CALL_LOG.with(|cell| {
        cell.borrow_mut().graph_new_calls += 1;
    });
}

fn record_step_add() {
    WRITER_CALL_LOG.with(|cell| {
        cell.borrow_mut().step_add_calls += 1;
    });
}

/// Persistence outcome — the new graph id and on-disk path.
#[derive(Debug, Clone)]
pub struct Persisted {
    /// Minted graph id (e.g. `VG-007`).
    pub graph_id: String,
    /// Absolute path to the on-disk `.ttl`.
    pub graph_path: PathBuf,
}

/// Persist a `New` proposal: create the graph via FT-041, append every
/// step via FT-044, and roll back on any failure.
pub fn persist_new_proposal(
    workdir: &Path,
    feature_id: &str,
    env_short: &str,
    steps: &[ProposedStep],
) -> Result<Persisted, HandlerError> {
    // 1. Create the empty graph through the slice-2.5 graph_new writer.
    record_graph_new();
    let graph_req = GraphNewRequest {
        id: None,
        verifies: feature_id.to_string(),
        environment: env_short.to_string(),
        workdir: Some(workdir.to_path_buf()),
    };
    let graph_outcome = verify_graph_new::run(&graph_req)?;

    // 2. For each step, append via the slice-2.5 step_add writer. If any
    //    fails, roll back (delete file + drop store projection).
    for step in steps {
        record_step_add();
        let req = build_step_add_request(workdir, &graph_outcome.id, step);
        if let Err(err) = verify_step_add::run(&req) {
            // Best-effort rollback — propagate the *original* error so
            // the caller surfaces TC-081's expected SchemaViolation.
            rollback_partial_graph(workdir, &graph_outcome.id, &graph_outcome.path);
            return Err(err);
        }
    }

    Ok(Persisted {
        graph_id: graph_outcome.id,
        graph_path: graph_outcome.path,
    })
}

fn build_step_add_request(
    workdir: &Path,
    graph_id: &str,
    step: &ProposedStep,
) -> StepAddRequest {
    let mut fields = std::collections::BTreeMap::<String, String>::new();
    for (key, value) in &step.fields {
        let v_str = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        fields.insert(key.clone(), v_str);
    }
    StepAddRequest {
        graph_id: graph_id.to_string(),
        step_type: step.step_type.clone(),
        fields,
        provides_evidence_for: step.provides_evidence_for.clone(),
        workdir: Some(workdir.to_path_buf()),
    }
}

/// Rollback the partial graph created during a failed persist call.
/// Both the on-disk file AND the orchestration-store projection are
/// dropped so subsequent `dec verify graph list` / `dec verify graph
/// show` do not observe the half-written artifact (TC-081 AC #2-#5).
fn rollback_partial_graph(workdir: &Path, graph_id: &str, graph_path: &Path) {
    // 1. Delete the on-disk Turtle (best effort).
    let _ = fs::remove_file(graph_path);
    // 2. Drop the projection from the orchestration store.
    let _ = drop_graph_from_store(workdir, graph_id);
}

fn drop_graph_from_store(workdir: &Path, graph_id: &str) -> Result<(), HandlerError> {
    let scope = ActiveScope::load(workdir).map_err(|e| HandlerError::Internal {
        detail: format!("rollback: loading active scope: {e}"),
    })?;
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Ok(());
    }
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("rollback: loading store: {e}"),
    })?;
    let store = Arc::new(store);
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| HandlerError::Internal {
        detail: format!("rollback: active stream iri: {e}"),
    })?;
    let _writer = StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| {
        HandlerError::Internal {
            detail: format!("rollback: opening stream writer: {e}"),
        }
    })?;

    // Walk every quad in the verify-graph named graph that mentions
    // the graph IRI as subject and remove them directly from the store
    // (this is a rollback; we cannot use the StreamWriter chokepoint
    // because the chokepoint would re-validate SHACL on an empty graph
    // intermediate state — and the partial state is what we're
    // attempting to discard).
    drop_graph_projection(&store, graph_id);
    persist_store(&store, &dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("rollback: persisting store: {e}"),
    })?;
    Ok(())
}

fn drop_graph_projection(store: &oxigraph::store::Store, graph_id: &str) {
    use oxigraph::model::{GraphNameRef, Quad, Subject, Term};

    let iri_str = if graph_id.starts_with("https://") {
        graph_id.to_string()
    } else {
        format!(
            "{prefix}{graph_id}",
            prefix = crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX
        )
    };
    let graph_iri = NamedNode::new_unchecked(iri_str);
    let verify_graph = verify_graph_named_graph();

    let mut to_remove: Vec<Quad> = Vec::new();
    // 1. All quads whose subject IS the graph IRI.
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(graph_iri.clone()).as_ref()),
            None,
            None,
            Some(GraphNameRef::NamedNode(verify_graph)),
        )
        .filter_map(Result::ok)
    {
        to_remove.push(q);
    }
    // 2. All quads whose subject is a step belonging to this graph
    //    (step IRIs start with the graph's step prefix).
    let step_prefix = format!(
        "{prefix}{graph_id}/",
        prefix = crate::core::vocab::IRI_DEC_STEP_PREFIX
    );
    for q in store
        .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(verify_graph)))
        .filter_map(Result::ok)
    {
        if let Subject::NamedNode(s) = &q.subject {
            if s.as_str().starts_with(&step_prefix) {
                to_remove.push(q.clone());
            }
        }
        // Also drop quads whose object refers to the graph or its steps,
        // in case the rdf:List head references survived from the
        // chokepoint commit (defensive cleanup).
        if let Term::NamedNode(o) = &q.object {
            if o.as_str() == graph_iri.as_str() || o.as_str().starts_with(&step_prefix) {
                to_remove.push(q.clone());
            }
        }
    }
    for q in to_remove {
        let _ = store.remove(q.as_ref());
    }
}
