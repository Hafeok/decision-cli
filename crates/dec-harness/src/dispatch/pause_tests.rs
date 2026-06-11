//! Unit tests for [`super::pause`] (FT-032 / ADR-025).
//!
//! Split out of `pause.rs` so the implementation file stays under
//! ADR-013 Rule 1's 400-line hard cap.

use std::sync::Arc;

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;

use super::*;
use crate::dispatch::group::DispatchGroup;
use crate::dispatch::lifecycle::{DispatchEvent, DispatchStatus};
use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::{
    feedback_class, in_stream, lifecycle_state, orchestration_graph, IRI_DEC_DISPATCH_STATUS,
};

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn mint_group(writer: &StreamWriter, suffix: &str) -> (NamedNode, NamedNode) {
    let group_iri = NamedNode::new(format!("urn:dec:group:{suffix}")).expect("group iri");
    let action_iri =
        NamedNode::new(format!("urn:dec:session/action/{suffix}")).expect("action iri");
    DispatchGroup::mint(writer, group_iri.clone(), action_iri.clone(), "FT-032")
        .expect("mint group");
    (group_iri, action_iri)
}

/// Insert a `dec:Feedback` row in `lifecycle_state = state`. Bypasses
/// the full FT-026 SHACL — these tests only care about the pause/resume
/// state machinery.
fn seed_feedback(store: &Store, suffix: &str, state: &str) -> NamedNode {
    let g: GraphName = orchestration_graph().into_owned().into();
    let iri = NamedNode::new(format!("urn:dec:feedback:{suffix}")).expect("feedback iri");
    store
        .transaction(|mut tx| insert_feedback_quads(&mut tx, &iri, state, &g))
        .expect("seed feedback");
    iri
}

fn insert_feedback_quads(
    tx: &mut oxigraph::store::Transaction<'_>,
    iri: &NamedNode,
    state: &str,
    g: &GraphName,
) -> Result<(), oxigraph::store::StorageError> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let cls = feedback_class().into_owned();
    tx.insert(Quad::new(iri.clone(), rdf_type, cls, g.clone()).as_ref())?;
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
    Ok(())
}

fn current_status(store: &Store, group_iri: &NamedNode) -> Option<String> {
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(group_iri.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if q.predicate.as_str() == IRI_DEC_DISPATCH_STATUS {
            if let Term::Literal(lit) = q.object {
                return Some(lit.value().to_string());
            }
        }
    }
    None
}

#[test]
fn pause_transitions_group_to_paused_and_writes_blocked_by() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "pause-1");
    let fb = seed_feedback(&store, "pause-1", "produced");
    let status = pause_on_feedback(&w, &store, &group, &[fb.clone()]).expect("pause");
    assert_eq!(status, DispatchStatus::PausedForFeedback);
    assert_eq!(
        current_status(&store, &group),
        Some("paused-for-feedback".to_string())
    );
    let blocked = list_blocked_by(&store, &group).expect("list");
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].feedback, fb);
}

#[test]
fn pause_refuses_groups_outside_awaiting_action() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "pause-2");
    let fb = seed_feedback(&store, "pause-2", "produced");
    // Transition past awaiting-action first.
    let mut g = DispatchGroup::read(&store, &group)
        .expect("read")
        .expect("present");
    g.transition(&w, DispatchEvent::ActionCompleted)
        .expect("advance to awaiting-interpretation");
    let err = pause_on_feedback(&w, &store, &group, &[fb])
        .expect_err("pause refused outside awaiting-action");
    assert!(matches!(err, PauseError::NotAwaitingAction { .. }));
}

#[test]
fn resume_check_is_noop_while_feedback_is_still_open() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "resume-1");
    let fb = seed_feedback(&store, "resume-1", "routed");
    pause_on_feedback(&w, &store, &group, &[fb]).expect("pause");
    let outcome = resume_check(&w, &store, &group).expect("check");
    assert!(matches!(
        outcome,
        ResumeOutcome::StillBlocked { pending: 1 }
    ));
    assert_eq!(
        current_status(&store, &group),
        Some("paused-for-feedback".to_string())
    );
}

#[test]
fn resume_check_resumes_when_every_blocker_is_addressed() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "resume-2");
    let fb1 = seed_feedback(&store, "resume-2a", "addressed");
    let fb2 = seed_feedback(&store, "resume-2b", "closed");
    pause_on_feedback(&w, &store, &group, &[fb1, fb2]).expect("pause");
    let outcome = resume_check(&w, &store, &group).expect("check");
    assert_eq!(outcome, ResumeOutcome::Resumed);
    // After resume the group is back to awaiting-action so the
    // orchestrator can re-dispatch the action.
    assert_eq!(
        current_status(&store, &group),
        Some("awaiting-action".to_string())
    );
}

#[test]
fn resume_check_blocks_when_any_feedback_was_rejected() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "resume-3");
    let fb1 = seed_feedback(&store, "resume-3a", "addressed");
    let fb2 = seed_feedback(&store, "resume-3b", "rejected");
    pause_on_feedback(&w, &store, &group, &[fb1, fb2]).expect("pause");
    let outcome = resume_check(&w, &store, &group).expect("check");
    assert_eq!(outcome, ResumeOutcome::Blocked);
    assert_eq!(
        current_status(&store, &group),
        Some("feedback-rejected-action-blocked".to_string())
    );
}

#[test]
fn list_paused_groups_for_feedback_finds_blocked_group() {
    let (store, w) = writer();
    let (group, _) = mint_group(&w, "find-1");
    let fb = seed_feedback(&store, "find-1", "produced");
    pause_on_feedback(&w, &store, &group, &[fb.clone()]).expect("pause");
    let found = list_paused_groups_for_feedback(&store, &fb).expect("query");
    assert_eq!(found, vec![group]);
}
