//! FT-109.C — materialize worker-dispatch session artifacts so the
//! existing `dec session list / show / log` surfaces cover both halves
//! of the verify → re-fix loop, not just the verify-runner side.
//!
//! The verify-graph-author and code-writer dispatch paths historically
//! built ad-hoc `activity/...` IRIs that were attached as
//! `dec:receivingSession` on feedback but never typed as
//! `dec:Session`. This module's [`materialize`] helper retroactively
//! commits the typing + provenance quads through the writer.
//!
//! Idempotent: re-calling with an already-typed IRI is a no-op.

use std::sync::Arc;

use anyhow::{Context, Result};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};

use dec_graph::scope::ActiveScope;
use dec_graph::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::orchestration_graph;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_STARTED_AT_TIME: &str = "http://www.w3.org/ns/prov#startedAtTime";
const PROV_ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";
const IRI_DEC_SESSION: &str = "https://decision-cli.dev/ns#Session";
const IRI_DEC_ROLE_ID: &str = "https://decision-cli.dev/ns#roleId";
const IRI_DEC_FEATURE_ID: &str = "https://decision-cli.dev/ns#featureId";
const IRI_DEC_STATUS: &str = "https://decision-cli.dev/ns#status";
const IRI_DEC_IN_STREAM: &str = "https://decision-cli.dev/ns#inStream";

/// Dispatch outcome flavours surfaced as the session's `dec:status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStatus {
    Completed,
    Failed,
}

impl DispatchStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// Commit (or re-confirm) the `dec:Session` quads for a worker dispatch.
///
/// Returns `true` when new quads were inserted; `false` when the IRI
/// was already typed as `dec:Session` (idempotent re-call). Errors
/// propagate the underlying store or writer failure.
///
/// `started_at` / `ended_at` are RFC3339 strings; `feature_id` is the
/// FT-NNN short id; `role_id` is the worker role string
/// (`"verify-graph-author"` or `"implementer"`).
#[allow(clippy::too_many_arguments)]
pub fn materialize(
    workdir: &std::path::Path,
    session_iri: &NamedNode,
    role_id: &str,
    feature_id: &str,
    started_at: &str,
    ended_at: &str,
    status: DispatchStatus,
) -> Result<bool> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .with_context(|| format!("opening orchestration store at {}", dump.display()))?;
    let store = Arc::new(store);

    if already_typed(&store, session_iri) {
        return Ok(false);
    }

    let scope = ActiveScope::load(workdir).context("loading active scope for dispatch session")?;
    let stream_iri =
        NamedNode::new(&scope.stream_iri).context("active stream iri for dispatch session")?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("opening writer for dispatch session materialise")?;

    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads = vec![
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
            NamedNodeRef::new_unchecked(IRI_DEC_SESSION).into_owned(),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
            NamedNodeRef::new_unchecked(PROV_ACTIVITY).into_owned(),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_ROLE_ID).into_owned(),
            Literal::new_simple_literal(role_id),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_FEATURE_ID).into_owned(),
            Literal::new_simple_literal(feature_id),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_STATUS).into_owned(),
            Literal::new_simple_literal(status.as_str()),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_IN_STREAM).into_owned(),
            stream_iri,
            g.clone(),
        ),
    ];
    if !started_at.is_empty() {
        quads.push(Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(PROV_STARTED_AT_TIME).into_owned(),
            Literal::new_simple_literal(started_at),
            g.clone(),
        ));
    }
    if !ended_at.is_empty() {
        quads.push(Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(PROV_ENDED_AT_TIME).into_owned(),
            Literal::new_simple_literal(ended_at),
            g.clone(),
        ));
    }
    writer.commit(Mutation::insert(quads)).with_context(|| {
        format!(
            "committing dispatch-session quads for {}",
            session_iri.as_str()
        )
    })?;
    persist_store(&store, &dump)
        .with_context(|| format!("persisting store after dispatch-session materialise"))?;
    Ok(true)
}

fn already_typed(store: &oxigraph::store::Store, iri: &NamedNode) -> bool {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let dec_session = NamedNodeRef::new_unchecked(IRI_DEC_SESSION);
    store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            Some(rdf_type),
            Some(Term::NamedNode(dec_session.into_owned()).as_ref()),
            None,
        )
        .filter_map(Result::ok)
        .next()
        .is_some()
}
