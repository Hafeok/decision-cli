//! Lifecycle-specific SHACL companion-field checks (FT-027 / ADR-024).
//!
//! Split out from `shacl.rs` so that the structural-cardinality shape
//! (FT-026) and the lifecycle-companion shape (FT-027) live in their
//! own files. Both layers run together inside `validate_quads`.

use oxrdf::{NamedNode, Quad};

use crate::vocab::{
    IRI_DEC_ADDRESSING_ARTIFACT, IRI_DEC_CLOSED_BY, IRI_DEC_LIFECYCLE_STATE,
    IRI_DEC_RECEIVING_SESSION, IRI_DEC_REJECTION_REASON, IRI_DEC_ROUTED_AT, IRI_DEC_SUPERSEDED_BY,
    IRI_DEC_TARGET_ROLE,
};

use super::lifecycle::LifecycleState;
use super::shacl::{iri_values, literal_values, violation, FeedbackViolation};

/// `sh:in` on `dec:lifecycleState` — the literal must be one of the
/// seven values defined in ADR-024 (FT-027).
pub(super) fn check_lifecycle_state_in(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    for raw in literal_values(quads, subject, IRI_DEC_LIFECYCLE_STATE) {
        if LifecycleState::parse(&raw).is_none() {
            violations.push(violation(
                subject,
                IRI_DEC_LIFECYCLE_STATE,
                &format!(
                    "dec:lifecycleState must be one of {{produced, routed, received, addressed, closed, rejected, superseded}}, got {raw:?}"
                ),
            ));
        }
    }
}

/// Per-state companion fields per ADR-024 §SHACL fragment:
///   `routed`    → `dec:routedAt` AND `dec:targetRole`
///   `received`  → `dec:receivingSession`
///   `addressed` → `dec:addressingArtifact`
///   `closed`    → `dec:closedBy` AND `dec:addressingArtifact`
///   `rejected`  → `dec:rejectionReason`
///   `superseded`→ `dec:supersededBy`
pub(super) fn check_lifecycle_companion_fields(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    let states: Vec<String> = literal_values(quads, subject, IRI_DEC_LIFECYCLE_STATE);
    for raw in &states {
        let Some(state) = LifecycleState::parse(raw) else {
            continue;
        };
        check_state(state, quads, subject, violations);
    }
}

/// A companion-field rule attached to a target state.
///
/// `is_iri` distinguishes IRI-valued (`dec:addressingArtifact`) from
/// literal-valued (`dec:routedAt`) predicates so the validator picks
/// the right collector function.
struct CompanionRule {
    predicate: &'static str,
    detail: &'static str,
    is_iri: bool,
}

const ROUTED_RULES: &[CompanionRule] = &[
    CompanionRule {
        predicate: IRI_DEC_ROUTED_AT,
        detail: "routed state requires dec:routedAt",
        is_iri: false,
    },
    CompanionRule {
        predicate: IRI_DEC_TARGET_ROLE,
        detail: "routed state requires dec:targetRole",
        is_iri: false,
    },
];

const RECEIVED_RULES: &[CompanionRule] = &[CompanionRule {
    predicate: IRI_DEC_RECEIVING_SESSION,
    detail: "received state requires dec:receivingSession",
    is_iri: true,
}];

const ADDRESSED_RULES: &[CompanionRule] = &[CompanionRule {
    predicate: IRI_DEC_ADDRESSING_ARTIFACT,
    detail: "addressed state requires dec:addressingArtifact",
    is_iri: true,
}];

const CLOSED_RULES: &[CompanionRule] = &[
    CompanionRule {
        predicate: IRI_DEC_CLOSED_BY,
        detail: "closed state requires dec:closedBy",
        is_iri: true,
    },
    CompanionRule {
        predicate: IRI_DEC_ADDRESSING_ARTIFACT,
        detail:
            "closed state requires dec:addressingArtifact (PROV-O link to the addressing artifact)",
        is_iri: true,
    },
];

const REJECTED_RULES: &[CompanionRule] = &[CompanionRule {
    predicate: IRI_DEC_REJECTION_REASON,
    detail: "rejected state requires dec:rejectionReason",
    is_iri: false,
}];

const SUPERSEDED_RULES: &[CompanionRule] = &[CompanionRule {
    predicate: IRI_DEC_SUPERSEDED_BY,
    detail: "superseded state requires dec:supersededBy",
    is_iri: true,
}];

fn rules_for(state: LifecycleState) -> &'static [CompanionRule] {
    match state {
        LifecycleState::Routed => ROUTED_RULES,
        LifecycleState::Received => RECEIVED_RULES,
        LifecycleState::Addressed => ADDRESSED_RULES,
        LifecycleState::Closed => CLOSED_RULES,
        LifecycleState::Rejected => REJECTED_RULES,
        LifecycleState::Superseded => SUPERSEDED_RULES,
        LifecycleState::Produced => &[],
    }
}

fn check_state(
    state: LifecycleState,
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<FeedbackViolation>,
) {
    for rule in rules_for(state) {
        if rule.is_iri {
            require_iri(quads, subject, rule.predicate, rule.detail, violations);
        } else {
            require_literal(quads, subject, rule.predicate, rule.detail, violations);
        }
    }
}

fn require_literal(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    detail: &str,
    violations: &mut Vec<FeedbackViolation>,
) {
    if literal_values(quads, subject, predicate).is_empty() {
        violations.push(violation(subject, predicate, detail));
    }
}

fn require_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    detail: &str,
    violations: &mut Vec<FeedbackViolation>,
) {
    if iri_values(quads, subject, predicate).is_empty() {
        violations.push(violation(subject, predicate, detail));
    }
}
