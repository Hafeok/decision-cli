//! Feedback-resume subscription handler (FT-032 / ADR-025).
//!
//! The seed in `seeds/feedback_resume.ttl` watches for
//! `dec:Feedback` artifacts whose `dec:lifecycleState` has reached a
//! terminal value (`addressed`, `rejected`, `closed`) AND that are
//! referenced by a paused `dec:DispatchGroup` via `dec:blockedBy`. The
//! handler invokes [`core::dispatch::resume_check`] on each matched
//! group: if every blocking feedback is addressed/closed the group
//! transitions back to `awaiting-action`; if any is `rejected` the
//! group transitions to `feedback-rejected-action-blocked`.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, the handler lives
//! in `core/`: every feature that consumes the resume signal (FT-031
//! worker harness, FT-033 CLI close commands, future Phase-B retry
//! features) drives it through this single entry point.

use std::collections::BTreeSet;

use anyhow::Result;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::dispatch::{resume_check, ResumeError, ResumeOutcome};
use crate::core::stream_writer::StreamWriter;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Stable IRI of the seeded feedback-resume subscription
/// (`crates/decision-cli/src/core/subscriptions/seeds/feedback_resume.ttl`).
pub const FEEDBACK_RESUME_SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/feedback-resume";

/// Opaque handler tag the seed carries on `oxi:handler`. The harness
/// binds this tag to [`handle_pending`].
pub const FEEDBACK_RESUME_HANDLER: &str = "feedback-resume";

/// Raw Turtle bytes for the feedback-resume subscription seed, embedded
/// via `include_str!`. Loaded into the orchestration store at `dec init`
/// time by [`crate::features::init`].
pub const FEEDBACK_RESUME_SEED_TTL: &str = include_str!("seeds/feedback_resume.ttl");

/// Inline copy of the SPARQL body in `seeds/feedback_resume.ttl`,
/// adapted with a `GRAPH ?g UNION` so it matches DispatchGroup quads in
/// the orchestration named graph (FT-021's quad builders write the
/// group quads into `dec:graph/orchestration`).
pub(crate) const PENDING_RESUME_QUERY: &str = "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT DISTINCT ?group WHERE {
  { ?group a dec:DispatchGroup ;
           dec:dispatchStatus \"paused-for-feedback\" ;
           dec:blockedBy ?feedback .
    ?feedback dec:lifecycleState ?state .
    FILTER(?state IN (\"addressed\", \"rejected\", \"closed\")) }
  UNION
  { GRAPH ?g {
      ?group a dec:DispatchGroup ;
             dec:dispatchStatus \"paused-for-feedback\" ;
             dec:blockedBy ?feedback .
      ?feedback dec:lifecycleState ?state .
      FILTER(?state IN (\"addressed\", \"rejected\", \"closed\"))
    } }
}";

/// Errors surfaced by [`handle_pending`].
#[derive(Debug, Error)]
pub enum FeedbackResumeError {
    /// Underlying SPARQL / store failure.
    #[error("resume subscription lookup: {0}")]
    Lookup(String),
    /// `resume_check` returned an error for a specific group.
    #[error("resume_check for <{group}> failed: {source}")]
    Resume {
        /// IRI of the failing group.
        group: String,
        /// Underlying resume error.
        #[source]
        source: ResumeError,
    },
}

/// One handled row from the resume subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandledGroup {
    /// IRI of the resumed / blocked group.
    pub group: NamedNode,
    /// Outcome of the [`resume_check`] call.
    pub outcome: ResumeOutcome,
}

/// Run the resume subscription's `SELECT` against the post-commit store
/// snapshot and call [`resume_check`] for every matched paused group.
///
/// Returns one [`HandledGroup`] per group processed (including no-op
/// `StillBlocked` outcomes — useful for telemetry). Errors surface
/// per-group via [`FeedbackResumeError::Resume`]; the first failing row
/// short-circuits the loop so the caller can decide whether to retry.
pub fn handle_pending(
    writer: &StreamWriter,
    store: &Store,
) -> Result<Vec<HandledGroup>, FeedbackResumeError> {
    let groups = pending_groups(store).map_err(FeedbackResumeError::Lookup)?;
    let mut handled: Vec<HandledGroup> = Vec::with_capacity(groups.len());
    for group in groups {
        let outcome =
            resume_check(writer, store, &group).map_err(|source| FeedbackResumeError::Resume {
                group: group.as_str().to_string(),
                source,
            })?;
        handled.push(HandledGroup { group, outcome });
    }
    Ok(handled)
}

/// Build the quad set for the feedback-resume subscription seed. Used
/// by `dec init` (FT-009 §Behaviour step 4) so the subscription is
/// available from the first dispatch.
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(FEEDBACK_RESUME_SUBSCRIPTION_IRI);
    let mut quads = seed_type_quads(&sub, &subs_graph);
    quads.extend(seed_query_quads(&sub, &subs_graph));
    quads.extend(seed_metadata_quads(sub, &subs_graph));
    quads
}

fn seed_type_quads(sub: &NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    vec![Quad::new(
        sub.clone(),
        rdf_type,
        sub_cls,
        subs_graph.clone(),
    )]
}

fn seed_query_quads(sub: &NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    vec![
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(PENDING_RESUME_QUERY),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_INLINE),
            subs_graph.clone(),
        ),
    ]
}

fn seed_metadata_quads(sub: NamedNode, subs_graph: &GraphName) -> Vec<Quad> {
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(FEEDBACK_RESUME_HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal(
                "feedback resume check (terminal feedback unblocks paused dispatch)",
            ),
            subs_graph.clone(),
        ),
    ]
}

fn pending_groups(store: &Store) -> Result<Vec<NamedNode>, String> {
    let solutions = match store
        .query(PENDING_RESUME_QUERY)
        .map_err(|e| format!("running feedback-resume query: {e}"))?
    {
        QueryResults::Solutions(s) => s,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<NamedNode> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for sol in solutions {
        let sol = sol.map_err(|e| format!("decoding row: {e}"))?;
        if let Some(Term::NamedNode(group)) = sol.get("group").cloned() {
            if seen.insert(group.as_str().to_string()) {
                out.push(group);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use oxigraph::model::{GraphName, Literal, NamedNodeRef, Quad};
    use oxigraph::store::Store;

    use super::*;
    use crate::core::dispatch::{pause_on_feedback, DispatchGroup};
    use crate::core::stream_writer::StreamWriter;
    use crate::core::vocab::{feedback_class, in_stream, lifecycle_state, orchestration_graph};

    const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
    const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

    fn writer() -> (Arc<Store>, StreamWriter) {
        let store = Arc::new(Store::new().expect("in-memory store"));
        let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
        let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
        (store, w)
    }

    fn seed_feedback(store: &Store, suffix: &str, state: &str) -> NamedNode {
        let g: GraphName = orchestration_graph().into_owned().into();
        let iri = NamedNode::new(format!("urn:dec:feedback:{suffix}")).expect("feedback iri");
        store
            .transaction(|mut tx| {
                tx.insert(
                    Quad::new(
                        iri.clone(),
                        NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
                        feedback_class().into_owned(),
                        g.clone(),
                    )
                    .as_ref(),
                )?;
                tx.insert(
                    Quad::new(
                        iri.clone(),
                        lifecycle_state().into_owned(),
                        Literal::new_simple_literal(state),
                        g.clone(),
                    )
                    .as_ref(),
                )?;
                tx.insert(
                    Quad::new(
                        iri.clone(),
                        in_stream().into_owned(),
                        NamedNode::new(STREAM_IRI).expect("stream iri"),
                        g.clone(),
                    )
                    .as_ref(),
                )?;
                Ok::<_, oxigraph::store::StorageError>(())
            })
            .expect("seed feedback");
        iri
    }

    fn set_feedback_state(store: &Store, fb: &NamedNode, new_state: &str) {
        let g: GraphName = orchestration_graph().into_owned().into();
        store
            .transaction(|mut tx| {
                let prior: Vec<Quad> = store
                    .quads_for_pattern(
                        Some(oxigraph::model::Subject::NamedNode(fb.clone()).as_ref()),
                        Some(lifecycle_state()),
                        None,
                        None,
                    )
                    .filter_map(Result::ok)
                    .collect();
                for q in &prior {
                    tx.remove(q.as_ref())?;
                }
                tx.insert(
                    Quad::new(
                        fb.clone(),
                        lifecycle_state().into_owned(),
                        Literal::new_simple_literal(new_state),
                        g.clone(),
                    )
                    .as_ref(),
                )?;
                Ok::<_, oxigraph::store::StorageError>(())
            })
            .expect("update feedback state");
    }

    fn mint_paused_group(
        store: &Store,
        writer: &StreamWriter,
        suffix: &str,
        fb: &NamedNode,
    ) -> NamedNode {
        let group_iri = NamedNode::new(format!("urn:dec:group:{suffix}")).expect("group iri");
        let action_iri =
            NamedNode::new(format!("urn:dec:session/action/{suffix}")).expect("action iri");
        DispatchGroup::mint(writer, group_iri.clone(), action_iri, "FT-032").expect("mint");
        pause_on_feedback(writer, store, &group_iri, &[fb.clone()]).expect("pause");
        group_iri
    }

    #[test]
    fn handler_resumes_paused_group_when_feedback_addressed() {
        let (store, w) = writer();
        let fb = seed_feedback(&store, "resume-h-1", "produced");
        let group = mint_paused_group(&store, &w, "resume-h-1", &fb);
        // Worker addresses the feedback — drive the literal directly so
        // the test does not depend on the addressing-artifact SHACL.
        set_feedback_state(&store, &fb, "addressed");
        let handled = handle_pending(&w, &store).expect("handler");
        assert_eq!(handled.len(), 1);
        assert_eq!(handled[0].group, group);
        assert_eq!(handled[0].outcome, ResumeOutcome::Resumed);
    }

    #[test]
    fn handler_marks_blocked_when_feedback_rejected() {
        let (store, w) = writer();
        let fb = seed_feedback(&store, "resume-h-2", "produced");
        mint_paused_group(&store, &w, "resume-h-2", &fb);
        set_feedback_state(&store, &fb, "rejected");
        let handled = handle_pending(&w, &store).expect("handler");
        assert_eq!(handled.len(), 1);
        assert_eq!(handled[0].outcome, ResumeOutcome::Blocked);
    }

    #[test]
    fn handler_skips_groups_whose_blockers_are_still_open() {
        let (store, w) = writer();
        let fb = seed_feedback(&store, "resume-h-3", "produced");
        mint_paused_group(&store, &w, "resume-h-3", &fb);
        // No state change — feedback stays "produced"; subscription's
        // FILTER excludes the row.
        let handled = handle_pending(&w, &store).expect("handler");
        assert!(handled.is_empty(), "no terminal blocker ⇒ no handled row");
    }
}
