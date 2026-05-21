//! Verify-graph-author auto-dispatch subscription (FT-050 / ADR-030).
//!
//! When a `dec:Feature` artifact is created or updated and already lists
//! TCs, this subscription emits a `dec:VerifyGraphAuthorDispatchEvent`
//! per uncovered `(feature, env)` pair so the verify-graph-author worker
//! ([FT-048](FT-048)) is dispatched without anyone running
//! `dec verify graph generate` by hand.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, the handler lives
//! under `core/`: the subscription is platform substrate, not feature
//! volatile. The CLI verb that manually does the same thing is
//! [`features::verify_graph_generate`].
//!
//! The handler is intentionally synchronous in-process: subscriptions
//! evaluate against the post-commit store snapshot, and the matched
//! features drive emission. Idempotency is preserved across process
//! restarts by the in-store dedup ledger ([`ledger`]) — the ledger is
//! the structural guard FT-050 §Invariants names.

pub mod config;
pub mod ledger;
pub mod quads;
pub mod session;

use anyhow::Result;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    auto_dispatch_ledger_graph, IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER,
};

pub use self::config::{
    parse_from_str as parse_config_from_str, AutoDispatchConfig, DEFAULT_DEDUP_TTL_SECONDS,
    ENV_WILDCARD,
};
pub use self::ledger::{
    entry_iri as ledger_entry_iri, get_entry as ledger_get_entry, record_dispatch as ledger_record,
    within_ttl as ledger_within_ttl, LedgerEntry, LedgerError,
};
pub use self::quads::{build_event_quads as build_dispatch_event_quads, DispatchEventQuadInput};
pub use self::session::{
    load_proposal_document, persist_pending_review_session, PendingReviewError,
    PendingReviewInput, PendingReviewSession,
};

/// Stable IRI of the seeded verify-graph-author auto-dispatch subscription
/// (`core/subscriptions/seeds/verify_graph_author_dispatch.ttl`).
pub const VERIFY_GRAPH_AUTHOR_DISPATCH_SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/verify-graph-author-dispatch";

/// Opaque handler tag the subscription seed carries on `oxi:handler`. The
/// slice-2.6 harness binds this tag to [`emit_dispatch_event`].
pub const VERIFY_GRAPH_AUTHOR_DISPATCH_HANDLER: &str = "verify-graph-author-dispatch";

/// Raw Turtle bytes for the verify-graph-author dispatch subscription
/// seed, embedded via `include_str!`. Loaded into the orchestration
/// store at `dec init` time by [`crate::features::init`].
pub const VERIFY_GRAPH_AUTHOR_DISPATCH_SEED_TTL: &str =
    include_str!("../seeds/verify_graph_author_dispatch.ttl");

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// In-memory handle to a `dec:VerifyGraphAuthorDispatchEvent` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyGraphAuthorDispatchEvent {
    /// Stable IRI of the event artifact.
    pub iri: NamedNode,
    /// Short feature id literal (e.g. `FT-050`).
    pub feature: String,
    /// Short env id literal (e.g. `ENV-001-ephemeral-cli`).
    pub env: String,
    /// SHA-256 hex of the bundle.
    pub bundle_hash: String,
    /// IRI of the originating feature create/update event.
    pub triggered_by_event_id: String,
}

/// Errors surfaced by the auto-dispatch handler.
#[derive(Debug, Error)]
pub enum AutoDispatchError {
    /// Underlying SPARQL / store failure.
    #[error("auto-dispatch lookup: {0}")]
    Lookup(String),
    /// Could not mint a fresh IRI for the dispatch event.
    #[error("invalid event IRI: {0}")]
    IriMint(String),
    /// Underlying `StreamWriter::commit` failure.
    #[error("commit failed: {0}")]
    Commit(String),
    /// Ledger read/write failure.
    #[error("ledger: {0}")]
    Ledger(#[source] LedgerError),
}

impl From<LedgerError> for AutoDispatchError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

/// One `(feature, env)` pair the handler resolved to a dispatch event.
/// Mirrors `VerifierDispatchSeed`'s role for FT-022.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoDispatchSeed {
    /// Short feature id (e.g. `FT-K`).
    pub feature: String,
    /// Short env id (e.g. `ENV-001-ephemeral-cli`).
    pub env: String,
    /// IRI of the originating feature create/update event.
    pub triggered_by_event_id: String,
    /// SHA-256 hex of the bundle the worker would have seen.
    pub bundle_hash: String,
}

/// Idempotency guard: returns `true` iff the store already carries a
/// `dec:VerifyGraphAuthorDispatchEvent` for `(feature, env)` within the
/// configured TTL.
pub fn within_dedup_window(
    store: &Store,
    feature: &str,
    env: &str,
    cfg: &AutoDispatchConfig,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, AutoDispatchError> {
    ledger_within_ttl(store, feature, env, cfg.dedup_ttl_seconds, now).map_err(Into::into)
}

/// Emit a single `dec:VerifyGraphAuthorDispatchEvent` per `(feature, env)`
/// seed. Idempotent: if the ledger already shows a fresh row, returns
/// `Ok(None)`.
///
/// The emitted event lands in the orchestration graph and carries:
/// * `rdf:type dec:VerifyGraphAuthorDispatchEvent` and `rdf:type dec:Event`
///   (so the slice-1 outbox / SSE transport picks it up via the
///   scope-tagging path),
/// * `dec:eventClass "verify-graph-author-dispatch"`,
/// * `dec:targetRole "verify-graph-author"`,
/// * `dec:feature` literal (short id),
/// * `dec:environment` literal (short id),
/// * `dec:bundleHash` literal,
/// * `dec:triggeredByEventId` literal,
/// * `dec:emittedAt <rfc3339>`.
///
/// `dec:inStream` is added by [`StreamWriter::commit`] (ADR-005), since
/// `dec:Event` is one of `SCOPED_CLASSES`.
pub fn emit_dispatch_event(
    writer: &StreamWriter,
    store: &Store,
    seed: &AutoDispatchSeed,
    cfg: &AutoDispatchConfig,
    now_rfc3339: &str,
) -> Result<Option<VerifyGraphAuthorDispatchEvent>, AutoDispatchError> {
    let now = chrono::DateTime::parse_from_rfc3339(now_rfc3339)
        .map_err(|e| AutoDispatchError::Lookup(format!("parsing emitted_at: {e}")))?
        .with_timezone(&chrono::Utc);

    // Ledger dedup check FIRST so re-runs don't emit duplicates.
    if within_dedup_window(store, &seed.feature, &seed.env, cfg, now)? {
        return Ok(None);
    }

    let event_iri = mint_event_iri()?;
    let quads = build_dispatch_event_quads(&DispatchEventQuadInput {
        event_iri: &event_iri,
        feature_short: &seed.feature,
        env_short: &seed.env,
        bundle_hash: &seed.bundle_hash,
        triggered_by_event_id: &seed.triggered_by_event_id,
        emitted_at: now_rfc3339,
    });
    let mutation = oxi_events::Mutation::insert(quads.iter().cloned())
        .with_cause("FT-050 emit verify-graph-author-dispatch event");
    writer
        .commit(mutation)
        .map_err(|e| AutoDispatchError::Commit(format!("{e:#}")))?;

    // Refresh the ledger AFTER successful emission so a commit failure
    // doesn't poison the dedup window.
    ledger_record(writer, store, &seed.feature, &seed.env, now_rfc3339)?;

    Ok(Some(VerifyGraphAuthorDispatchEvent {
        iri: event_iri,
        feature: seed.feature.clone(),
        env: seed.env.clone(),
        bundle_hash: seed.bundle_hash.clone(),
        triggered_by_event_id: seed.triggered_by_event_id.clone(),
    }))
}

fn mint_event_iri() -> Result<NamedNode, AutoDispatchError> {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new(format!(
        "https://decision-cli.dev/ns/event/verify-graph-author-dispatch/{uuid}"
    ))
    .map_err(|e| AutoDispatchError::IriMint(e.to_string()))
}

/// Build the quad set that seeds the verify-graph-author-dispatch
/// subscription into the `oxi-events:subscriptions` named graph. Used
/// by the init pipeline alongside the other v0 subscription seeds.
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(VERIFY_GRAPH_AUTHOR_DISPATCH_SUBSCRIPTION_IRI);
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred =
        NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(sub.clone(), rdf_type, sub_cls, subs_graph.clone()),
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(
                "PREFIX dec: <https://decision-cli.dev/ns#>\nSELECT ?feature WHERE { ?feature a dec:Feature . }",
            ),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_ASYNC),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(VERIFY_GRAPH_AUTHOR_DISPATCH_HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal(
                "verify-graph-author auto-dispatch (new/updated feature has TCs and no covering graph)",
            ),
            subs_graph,
        ),
    ]
}

/// Pure helper exposed for tests / introspection: the ledger named graph
/// IRI, as a [`NamedNode`].
#[must_use]
pub fn ledger_graph_iri() -> NamedNode {
    auto_dispatch_ledger_graph().into_owned()
}

/// Named graph IRI string for the auto-dispatch ledger (mirrors the
/// constant in `core::vocab::auto_dispatch`).
#[must_use]
pub const fn ledger_graph_iri_str() -> &'static str {
    IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER
}

#[cfg(test)]
mod tests;
