//! DDD vocabulary IRIs for decision-cli's orchestration graph.
//!
//! These are the application-level identifiers `oxi-events` is forbidden
//! from naming (ADR-001). Vocabulary IRIs are intentionally undocumented
//! individually — names speak for themselves.

#![allow(missing_docs)]

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
    IRI_DEC_GOAL,
    IRI_DEC_DISPATCH,
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
