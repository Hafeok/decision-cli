//! FT-057 / ADR-034 / ADR-037 — `dec:SessionRecord` escalation-telemetry vocabulary.
//!
//! Split out of `core::vocab` (mod.rs) per the ADR-013 file-length policy.
//! Re-exported from the parent module so external callers continue to
//! import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

// --- Escalation edge predicates ---------------------------------------------

/// `dec:escalated_from` predicate — Session → prior Session in chain.
pub const IRI_DEC_ESCALATED_FROM: &str = "https://decision-cli.dev/ns#escalated_from";

/// `dec:escalated_to` predicate — Session → next Session in chain.
pub const IRI_DEC_ESCALATED_TO: &str = "https://decision-cli.dev/ns#escalated_to";

/// `dec:escalation_reason` predicate — Session → trigger signal literal.
pub const IRI_DEC_ESCALATION_REASON: &str = "https://decision-cli.dev/ns#escalation_reason";

// --- Token-breakdown predicates ---------------------------------------------

/// `dec:input_tokens_base` predicate — Session → integer base input tokens.
pub const IRI_DEC_INPUT_TOKENS_BASE: &str = "https://decision-cli.dev/ns#input_tokens_base";

/// `dec:input_tokens_cache_write` predicate — Session → integer cache-write input tokens.
pub const IRI_DEC_INPUT_TOKENS_CACHE_WRITE: &str =
    "https://decision-cli.dev/ns#input_tokens_cache_write";

/// `dec:input_tokens_cache_hit` predicate — Session → integer cache-hit input tokens.
pub const IRI_DEC_INPUT_TOKENS_CACHE_HIT: &str =
    "https://decision-cli.dev/ns#input_tokens_cache_hit";

/// `dec:output_tokens` predicate — Session → integer output tokens.
pub const IRI_DEC_OUTPUT_TOKENS: &str = "https://decision-cli.dev/ns#output_tokens";

// --- Capability link --------------------------------------------------------

/// `dec:capability` predicate — Session → resolving Capability IRI.
pub const IRI_DEC_SESSION_CAPABILITY: &str = "https://decision-cli.dev/ns#capability";

// --- NamedNodeRef accessors -------------------------------------------------

#[must_use]
pub fn escalated_from_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATED_FROM)
}

#[must_use]
pub fn escalated_to_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATED_TO)
}

#[must_use]
pub fn escalation_reason_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ESCALATION_REASON)
}

#[must_use]
pub fn input_tokens_base_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_INPUT_TOKENS_BASE)
}

#[must_use]
pub fn input_tokens_cache_write_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_INPUT_TOKENS_CACHE_WRITE)
}

#[must_use]
pub fn input_tokens_cache_hit_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_INPUT_TOKENS_CACHE_HIT)
}

#[must_use]
pub fn output_tokens_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_OUTPUT_TOKENS)
}

#[must_use]
pub fn session_capability_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SESSION_CAPABILITY)
}
