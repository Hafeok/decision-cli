//! Verifier-dispatch subscription handler (FT-022 / ADR-017).
//!
//! The seed in `seeds/verifier_dispatch.ttl` is the persisted matcher;
//! this module owns the *handler* side — the small piece of logic that
//! turns a match into a `dec:VerifierDispatchEvent` artifact in the
//! orchestration store.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, the handler lives
//! under `core/`: any feature that mints a `dec:DispatchGroup` (slice 2's
//! `features/implement`, future role-dispatch flows) drives the same
//! handler. Feature modules call [`dispatch_pending_groups`] once they
//! have transitioned a group into `awaiting-interpretation` so the
//! event is emitted synchronously alongside the action's completion
//! commit — the verifier worker (FT-023) then picks it up via the
//! outbox (FT-003) or SSE transport (FT-004).
//!
//! The handler is intentionally synchronous in-process: subscriptions
//! evaluate against the post-commit store snapshot, and the matched
//! `(group, actionSession)` pairs drive the emission. FT-022's
//! idempotency clause (`FILTER NOT EXISTS { ?group dec:hasInterpretationSession ?s }`)
//! lives in the SPARQL — the registry's diff semantics give us
//! at-most-once-per-group within a single process; we add a Rust-side
//! guard (`already_dispatched`) so re-firing across processes (or after
//! a crash before the verifier acks) never produces a duplicate event.

mod seed;

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    bundle_seed, dispatch_group_ref, emitted_at, event_class, in_stream, orchestration_graph,
    target_role, verifier_dispatch_event_class, EVENT_CLASS_VERIFIER_DISPATCH, IRI_DEC_EVENT,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Stable IRI of the seeded verifier-dispatch subscription
/// (`crates/decision-cli/src/core/subscriptions/seeds/verifier_dispatch.ttl`).
/// Callers reference this constant when asserting the subscription is
/// present in a freshly-initialised store.
pub const VERIFIER_DISPATCH_SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/verifier-dispatch";

/// Opaque handler tag the subscription seed carries on `oxi:handler`. The
/// slice-2 harness binds this tag to [`emit_verifier_dispatch_event`].
pub const VERIFIER_DISPATCH_HANDLER: &str = "verifier-dispatch";

/// Stable role id the dispatched event targets (FT-019 / [`crate::core::role_catalog`]).
pub const VERIFIER_TARGET_ROLE: &str = "verifier";

/// Raw Turtle bytes for the verifier-dispatch subscription seed, embedded
/// via `include_str!`. Loaded into the orchestration store at `dec init`
/// time by [`crate::features::init`].
pub const VERIFIER_DISPATCH_SEED_TTL: &str = include_str!("../seeds/verifier_dispatch.ttl");

/// One `(DispatchGroup, ActionSession)` pair selected by the seed
/// subscription. Returned from [`dispatch_pending_groups`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierDispatchSeed {
    /// IRI of the `dec:DispatchGroup` in `awaiting-interpretation`.
    pub group: NamedNode,
    /// IRI of the paired `dec:ActionSession` that produced the artifact.
    pub action_session: NamedNode,
}

/// In-memory handle to an emitted `dec:VerifierDispatchEvent` artifact.
/// Mirrors the quads written to the orchestration store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierDispatchEvent {
    /// Stable IRI of the event artifact.
    pub iri: NamedNode,
    /// `dec:dispatchGroup` link.
    pub group: NamedNode,
    /// `dec:bundleSeed` link (the action session that seeds the bundle).
    pub bundle_seed: NamedNode,
    /// `dec:targetRole` literal.
    pub target_role: String,
}

/// Errors surfaced by the verifier-dispatch handler.
#[derive(Debug, Error)]
pub enum VerifierDispatchError {
    /// Underlying SPARQL / store failure.
    #[error("dispatch group lookup: {0}")]
    Lookup(String),

    /// Could not mint a fresh IRI for the dispatch event.
    #[error("invalid event IRI: {0}")]
    IriMint(String),

    /// Underlying `StreamWriter::commit` failure.
    #[error("commit failed: {0}")]
    Commit(String),
}

/// Run the subscription's `SELECT` body against the post-commit store
/// snapshot and return every `(group, action_session)` pair that needs a
/// verifier-dispatch event.
///
/// Mirrors the SPARQL embedded in `seeds/verifier_dispatch.ttl`. We
/// re-execute the query rather than rely on the registry's diff so the
/// handler is correct across process restarts (the registry's cached
/// prior result set is in-memory only — a fresh process sees an empty
/// cache and would re-emit every existing group otherwise).
pub fn dispatch_pending_groups(store: &Store) -> Result<Vec<VerifierDispatchSeed>> {
    let q = include_str!("../seeds/verifier_dispatch.ttl");
    // The full TTL is the persisted form; we have to extract just the
    // SPARQL body. For Phase A the simplest correct path is to keep the
    // query inline here — the test suite asserts that the inline string
    // matches the seed's persisted `oxi:selectQuery`.
    let _ = q; // silence unused warning when the constant is re-used in tests.
    let bindings = run_pending_query(store).map_err(VerifierDispatchError::Lookup)?;
    Ok(bindings)
}

/// Inline copy of the SPARQL body in `seeds/verifier_dispatch.ttl`,
/// adapted with a `GRAPH` UNION so it matches DispatchGroup quads
/// regardless of whether they live in the default graph or the
/// orchestration named graph (DispatchGroup quads are written into the
/// `dec:graph/orchestration` named graph by FT-021's quad builders).
///
/// Kept here so the runtime handler does not have to parse the Turtle
/// seed to recover the query. The seed TTL carries the *logical* query;
/// the runtime const carries the same logical query with the graph
/// indirection a SPARQL engine needs to find the rows.
pub(crate) const PENDING_GROUPS_QUERY: &str = "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?group ?actionSession WHERE {
  { ?group a dec:DispatchGroup ;
           dec:dispatchStatus \"awaiting-interpretation\" ;
           dec:hasActionSession ?actionSession .
    FILTER NOT EXISTS { ?group dec:hasInterpretationSession ?interpretationSession } }
  UNION
  { GRAPH ?g {
      ?group a dec:DispatchGroup ;
             dec:dispatchStatus \"awaiting-interpretation\" ;
             dec:hasActionSession ?actionSession .
      FILTER NOT EXISTS { ?group dec:hasInterpretationSession ?interpretationSession }
    } }
}";

fn run_pending_query(store: &Store) -> Result<Vec<VerifierDispatchSeed>, String> {
    let solutions = match store
        .query(PENDING_GROUPS_QUERY)
        .map_err(|e| format!("running verifier-dispatch query: {e}"))?
    {
        QueryResults::Solutions(s) => s,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<VerifierDispatchSeed> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for sol in solutions {
        let sol = sol.map_err(|e| format!("decoding row: {e}"))?;
        let Some(Term::NamedNode(group)) = sol.get("group").cloned() else {
            continue;
        };
        let Some(Term::NamedNode(action)) = sol.get("actionSession").cloned() else {
            continue;
        };
        // The SELECT can return the same ?group twice if an
        // action-session is shared (it should not be — guard anyway).
        if !seen.insert(group.as_str().to_string()) {
            continue;
        }
        out.push(VerifierDispatchSeed {
            group,
            action_session: action,
        });
    }
    Ok(out)
}

/// Idempotency guard: returns `true` iff the store already carries a
/// `dec:VerifierDispatchEvent` linking to `group`. Used by
/// [`emit_verifier_dispatch_event`] so a re-run after a crash does not
/// produce a duplicate event (FT-022 §Invariants).
pub fn already_dispatched(store: &Store, group: &NamedNode) -> Result<bool> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ \
           {{ ?ev a dec:VerifierDispatchEvent ; dec:dispatchGroup <{g}> . }} \
           UNION \
           {{ GRAPH ?h {{ ?ev a dec:VerifierDispatchEvent ; dec:dispatchGroup <{g}> . }} }} \
         }}",
        g = group.as_str(),
    );
    match store.query(q.as_str()).context("ask existing dispatch")? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}

/// Emit a single `dec:VerifierDispatchEvent` for one pending seed. Idempotent:
/// if the store already carries an event for `seed.group`, returns `Ok(None)`.
///
/// The emitted event lands in the orchestration graph and carries:
///
/// * `rdf:type dec:VerifierDispatchEvent` and `rdf:type dec:Event` (so
///   the slice-1 outbox / SSE transport picks it up as any other
///   `dec:Event` via the scope-tagging path),
/// * `dec:eventClass "verifier-dispatch"`,
/// * `dec:targetRole "verifier"`,
/// * `dec:dispatchGroup <group>`,
/// * `dec:bundleSeed <action_session>` (the verifier worker harness
///   resolves the produced artifact from the session per FT-023), and
/// * `dec:emittedAt <rfc3339>`.
///
/// `dec:inStream` is added by [`StreamWriter::commit`] (ADR-005), since
/// `dec:Event` is one of `SCOPED_CLASSES`.
pub fn emit_verifier_dispatch_event(
    writer: &StreamWriter,
    store: &Store,
    seed: &VerifierDispatchSeed,
    emitted_at_rfc3339: &str,
) -> Result<Option<VerifierDispatchEvent>, VerifierDispatchError> {
    if already_dispatched(store, &seed.group)
        .map_err(|e| VerifierDispatchError::Lookup(format!("{e:#}")))?
    {
        return Ok(None);
    }
    let event_iri = mint_event_iri()?;
    let quads = build_event_quads(&event_iri, seed, emitted_at_rfc3339);
    let mutation =
        Mutation::insert(quads.iter().cloned()).with_cause("FT-022 emit verifier-dispatch event");
    writer
        .commit(mutation)
        .map_err(|e| VerifierDispatchError::Commit(format!("{e:#}")))?;
    Ok(Some(VerifierDispatchEvent {
        iri: event_iri,
        group: seed.group.clone(),
        bundle_seed: seed.action_session.clone(),
        target_role: VERIFIER_TARGET_ROLE.to_string(),
    }))
}

fn mint_event_iri() -> Result<NamedNode, VerifierDispatchError> {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new(format!(
        "https://decision-cli.dev/ns/event/verifier-dispatch/{uuid}"
    ))
    .map_err(|e| VerifierDispatchError::IriMint(e.to_string()))
}

/// Build the canonical quad set for a `dec:VerifierDispatchEvent` artifact.
/// Pure / no IO; tests assert the predicate cardinalities directly.
#[must_use]
pub fn build_event_quads(
    event_iri: &NamedNode,
    seed: &VerifierDispatchSeed,
    emitted_at_rfc3339: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads = build_event_type_quads(event_iri, &g);
    quads.extend(build_event_payload_quads(
        event_iri,
        seed,
        emitted_at_rfc3339,
        &g,
    ));
    quads
}

fn build_event_type_quads(event_iri: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    // Two type quads: the abstract dec:Event (so ADR-005 scope
    // tagging fires) plus the concrete dec:VerifierDispatchEvent.
    vec![
        Quad::new(
            event_iri.clone(),
            rdf_type.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_EVENT).into_owned(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            rdf_type,
            verifier_dispatch_event_class().into_owned(),
            g.clone(),
        ),
    ]
}

fn build_event_payload_quads(
    event_iri: &NamedNode,
    seed: &VerifierDispatchSeed,
    emitted_at_rfc3339: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let mut quads = build_event_role_quads(event_iri, g);
    quads.extend(build_event_link_quads(
        event_iri,
        seed,
        emitted_at_rfc3339,
        g,
    ));
    quads
}

fn build_event_role_quads(event_iri: &NamedNode, g: &GraphName) -> Vec<Quad> {
    vec![
        Quad::new(
            event_iri.clone(),
            event_class().into_owned(),
            Literal::new_simple_literal(EVENT_CLASS_VERIFIER_DISPATCH),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            target_role().into_owned(),
            Literal::new_simple_literal(VERIFIER_TARGET_ROLE),
            g.clone(),
        ),
    ]
}

fn build_event_link_quads(
    event_iri: &NamedNode,
    seed: &VerifierDispatchSeed,
    emitted_at_rfc3339: &str,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        Quad::new(
            event_iri.clone(),
            dispatch_group_ref().into_owned(),
            seed.group.clone(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            bundle_seed().into_owned(),
            seed.action_session.clone(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            emitted_at().into_owned(),
            Literal::new_simple_literal(emitted_at_rfc3339),
            g.clone(),
        ),
    ]
}

/// Build the quad set that seeds the verifier-dispatch subscription into
/// the `oxi-events:subscriptions` named graph. Used by the init pipeline
/// alongside the slice-1 v0 bootstrap subscriptions (FT-009).
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    seed::seed_quads()
}

/// Convenience: shape the `dec:inStream` predicate as a NamedNode so
/// callers building synthetic quads in tests can reuse the same IRI the
/// writer uses. (Kept here rather than in `vocab.rs` so the surface stays
/// minimal.)
#[must_use]
pub fn in_stream_predicate() -> NamedNode {
    in_stream().into_owned()
}

#[cfg(test)]
mod tests;
