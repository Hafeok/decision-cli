//! FT-026 / ADR-022 — `dec:Feedback` vocabulary.
//!
//! Split out of `core::vocab` to keep per-file size within the ADR-013
//! 400-line ceiling. Re-exported from the parent module so callers
//! continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

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
pub const IRI_DEC_ROUTING_OVERRIDE_ACTOR: &str = "https://decision-cli.dev/ns#routingOverrideActor";

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
