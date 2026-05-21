//! DDD vocabulary IRIs for decision-cli's orchestration graph.
//!
//! These are the application-level identifiers `oxi-events` is forbidden
//! from naming (ADR-001). Vocabulary IRIs are intentionally undocumented
//! individually — names speak for themselves.

#![allow(missing_docs)]

mod verify_env;

pub use verify_env::*;

use oxigraph::model::NamedNodeRef;

pub const NS_DEC: &str = "https://decision-cli.dev/ns#";

pub const IRI_DEC_VALUE_STREAM: &str = "https://decision-cli.dev/ns#ValueStream";
pub const IRI_DEC_VALUE_ACTION: &str = "https://decision-cli.dev/ns#ValueAction";
pub const IRI_DEC_GOAL: &str = "https://decision-cli.dev/ns#Goal";
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

// --- FT-026 / ADR-022: Feedback vocabulary ------------------------------------

/// Class IRI for `dec:Feedback` (ADR-022).
pub const IRI_DEC_FEEDBACK: &str = "https://decision-cli.dev/ns#Feedback";

/// `dec:feedbackClass` predicate — controlled class tag (ADR-023; FT-028).
pub const IRI_DEC_FEEDBACK_CLASS: &str = "https://decision-cli.dev/ns#feedbackClass";

/// `dec:severity` predicate — severity hint literal.
pub const IRI_DEC_SEVERITY: &str = "https://decision-cli.dev/ns#severity";

/// `dec:evidence` predicate — citation back into the originating bundle/artifact.
pub const IRI_DEC_EVIDENCE: &str = "https://decision-cli.dev/ns#evidence";

/// `dec:recommendation` predicate — optional suggested fix.
pub const IRI_DEC_RECOMMENDATION: &str = "https://decision-cli.dev/ns#recommendation";

/// `dec:lifecycleState` predicate — lifecycle state literal (ADR-024; FT-027).
pub const IRI_DEC_LIFECYCLE_STATE: &str = "https://decision-cli.dev/ns#lifecycleState";

/// `dec:sourceSession` predicate — Feedback → Session that emitted it.
pub const IRI_DEC_SOURCE_SESSION: &str = "https://decision-cli.dev/ns#sourceSession";

/// `dec:sourceArtifact` predicate — Feedback → the bundled artifact it is about.
pub const IRI_DEC_SOURCE_ARTIFACT: &str = "https://decision-cli.dev/ns#sourceArtifact";

/// `dec:addressingArtifact` predicate — Feedback → resolving artifact.
pub const IRI_DEC_ADDRESSING_ARTIFACT: &str = "https://decision-cli.dev/ns#addressingArtifact";

/// `dec:closedBy` predicate — actor (session/human) that closed the loop.
pub const IRI_DEC_CLOSED_BY: &str = "https://decision-cli.dev/ns#closedBy";

/// `dec:rejectionReason` predicate — rationale when feedback is rejected.
pub const IRI_DEC_REJECTION_REASON: &str = "https://decision-cli.dev/ns#rejectionReason";

/// `dec:supersededBy` predicate — newer feedback that subsumes this one.
pub const IRI_DEC_SUPERSEDED_BY: &str = "https://decision-cli.dev/ns#supersededBy";

/// `dec:routedAt` predicate — RFC3339 timestamp of routing transition.
pub const IRI_DEC_ROUTED_AT: &str = "https://decision-cli.dev/ns#routedAt";

/// `dec:receivingSession` predicate — session that picked up the routed feedback.
pub const IRI_DEC_RECEIVING_SESSION: &str = "https://decision-cli.dev/ns#receivingSession";

/// `dec:dispositionOverride` predicate — per-emission blocking override (ADR-025).
pub const IRI_DEC_DISPOSITION_OVERRIDE: &str = "https://decision-cli.dev/ns#dispositionOverride";

/// `dec:dispositionRationale` predicate — operator rationale for an override.
pub const IRI_DEC_DISPOSITION_RATIONALE: &str = "https://decision-cli.dev/ns#dispositionRationale";

/// `dec:routingOverride` predicate — manual target-role override (FT-033 / ADR-026).
pub const IRI_DEC_ROUTING_OVERRIDE: &str = "https://decision-cli.dev/ns#routingOverride";
/// `dec:routingOverrideActor` predicate — operator identity for an override (FT-033).
pub const IRI_DEC_ROUTING_OVERRIDE_ACTOR: &str =
    "https://decision-cli.dev/ns#routingOverrideActor";

#[must_use]
pub fn feedback_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_FEEDBACK)
}

#[must_use]
pub fn feedback_class_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_FEEDBACK_CLASS)
}

#[must_use]
pub fn severity() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SEVERITY)
}

#[must_use]
pub fn evidence() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EVIDENCE)
}

#[must_use]
pub fn recommendation() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RECOMMENDATION)
}

#[must_use]
pub fn lifecycle_state() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LIFECYCLE_STATE)
}

#[must_use]
pub fn source_session() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SOURCE_SESSION)
}

#[must_use]
pub fn source_artifact() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SOURCE_ARTIFACT)
}

#[must_use]
pub fn addressing_artifact() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ADDRESSING_ARTIFACT)
}

#[must_use]
pub fn closed_by() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CLOSED_BY)
}

#[must_use]
pub fn rejection_reason() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_REJECTION_REASON)
}

#[must_use]
pub fn superseded_by() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SUPERSEDED_BY)
}

#[must_use]
pub fn routed_at() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROUTED_AT)
}

#[must_use]
pub fn receiving_session() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RECEIVING_SESSION)
}

#[must_use]
pub fn disposition_override() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPOSITION_OVERRIDE)
}

#[must_use]
pub fn disposition_rationale() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DISPOSITION_RATIONALE)
}

#[must_use]
pub fn routing_override() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROUTING_OVERRIDE)
}

#[must_use]
pub fn routing_override_actor() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROUTING_OVERRIDE_ACTOR)
}

/// Default lifecycle state for a freshly-emitted feedback (ADR-024).
/// FT-027 will own the full state machine; FT-026 only sees the seed.
pub const FEEDBACK_STATE_PRODUCED: &str = "produced";
/// Lifecycle states that count as still-open (i.e. not terminal).
pub const FEEDBACK_TERMINAL_STATES: &[&str] = &["closed", "rejected", "superseded"];

