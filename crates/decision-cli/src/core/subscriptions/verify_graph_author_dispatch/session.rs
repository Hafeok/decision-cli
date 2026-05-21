//! Pending-review session writer (FT-050 / ADR-030).
//!
//! When the auto-dispatch orchestrator runs the worker and receives a
//! `New` proposal, it creates a `dec:Session` artifact whose `dec:status`
//! is `pending_review` and whose `dec:proposalDocument` carries the
//! JSON-serialised proposal. The graph itself is NOT persisted; the
//! reviewer accepts via `dec verify graph generate <feature>
//! --environment <env> --from-session <id> --accept`.
//!
//! Per ADR-005 the session lands in the orchestration named graph
//! through the [`StreamWriter`] chokepoint so `dec:inStream` is tagged.

use anyhow::Result;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    orchestration_graph, proposal_document, IRI_DEC_ENVIRONMENT, IRI_DEC_SESSION,
    IRI_DEC_VERIFIES, SESSION_STATUS_PENDING_REVIEW,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";
const DEC_FEATURE_ID: &str = "https://decision-cli.dev/ns#featureId";

/// Errors surfaced when persisting a pending-review session.
#[derive(Debug, Error)]
pub enum PendingReviewError {
    /// IRI mint failure.
    #[error("invalid session IRI: {0}")]
    IriMint(String),
    /// Commit failure through the StreamWriter chokepoint.
    #[error("commit failed: {0}")]
    Commit(String),
}

/// Inputs to [`persist_pending_review_session`].
#[derive(Debug, Clone)]
pub struct PendingReviewInput<'a> {
    /// Short feature id (e.g. `FT-J`).
    pub feature: &'a str,
    /// Short env id (e.g. `ENV-001-ephemeral-cli`).
    pub env: &'a str,
    /// JSON string of the `GraphProposal` payload returned by the worker.
    pub proposal_document_json: &'a str,
    /// IRI of the dispatch event the session reacts to (for PROV-O).
    pub dispatch_event_iri: &'a NamedNode,
    /// RFC3339 timestamp.
    pub started_at: &'a str,
}

/// Result handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReviewSession {
    /// Stable IRI of the session.
    pub iri: NamedNode,
    /// Short feature id.
    pub feature: String,
    /// Short env id.
    pub env: String,
    /// The proposal document literal as stored.
    pub proposal_document_json: String,
}

/// Persist a `pending_review` session for a `(feature, env)` proposal.
///
/// Per TC-085 AC #1, the session carries:
/// * `status = pending_review`,
/// * `dec:proposalDocument = <JSON>`,
/// * `dec:verifies = FT-J`,
/// * `dec:environment = ENV-1`.
///
/// And critically (TC-085 AC #2) — no `VerificationGraph` is written.
pub fn persist_pending_review_session(
    writer: &StreamWriter,
    input: &PendingReviewInput<'_>,
) -> Result<PendingReviewSession, PendingReviewError> {
    let session_iri = mint_session_iri(input.feature, input.env)?;
    let quads = build_session_quads(&session_iri, input);
    let mutation = Mutation::insert(quads.iter().cloned())
        .with_cause(format!(
            "FT-050 pending-review session for ({feat}, {env})",
            feat = input.feature,
            env = input.env
        ));
    writer
        .commit(mutation)
        .map_err(|e| PendingReviewError::Commit(format!("{e:#}")))?;
    Ok(PendingReviewSession {
        iri: session_iri,
        feature: input.feature.to_string(),
        env: input.env.to_string(),
        proposal_document_json: input.proposal_document_json.to_string(),
    })
}

fn mint_session_iri(feature: &str, env: &str) -> Result<NamedNode, PendingReviewError> {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new(format!(
        "urn:dec:session/verify-graph-author-dispatch/{feature}/{env}/{uuid}"
    ))
    .map_err(|e| PendingReviewError::IriMint(e.to_string()))
}

fn build_session_quads(
    session_iri: &NamedNode,
    input: &PendingReviewInput<'_>,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let session_class = NamedNodeRef::new_unchecked(IRI_DEC_SESSION).into_owned();
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY).into_owned();
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS).into_owned();
    let feature_id_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let environment_pred = NamedNodeRef::new_unchecked(IRI_DEC_ENVIRONMENT).into_owned();
    let verifies_pred = NamedNodeRef::new_unchecked(IRI_DEC_VERIFIES).into_owned();
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    vec![
        Quad::new(session_iri.clone(), rdf_type.clone(), session_class, g.clone()),
        Quad::new(session_iri.clone(), rdf_type, activity, g.clone()),
        Quad::new(
            session_iri.clone(),
            status_pred,
            Literal::new_simple_literal(SESSION_STATUS_PENDING_REVIEW),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            feature_id_pred,
            Literal::new_simple_literal(input.feature),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            verifies_pred,
            Literal::new_simple_literal(input.feature),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            environment_pred,
            Literal::new_simple_literal(input.env),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            proposal_document().into_owned(),
            Literal::new_simple_literal(input.proposal_document_json),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            at_time,
            Literal::new_simple_literal(input.started_at),
            g,
        ),
    ]
}

/// Look up a pending-review session by IRI, returning the stored
/// proposal document literal (the JSON string). Returns `Ok(None)` when
/// the session does not exist or is not `pending_review`.
pub fn load_proposal_document(
    store: &Store,
    session_iri: &NamedNode,
) -> Result<Option<String>> {
    use oxigraph::model::Subject;

    // Confirm the session is pending_review.
    let mut is_pending = false;
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS);
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(session_iri.clone()).as_ref()),
            Some(status_pred),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let oxigraph::model::Term::Literal(lit) = &q.object {
            if lit.value() == SESSION_STATUS_PENDING_REVIEW {
                is_pending = true;
                break;
            }
        }
    }
    if !is_pending {
        return Ok(None);
    }
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(session_iri.clone()).as_ref()),
            Some(proposal_document()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let oxigraph::model::Term::Literal(lit) = &q.object {
            return Ok(Some(lit.value().to_string()));
        }
    }
    Ok(None)
}
