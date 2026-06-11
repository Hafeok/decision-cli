//! DDD vocabulary IRIs for decision-cli's orchestration graph.
//!
//! These are the application-level identifiers `oxi-events` is forbidden
//! from naming (ADR-001). Vocabulary IRIs are intentionally undocumented
//! individually — names speak for themselves.

#![allow(missing_docs)]

mod auto_dispatch;
mod capability;
mod catalog;
mod conformance_audit;
mod feedback;
mod mechanical_provenance;
mod role_binding;
mod runner_dispatch;
mod session_record;
mod signature_verdict;
mod verify_bench;
mod verify_graph;
mod verify_result;
mod waiver;
mod worker_image;
mod worker_image_submission;

pub use auto_dispatch::*;
pub use capability::*;
pub use catalog::*;
pub use conformance_audit::*;
pub use feedback::*;
pub use mechanical_provenance::*;
pub use role_binding::*;
pub use runner_dispatch::*;
pub use session_record::*;
pub use signature_verdict::*;
pub use verify_bench::*;
pub use verify_graph::*;
pub use verify_result::*;
pub use waiver::*;
pub use worker_image::*;
pub use worker_image_submission::*;

use oxrdf::NamedNodeRef;

pub const NS_DEC: &str = "https://decision-cli.dev/ns#";

pub const IRI_DEC_VALUE_STREAM: &str = "https://decision-cli.dev/ns#ValueStream";
pub const IRI_DEC_VALUE_ACTION: &str = "https://decision-cli.dev/ns#ValueAction";
pub const IRI_DEC_GOAL: &str = "https://decision-cli.dev/ns#Goal";
/// IRI of the `dec:Session` class.
pub const IRI_DEC_SESSION: &str = "https://decision-cli.dev/ns#Session";
pub const IRI_DEC_DISPATCH: &str = "https://decision-cli.dev/ns#Dispatch";
pub const IRI_DEC_EVENT: &str = "https://decision-cli.dev/ns#Event";

pub const IRI_DEC_IN_STREAM: &str = "https://decision-cli.dev/ns#inStream";
pub const IRI_DEC_GRAPH_ORCHESTRATION: &str = "https://decision-cli.dev/ns/orchestration";

// --- FT-021 / ADR-017: DispatchGroup vocabulary ------------------------------

/// Class IRI for `dec:DispatchGroup` (ADR-017).
pub const IRI_DEC_DISPATCH_GROUP: &str = "https://decision-cli.dev/ns#DispatchGroup";

/// Class IRI for `dec:ActionSession` (ADR-017).
pub const IRI_DEC_ACTION_SESSION: &str = "https://decision-cli.dev/ns#ActionSession";

/// Class IRI for `dec:InterpretationSession` (ADR-017).
pub const IRI_DEC_INTERPRETATION_SESSION: &str =
    "https://decision-cli.dev/ns#InterpretationSession";

/// `dec:hasActionSession` predicate — DispatchGroup → ActionSession.
pub const IRI_DEC_HAS_ACTION_SESSION: &str = "https://decision-cli.dev/ns#hasActionSession";

/// `dec:hasInterpretationSession` predicate — DispatchGroup → InterpretationSession.
pub const IRI_DEC_HAS_INTERPRETATION_SESSION: &str =
    "https://decision-cli.dev/ns#hasInterpretationSession";

/// `dec:dispatchedFor` predicate — DispatchGroup → feature_spec string.
pub const IRI_DEC_DISPATCHED_FOR: &str = "https://decision-cli.dev/ns#dispatchedFor";

/// `dec:dispatchStatus` predicate — DispatchGroup → status literal.
pub const IRI_DEC_DISPATCH_STATUS: &str = "https://decision-cli.dev/ns#dispatchStatus";

/// `prov:wasInformedBy` predicate — Activity → Activity.
pub const IRI_PROV_WAS_INFORMED_BY: &str = "http://www.w3.org/ns/prov#wasInformedBy";

/// DispatchGroup lifecycle states per FT-021 §Outputs.
pub const DISPATCH_STATUS_AWAITING_ACTION: &str = "awaiting-action";
pub const DISPATCH_STATUS_AWAITING_INTERPRETATION: &str = "awaiting-interpretation";
pub const DISPATCH_STATUS_INTERPRETATION_RUNNING: &str = "interpretation-running";
pub const DISPATCH_STATUS_INTERPRETATION_REJECTED: &str = "interpretation-rejected";
pub const DISPATCH_STATUS_AWAITING_AMENDMENT: &str = "awaiting-amendment";
pub const DISPATCH_STATUS_ACTION_FAILED: &str = "action-failed";
pub const DISPATCH_STATUS_INTERPRETATION_FAILED: &str = "interpretation-failed";
pub const DISPATCH_STATUS_COMPLETE: &str = "complete";

/// FT-032 / ADR-025: DispatchGroup paused while blocking feedback is open.
pub const DISPATCH_STATUS_PAUSED_FOR_FEEDBACK: &str = "paused-for-feedback";

/// FT-032 / ADR-025: terminal status when a blocking feedback is rejected.
pub const DISPATCH_STATUS_FEEDBACK_REJECTED_ACTION_BLOCKED: &str =
    "feedback-rejected-action-blocked";

/// FT-032 / ADR-025: `dec:blockedBy` predicate — DispatchGroup → blocking Feedback IRIs.
pub const IRI_DEC_BLOCKED_BY: &str = "https://decision-cli.dev/ns#blockedBy";

#[must_use]
pub fn blocked_by() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BLOCKED_BY)
}

#[must_use]
pub fn dispatch_group_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH_GROUP)
}

#[must_use]
pub fn action_session_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ACTION_SESSION)
}

#[must_use]
pub fn interpretation_session_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_INTERPRETATION_SESSION)
}

#[must_use]
pub fn has_action_session() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_HAS_ACTION_SESSION)
}

#[must_use]
pub fn has_interpretation_session() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_HAS_INTERPRETATION_SESSION)
}

#[must_use]
pub fn dispatched_for() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPATCHED_FOR)
}

#[must_use]
pub fn dispatch_status() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH_STATUS)
}

#[must_use]
pub fn was_informed_by() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_WAS_INFORMED_BY)
}

// --- FT-020 / ADR-018: VerificationVerdict vocabulary -------------------------

/// Class IRI for `dec:VerificationVerdict` (ADR-018).
pub const IRI_DEC_VERIFICATION_VERDICT: &str = "https://decision-cli.dev/ns#VerificationVerdict";

/// `dec:verdict` predicate — one of `approved`, `rejected`, `amendment-required`.
pub const IRI_DEC_VERDICT: &str = "https://decision-cli.dev/ns#verdict";

/// `dec:rationale` predicate — free-form prose, SHACL `sh:minLength 20`.
pub const IRI_DEC_RATIONALE: &str = "https://decision-cli.dev/ns#rationale";

/// `dec:violates` predicate — references to TCs or ADRs that were violated.
pub const IRI_DEC_VIOLATES: &str = "https://decision-cli.dev/ns#violates";

/// `dec:amendmentGuidance` predicate — actionable guidance for amendment-required.
pub const IRI_DEC_AMENDMENT_GUIDANCE: &str = "https://decision-cli.dev/ns#amendmentGuidance";

/// Verdict literal values (per ADR-018 §SHACL shape, `sh:in`).
pub const VERDICT_APPROVED: &str = "approved";
pub const VERDICT_REJECTED: &str = "rejected";
pub const VERDICT_AMENDMENT_REQUIRED: &str = "amendment-required";

// --- FT-022 / ADR-017: VerifierDispatchEvent vocabulary -----------------------

/// Class IRI for `dec:VerifierDispatchEvent` (FT-022 §Outputs).
pub const IRI_DEC_VERIFIER_DISPATCH_EVENT: &str =
    "https://decision-cli.dev/ns#VerifierDispatchEvent";

/// `dec:eventClass` predicate — short tag for the event payload class.
pub const IRI_DEC_EVENT_CLASS: &str = "https://decision-cli.dev/ns#eventClass";

/// `dec:targetRole` predicate — `dec:roleId` the event is routed to.
pub const IRI_DEC_TARGET_ROLE: &str = "https://decision-cli.dev/ns#targetRole";

/// `dec:dispatchGroup` predicate — link back to the originating DispatchGroup.
pub const IRI_DEC_DISPATCH_GROUP_REF: &str = "https://decision-cli.dev/ns#dispatchGroup";

/// `dec:bundleSeed` predicate — IRI of the action artifact/session seeding the
/// downstream bundle (consumed by the verifier worker harness in FT-023).
pub const IRI_DEC_BUNDLE_SEED: &str = "https://decision-cli.dev/ns#bundleSeed";

/// `dec:emittedAt` predicate — RFC3339 timestamp of event emission.
pub const IRI_DEC_EMITTED_AT: &str = "https://decision-cli.dev/ns#emittedAt";

/// Stable `dec:eventClass` literal for verifier-dispatch events.
pub const EVENT_CLASS_VERIFIER_DISPATCH: &str = "verifier-dispatch";

#[must_use]
pub fn verifier_dispatch_event_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFIER_DISPATCH_EVENT)
}

#[must_use]
pub fn event_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EVENT_CLASS)
}

#[must_use]
pub fn target_role() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TARGET_ROLE)
}

#[must_use]
pub fn dispatch_group_ref() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH_GROUP_REF)
}

#[must_use]
pub fn bundle_seed() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BUNDLE_SEED)
}

#[must_use]
pub fn emitted_at() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EMITTED_AT)
}

#[must_use]
pub fn verification_verdict_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_VERDICT)
}

#[must_use]
pub fn verdict() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERDICT)
}

#[must_use]
pub fn rationale() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RATIONALE)
}

#[must_use]
pub fn violates() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VIOLATES)
}

#[must_use]
pub fn amendment_guidance() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AMENDMENT_GUIDANCE)
}

/// Class IRIs whose instances must carry a `dec:inStream` link to the
/// active `dec:ValueStream` (TC-014, ADR-005).
pub const SCOPED_CLASSES: &[&str] = &[
    IRI_DEC_SESSION,
    IRI_DEC_ACTION_SESSION,
    IRI_DEC_INTERPRETATION_SESSION,
    IRI_DEC_GOAL,
    IRI_DEC_DISPATCH,
    IRI_DEC_DISPATCH_GROUP,
    IRI_DEC_EVENT,
];

#[must_use]
pub fn in_stream() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_IN_STREAM)
}

#[must_use]
pub fn value_stream_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VALUE_STREAM)
}

#[must_use]
pub fn orchestration_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION)
}

// --- FT-056 / ADR-035: Bundle stakes vocabulary -----------------------------

/// Class IRI for `dec:Bundle` (FT-056 / ADR-035).
pub const IRI_DEC_BUNDLE: &str = "https://decision-cli.dev/ns#Bundle";

/// Named graph the bundle catalog lives in.
pub const IRI_DEC_GRAPH_BUNDLE: &str = "https://decision-cli.dev/ns/bundles";

/// Canonical IRI prefix for `dec:Bundle` artifacts: `…/ns/bundle/<hash>`.
pub const IRI_DEC_BUNDLE_PREFIX: &str = "https://decision-cli.dev/ns/bundle/";

/// `dec:stakes` predicate — Bundle → enum literal (ADR-035).
pub const IRI_DEC_STAKES: &str = "https://decision-cli.dev/ns#stakes";

/// `dec:focal` predicate — Bundle → focal artifact IRI.
pub const IRI_DEC_FOCAL: &str = "https://decision-cli.dev/ns#focal";

/// Stakes literal values per ADR-035 §"Who sets it".
pub const STAKES_ROUTINE: &str = "routine";
pub const STAKES_ELEVATED: &str = "elevated";
pub const STAKES_FOUNDATIONAL: &str = "foundational";

#[must_use]
pub fn bundle_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BUNDLE)
}

#[must_use]
pub fn bundle_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_BUNDLE)
}

#[must_use]
pub fn stakes_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STAKES)
}

#[must_use]
pub fn focal_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_FOCAL)
}
