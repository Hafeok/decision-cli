//! FT-054 / ADR-033 — `dec:Capability` vocabulary.
//!
//! Split out of `core::vocab` (mod.rs) to keep the per-file size within
//! the ADR-013 400-line ceiling. Re-exported from the parent module so
//! external callers continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

/// Class IRI for `dec:Capability` (FT-054 / ADR-033).
pub const IRI_DEC_CAPABILITY: &str = "https://decision-cli.dev/ns#Capability";

/// `dec:capability_id` predicate — stable tag for role lookup.
pub const IRI_DEC_CAPABILITY_ID: &str = "https://decision-cli.dev/ns#capability_id";
// NB: `dec:endpoint` predicate IRI is shared with verify_bench's
// `IRI_DEC_ENDPOINT` (the same predicate name with different semantics
// per domain class). Capability-side helpers below alias the same IRI
// string to keep the read sites scoped to capability vocabulary.
/// `dec:endpoint` predicate — `scaleway` | `anthropic`.
pub const IRI_DEC_CAPABILITY_ENDPOINT: &str = "https://decision-cli.dev/ns#endpoint";
/// `dec:model_identifier` predicate — exact provider model string.
pub const IRI_DEC_MODEL_IDENTIFIER: &str = "https://decision-cli.dev/ns#model_identifier";
/// `dec:tier` predicate — escalation-ladder tier (0–3).
pub const IRI_DEC_TIER: &str = "https://decision-cli.dev/ns#tier";
/// `dec:context_window` predicate — token capacity.
pub const IRI_DEC_CONTEXT_WINDOW: &str = "https://decision-cli.dev/ns#context_window";
/// `dec:max_output` predicate — max output tokens.
pub const IRI_DEC_MAX_OUTPUT: &str = "https://decision-cli.dev/ns#max_output";
/// `dec:supports_vision` predicate — image input support.
pub const IRI_DEC_SUPPORTS_VISION: &str = "https://decision-cli.dev/ns#supports_vision";
/// `dec:supports_tool_calling` predicate — function-calling support.
pub const IRI_DEC_SUPPORTS_TOOL_CALLING: &str = "https://decision-cli.dev/ns#supports_tool_calling";
/// `dec:cost_input_per_m` predicate — cost per 1M input tokens.
pub const IRI_DEC_COST_INPUT_PER_M: &str = "https://decision-cli.dev/ns#cost_input_per_m";
/// `dec:cost_output_per_m` predicate — cost per 1M output tokens.
pub const IRI_DEC_COST_OUTPUT_PER_M: &str = "https://decision-cli.dev/ns#cost_output_per_m";
/// `dec:cost_cache_hit_per_m` predicate — cost per 1M cache-hit input tokens.
pub const IRI_DEC_COST_CACHE_HIT_PER_M: &str = "https://decision-cli.dev/ns#cost_cache_hit_per_m";
/// `dec:cost_cache_write_5m` predicate — cost per 1M cache-write input tokens.
pub const IRI_DEC_COST_CACHE_WRITE_5M: &str = "https://decision-cli.dev/ns#cost_cache_write_5m";
/// `dec:cost_currency` predicate — `EUR` | `USD`.
pub const IRI_DEC_COST_CURRENCY: &str = "https://decision-cli.dev/ns#cost_currency";
/// `dec:configurable_effort` predicate — accepts `reasoning_effort`.
pub const IRI_DEC_CONFIGURABLE_EFFORT: &str = "https://decision-cli.dev/ns#configurable_effort";
/// `dec:exposes_reasoning_trace` predicate — emits separate reasoning chain.
pub const IRI_DEC_EXPOSES_REASONING_TRACE: &str =
    "https://decision-cli.dev/ns#exposes_reasoning_trace";
/// `dec:status` predicate — `active` | `preview` | `eol` | `candidate`.
pub const IRI_DEC_CAPABILITY_STATUS: &str = "https://decision-cli.dev/ns#status";
/// `dec:version` predicate — monotonically incrementing version (≥1).
pub const IRI_DEC_CAPABILITY_VERSION: &str = "https://decision-cli.dev/ns#version";
/// `dec:supersedes` predicate — link to prior version.
pub const IRI_DEC_CAPABILITY_SUPERSEDES: &str = "https://decision-cli.dev/ns#supersedes";
/// `dec:bootstrap_source` predicate — content hash of seed YAML.
pub const IRI_DEC_BOOTSTRAP_SOURCE: &str = "https://decision-cli.dev/ns#bootstrap_source";
/// `dec:notes` predicate — free-form catalog-maintainer notes.
pub const IRI_DEC_CAPABILITY_NOTES: &str = "https://decision-cli.dev/ns#notes";

/// Named graph holding the capability catalog projections (ADR-036).
pub const IRI_DEC_GRAPH_CAPABILITY: &str = "https://decision-cli.dev/ns/graph/capability";

/// IRI prefix for minted capability IRIs (`https://decision-cli.dev/ns/capability/<id>/v<version>`).
pub const IRI_DEC_CAPABILITY_PREFIX: &str = "https://decision-cli.dev/ns/capability/";

// --- Endpoint enum literals --------------------------------------------------

/// Endpoint literal — Scaleway OpenAI-compatible inference.
pub const ENDPOINT_SCALEWAY: &str = "scaleway";
/// Endpoint literal — Anthropic Messages API.
pub const ENDPOINT_ANTHROPIC: &str = "anthropic";

// --- Capability status literals ----------------------------------------------

/// Status literal — active and resolvable.
pub const CAPABILITY_STATUS_ACTIVE: &str = "active";
/// Status literal — preview / not yet stable.
pub const CAPABILITY_STATUS_PREVIEW: &str = "preview";
/// Status literal — end-of-life; dispatcher refuses.
pub const CAPABILITY_STATUS_EOL: &str = "eol";
/// Status literal — candidate for promotion; not yet bound to any role.
pub const CAPABILITY_STATUS_CANDIDATE: &str = "candidate";

// --- Currency literals -------------------------------------------------------

/// Currency literal — Euros (Scaleway).
pub const CURRENCY_EUR: &str = "EUR";
/// Currency literal — US Dollars (Anthropic).
pub const CURRENCY_USD: &str = "USD";

#[must_use]
pub fn capability_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY)
}

#[must_use]
pub fn capability_id_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_ID)
}

#[must_use]
pub fn capability_endpoint_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_ENDPOINT)
}

#[must_use]
pub fn model_identifier_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_MODEL_IDENTIFIER)
}

#[must_use]
pub fn tier_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TIER)
}

#[must_use]
pub fn context_window_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CONTEXT_WINDOW)
}

#[must_use]
pub fn max_output_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_MAX_OUTPUT)
}

#[must_use]
pub fn supports_vision_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SUPPORTS_VISION)
}

#[must_use]
pub fn supports_tool_calling_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_SUPPORTS_TOOL_CALLING)
}

#[must_use]
pub fn cost_input_per_m_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COST_INPUT_PER_M)
}

#[must_use]
pub fn cost_output_per_m_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COST_OUTPUT_PER_M)
}

#[must_use]
pub fn cost_cache_hit_per_m_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COST_CACHE_HIT_PER_M)
}

#[must_use]
pub fn cost_cache_write_5m_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COST_CACHE_WRITE_5M)
}

#[must_use]
pub fn cost_currency_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COST_CURRENCY)
}

#[must_use]
pub fn configurable_effort_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CONFIGURABLE_EFFORT)
}

#[must_use]
pub fn exposes_reasoning_trace_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EXPOSES_REASONING_TRACE)
}

#[must_use]
pub fn capability_status_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_STATUS)
}

#[must_use]
pub fn capability_version_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_VERSION)
}

#[must_use]
pub fn capability_supersedes_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_SUPERSEDES)
}

#[must_use]
pub fn bootstrap_source_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BOOTSTRAP_SOURCE)
}

#[must_use]
pub fn capability_notes_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_NOTES)
}

#[must_use]
pub fn capability_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_CAPABILITY)
}
