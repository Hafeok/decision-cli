//! FT-055 / ADR-033 / ADR-034 — `dec:RoleBinding`, `dec:EscalationStep`,
//! `dec:EscalationTrigger` vocabulary.
//!
//! Split out of `core::vocab` (mod.rs) per the ADR-013 file-length policy.
//! Re-exported from the parent module so external callers continue to
//! import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

// --- Class IRIs --------------------------------------------------------------

/// Class IRI for `dec:RoleBinding` (FT-055 / ADR-033).
pub const IRI_DEC_ROLE_BINDING: &str = "https://decision-cli.dev/ns#RoleBinding";

/// Class IRI for `dec:EscalationStep` (FT-055 / ADR-034).
pub const IRI_DEC_ESCALATION_STEP: &str = "https://decision-cli.dev/ns#EscalationStep";

/// Class IRI for `dec:EscalationTrigger` (FT-055 / ADR-034).
pub const IRI_DEC_ESCALATION_TRIGGER: &str = "https://decision-cli.dev/ns#EscalationTrigger";

// --- Predicate IRIs ----------------------------------------------------------

/// `dec:role_id` predicate — references a role's id in the FT-030 role
/// catalog. Not the same as `dec:roleId` on `dec:Role`; this carries an
/// xsd:string lookup key rather than a node link to avoid coupling.
pub const IRI_DEC_ROLE_BINDING_ROLE_ID: &str = "https://decision-cli.dev/ns#role_id";

/// `dec:default_capability` predicate — RoleBinding → Capability.
pub const IRI_DEC_DEFAULT_CAPABILITY: &str =
    "https://decision-cli.dev/ns#default_capability";

/// `dec:escalation_steps` predicate — RoleBinding → rdf:List of EscalationStep.
pub const IRI_DEC_ESCALATION_STEPS: &str =
    "https://decision-cli.dev/ns#escalation_steps";

/// `dec:active` predicate — boolean, exactly one per RoleBinding.
pub const IRI_DEC_ROLE_BINDING_ACTIVE: &str = "https://decision-cli.dev/ns#active";

/// `dec:step_capability` predicate — EscalationStep → Capability.
pub const IRI_DEC_STEP_CAPABILITY: &str = "https://decision-cli.dev/ns#step_capability";

/// `dec:triggers` predicate — EscalationStep → EscalationTrigger (multi-valued).
pub const IRI_DEC_TRIGGERS: &str = "https://decision-cli.dev/ns#triggers";

/// `dec:trigger_signal` predicate — EscalationTrigger → controlled
/// vocabulary literal (ADR-034).
pub const IRI_DEC_TRIGGER_SIGNAL: &str = "https://decision-cli.dev/ns#trigger_signal";

// --- IRI prefixes for minted role-binding artifacts --------------------------

/// IRI prefix for minted role-binding IRIs:
/// `https://decision-cli.dev/ns/binding/<role_id>/v<version>`.
pub const IRI_DEC_ROLE_BINDING_PREFIX: &str = "https://decision-cli.dev/ns/binding/";

/// Named graph holding the role-binding catalog projections.
pub const IRI_DEC_GRAPH_ROLE_BINDING: &str = "https://decision-cli.dev/ns/graph/role-binding";

// --- Trigger signal literals (ADR-034 closed vocabulary) ---------------------

pub const TRIGGER_STAKES_ROUTINE: &str = "stakes_routine";
pub const TRIGGER_STAKES_ELEVATED: &str = "stakes_elevated";
pub const TRIGGER_STAKES_FOUNDATIONAL: &str = "stakes_foundational";

pub const TRIGGER_CONFIDENCE_BELOW_05: &str = "confidence_below_0.5";
pub const TRIGGER_CONFIDENCE_BELOW_07: &str = "confidence_below_0.7";
pub const TRIGGER_CONFIDENCE_BELOW_09: &str = "confidence_below_0.9";

pub const TRIGGER_AUDIT_PASS: &str = "audit_pass";
pub const TRIGGER_AUDIT_FAIL: &str = "audit_fail";

pub const TRIGGER_PRIOR_ATTEMPTS_GE_1: &str = "prior_attempts_ge_1";
pub const TRIGGER_PRIOR_ATTEMPTS_GE_2: &str = "prior_attempts_ge_2";
pub const TRIGGER_PRIOR_ATTEMPTS_GE_3: &str = "prior_attempts_ge_3";
pub const TRIGGER_PRIOR_ATTEMPTS_GE_4: &str = "prior_attempts_ge_4";
pub const TRIGGER_PRIOR_ATTEMPTS_GE_5: &str = "prior_attempts_ge_5";

pub const TRIGGER_FEEDBACK_CONTRADICTION: &str = "feedback_contradiction";
pub const TRIGGER_FEEDBACK_UNIMPLEMENTABLE_CRITICAL: &str =
    "feedback_unimplementable_critical";
pub const TRIGGER_FEEDBACK_GAP: &str = "feedback_gap";
pub const TRIGGER_FEEDBACK_SCOPE_ISSUE: &str = "feedback_scope_issue";

/// Closed vocabulary for `dec:trigger_signal` (ADR-034).
pub const TRIGGER_SIGNAL_VOCABULARY: &[&str] = &[
    TRIGGER_STAKES_ROUTINE,
    TRIGGER_STAKES_ELEVATED,
    TRIGGER_STAKES_FOUNDATIONAL,
    TRIGGER_CONFIDENCE_BELOW_05,
    TRIGGER_CONFIDENCE_BELOW_07,
    TRIGGER_CONFIDENCE_BELOW_09,
    TRIGGER_AUDIT_PASS,
    TRIGGER_AUDIT_FAIL,
    TRIGGER_PRIOR_ATTEMPTS_GE_1,
    TRIGGER_PRIOR_ATTEMPTS_GE_2,
    TRIGGER_PRIOR_ATTEMPTS_GE_3,
    TRIGGER_PRIOR_ATTEMPTS_GE_4,
    TRIGGER_PRIOR_ATTEMPTS_GE_5,
    TRIGGER_FEEDBACK_CONTRADICTION,
    TRIGGER_FEEDBACK_UNIMPLEMENTABLE_CRITICAL,
    TRIGGER_FEEDBACK_GAP,
    TRIGGER_FEEDBACK_SCOPE_ISSUE,
];

// --- NamedNodeRef accessors --------------------------------------------------

#[must_use]
pub fn role_binding_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROLE_BINDING)
}

#[must_use]
pub fn escalation_step_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATION_STEP)
}

#[must_use]
pub fn escalation_trigger_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATION_TRIGGER)
}

#[must_use]
pub fn role_binding_role_id_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROLE_BINDING_ROLE_ID)
}

#[must_use]
pub fn default_capability_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_DEFAULT_CAPABILITY)
}

#[must_use]
pub fn escalation_steps_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATION_STEPS)
}

#[must_use]
pub fn role_binding_active_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ROLE_BINDING_ACTIVE)
}

#[must_use]
pub fn step_capability_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STEP_CAPABILITY)
}

#[must_use]
pub fn triggers_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TRIGGERS)
}

#[must_use]
pub fn trigger_signal_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TRIGGER_SIGNAL)
}

#[must_use]
pub fn role_binding_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_ROLE_BINDING)
}
