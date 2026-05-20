//! Blocking-feedback pause/resume substrate for DispatchGroups (FT-032 / ADR-025).
//!
//! When a worker emits blocking feedback, the orchestrator drives the
//! group's `dec:dispatchStatus` from `awaiting-action` to
//! `paused-for-feedback`, recording the blocking feedback IRIs via
//! `dec:blockedBy`. The group refuses to advance to
//! `awaiting-interpretation` until every blocking feedback in
//! `dec:blockedBy` is `addressed` (resume) or `rejected` (terminal).
//!
//! This module exposes the two entry points FT-032 §Behaviour names:
//!
//!   * [`pause_on_feedback`] — called by the worker harness (FT-031) when
//!     it detects a blocking emission. Drops the action artifact upstream
//!     and parks the group.
//!   * [`resume_check`] — called by the resume subscription (also here)
//!     when a feedback artifact reaches a terminal state. Computes the
//!     correct lifecycle transition and applies it through
//!     [`StreamWriter`] so SHACL has the chance to refuse a malformed
//!     resume.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, both APIs live in
//! `core/`: every consumer (FT-031 worker harness, FT-033 CLI close,
//! future Phase-B retry features) imports them through this module.

use std::collections::BTreeSet;

use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{IRI_DEC_BLOCKED_BY, IRI_DEC_LIFECYCLE_STATE};

use super::group::DispatchGroup;
use super::lifecycle::{DispatchEvent, DispatchStatus, LifecycleError};
use super::quads::build_blocked_by_quad;

/// One blocking-feedback entry on a paused dispatch (returned from
/// [`list_blocked_by`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedByEntry {
    /// IRI of the blocking feedback artifact.
    pub feedback: NamedNode,
    /// Current lifecycle state if parseable, else `None` (a downstream
    /// SHACL audit will surface the malformed literal).
    pub state: Option<LifecycleState>,
}

/// Errors produced by [`pause_on_feedback`].
#[derive(Debug, Error)]
pub enum PauseError {
    /// The named group is not in `awaiting-action`; FT-032 Phase A
    /// restricts pausing to the pre-interpretation window.
    #[error(
        "cannot pause DispatchGroup <{group}> in state {state:?}; \
         pause is allowed only from awaiting-action (FT-032)"
    )]
    NotAwaitingAction {
        /// IRI of the offending group.
        group: String,
        /// State at the time of the pause call.
        state: DispatchStatus,
    },
    /// State machine refused the pause transition (only happens if
    /// callers bypass the [`Self::NotAwaitingAction`] guard above).
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    /// Underlying store / writer failure.
    #[error("pause commit failed: {0}")]
    Commit(String),
    /// Caller passed an empty blocking-feedback list. ADR-025 requires
    /// at least one IRI so the resume subscription has something to
    /// watch.
    #[error("pause_on_feedback called with empty blocking_feedback list")]
    NoBlockingFeedback,
}

/// Errors produced by [`resume_check`].
#[derive(Debug, Error)]
pub enum ResumeError {
    /// State machine rejected the resume transition.
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    /// Underlying store / writer failure.
    #[error("resume commit failed: {0}")]
    Commit(String),
    /// The named group does not exist in the store.
    #[error("DispatchGroup <{0}> not found")]
    GroupMissing(String),
}

/// Outcome of a [`resume_check`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeOutcome {
    /// At least one blocking feedback is still non-terminal; the group
    /// stays in `paused-for-feedback`. No mutation was committed.
    StillBlocked {
        /// Count of blocking-feedback rows still non-terminal.
        pending: usize,
    },
    /// Every blocking feedback is addressed (or closed); the group
    /// transitioned back to `awaiting-action` so a retry can fire.
    Resumed,
    /// At least one blocking feedback was rejected; the group transitioned
    /// to `feedback-rejected-action-blocked` (terminal).
    Blocked,
    /// The group was not in `paused-for-feedback` (idempotent no-op).
    NotPaused {
        /// Current status as observed.
        current: DispatchStatus,
    },
}

/// Pause the named DispatchGroup on blocking feedback emission.
///
/// Steps, mirroring FT-032 §Behaviour:
///
///   1. Reads the group's current state; refuses unless `awaiting-action`.
///   2. Drives the state machine through [`DispatchEvent::BlockingFeedbackEmitted`].
///   3. Persists the status transition AND one `dec:blockedBy` quad per
///      blocking feedback IRI in the same atomic [`StreamWriter::commit`].
///
/// Idempotent over `blocking_feedback`: re-pausing an already-paused
/// group with the same IRI set is a no-op write (insert duplicates are
/// suppressed by oxigraph's set semantics on quads, and the status
/// transition is `PausedForFeedback → PausedForFeedback` per the
/// lifecycle table).
pub fn pause_on_feedback(
    writer: &StreamWriter,
    store: &Store,
    group_iri: &NamedNode,
    blocking_feedback: &[NamedNode],
) -> Result<DispatchStatus, PauseError> {
    if blocking_feedback.is_empty() {
        return Err(PauseError::NoBlockingFeedback);
    }
    let mut group = DispatchGroup::read(store, group_iri)
        .map_err(|e| PauseError::Commit(format!("read group: {e:#}")))?
        .ok_or_else(|| PauseError::Commit(format!("no DispatchGroup at <{group_iri}>")))?;

    match group.status {
        DispatchStatus::AwaitingAction | DispatchStatus::PausedForFeedback => {}
        other => {
            return Err(PauseError::NotAwaitingAction {
                group: group_iri.as_str().to_string(),
                state: other,
            });
        }
    }

    // Status transition: only swap the literal if we are moving for the
    // first time. PausedForFeedback → PausedForFeedback is a no-op as far
    // as the persisted status literal is concerned, but the blocked-by
    // links still need to be appended.
    if group.status == DispatchStatus::AwaitingAction {
        group
            .transition(writer, DispatchEvent::BlockingFeedbackEmitted)
            .map_err(|e| match e {
                super::group::GroupError::Lifecycle(le) => PauseError::Lifecycle(le),
                super::group::GroupError::Commit(c) => PauseError::Commit(c),
            })?;
    }

    let mut inserts: Vec<oxigraph::model::Quad> = Vec::with_capacity(blocking_feedback.len());
    for fb in blocking_feedback {
        inserts.push(build_blocked_by_quad(group_iri, fb));
    }
    let mutation = Mutation {
        inserts,
        ..Mutation::default()
    }
    .with_cause(format!(
        "FT-032 pause_on_feedback {} (added {} dec:blockedBy)",
        group_iri.as_str(),
        blocking_feedback.len()
    ));
    writer
        .commit(mutation)
        .map_err(|e| PauseError::Commit(format!("{e:#}")))?;

    Ok(group.status)
}

/// Check whether a paused DispatchGroup can resume.
///
/// Walks the group's `dec:blockedBy` set and inspects each linked
/// feedback's `dec:lifecycleState`:
///
///   * Any non-terminal feedback → [`ResumeOutcome::StillBlocked`] (no-op).
///   * All terminal AND none rejected → transition to `awaiting-action`
///     so the orchestrator can re-dispatch the action ([`ResumeOutcome::Resumed`]).
///   * At least one rejected → transition to
///     `feedback-rejected-action-blocked` ([`ResumeOutcome::Blocked`], terminal).
///   * Group not paused → [`ResumeOutcome::NotPaused`] (no-op).
///
/// Idempotent — subscriptions retry naturally; calling resume_check on
/// an already-resumed (or already-blocked) group is a typed no-op.
pub fn resume_check(
    writer: &StreamWriter,
    store: &Store,
    group_iri: &NamedNode,
) -> Result<ResumeOutcome, ResumeError> {
    let mut group = DispatchGroup::read(store, group_iri)
        .map_err(|e| ResumeError::Commit(format!("read group: {e:#}")))?
        .ok_or_else(|| ResumeError::GroupMissing(group_iri.as_str().to_string()))?;

    if group.status != DispatchStatus::PausedForFeedback {
        return Ok(ResumeOutcome::NotPaused {
            current: group.status,
        });
    }

    let blocked_by = list_blocked_by(store, group_iri)
        .map_err(|e| ResumeError::Commit(format!("list dec:blockedBy: {e}")))?;
    if blocked_by.is_empty() {
        // No feedback in the list means we have nothing to gate on;
        // SHACL refuses paused groups without dec:blockedBy at write
        // time, so this branch is defensive only.
        return Ok(ResumeOutcome::StillBlocked { pending: 0 });
    }

    let mut pending = 0usize;
    let mut any_rejected = false;
    for entry in &blocked_by {
        match entry.state {
            Some(LifecycleState::Rejected) => any_rejected = true,
            Some(LifecycleState::Addressed) | Some(LifecycleState::Closed) => {}
            // Superseded ⇒ the feedback was rolled forward into another
            // emission. Treat as "no longer gating" so the dispatch can
            // resume on the successor's terminal state. Same shape as
            // addressed.
            Some(LifecycleState::Superseded) => {}
            Some(_) | None => pending += 1,
        }
    }

    if any_rejected {
        group
            .transition(writer, DispatchEvent::BlockingFeedbackRejected)
            .map_err(map_group_error)?;
        return Ok(ResumeOutcome::Blocked);
    }
    if pending > 0 {
        return Ok(ResumeOutcome::StillBlocked { pending });
    }
    group
        .transition(writer, DispatchEvent::BlockingFeedbackAddressed)
        .map_err(map_group_error)?;
    Ok(ResumeOutcome::Resumed)
}

fn map_group_error(e: super::group::GroupError) -> ResumeError {
    match e {
        super::group::GroupError::Lifecycle(le) => ResumeError::Lifecycle(le),
        super::group::GroupError::Commit(c) => ResumeError::Commit(c),
    }
}

/// List the `dec:blockedBy` feedback IRIs (and their current lifecycle
/// states) for `group_iri`. Returns an empty vec when no row is present.
pub fn list_blocked_by(
    store: &Store,
    group_iri: &NamedNode,
) -> Result<Vec<BlockedByEntry>, String> {
    let mut feedback_iris: Vec<NamedNode> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(group_iri.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if q.predicate.as_str() != IRI_DEC_BLOCKED_BY {
            continue;
        }
        if let Term::NamedNode(n) = q.object {
            let key = n.as_str().to_string();
            if seen.insert(key) {
                feedback_iris.push(n);
            }
        }
    }
    let mut out: Vec<BlockedByEntry> = Vec::with_capacity(feedback_iris.len());
    for fb in feedback_iris {
        let state = read_lifecycle_state(store, &fb);
        out.push(BlockedByEntry {
            feedback: fb,
            state,
        });
    }
    Ok(out)
}

fn read_lifecycle_state(store: &Store, feedback_iri: &NamedNode) -> Option<LifecycleState> {
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(feedback_iri.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if q.predicate.as_str() != IRI_DEC_LIFECYCLE_STATE {
            continue;
        }
        if let Term::Literal(lit) = q.object {
            return LifecycleState::parse(lit.value());
        }
    }
    None
}

/// Find every `dec:DispatchGroup` whose `dec:blockedBy` includes
/// `feedback_iri`. Used by the resume subscription's delivery handler:
/// when a feedback artifact transitions to a terminal state, look up
/// every paused group it gates and call [`resume_check`] on each.
pub fn list_paused_groups_for_feedback(
    store: &Store,
    feedback_iri: &NamedNode,
) -> Result<Vec<NamedNode>, String> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?group WHERE {{ \
           {{ ?group a dec:DispatchGroup ; \
                    dec:dispatchStatus \"paused-for-feedback\" ; \
                    dec:blockedBy <{fb}> . }} \
           UNION \
           {{ GRAPH ?g {{ ?group a dec:DispatchGroup ; \
                                 dec:dispatchStatus \"paused-for-feedback\" ; \
                                 dec:blockedBy <{fb}> . }} }} \
         }}",
        fb = feedback_iri.as_str()
    );
    let solutions = match store
        .query(q.as_str())
        .map_err(|e| format!("running resume-check query: {e}"))?
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

// Tests live in `pause_tests.rs` so this file stays under ADR-013
// Rule 1's 400-line hard cap. The compile-only path attribute keeps
// them in the same compilation unit without forcing a re-export.
#[cfg(test)]
#[path = "pause_tests.rs"]
mod tests;
