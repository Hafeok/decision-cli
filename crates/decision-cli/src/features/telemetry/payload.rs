//! JSON payload for LiteLLM telemetry POSTed to `/llm-call-telemetry` (FT-096).
//!
//! Wire shape mirrors the `TelemetryRecord` dataclass shipped by the
//! `pipeline-cli-telemetry` LiteLLM callback in
//! `workers/litellm-telemetry-callback/src/litellm_telemetry_callback/callback.py`.
//! Field names match 1:1 so any change to one side breaks parity loudly.

use serde::{Deserialize, Serialize};

/// One LiteLLM call's telemetry, indexed by `ddd_session_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryPayload {
    /// The DDD session id the worker propagated as metadata. Empty
    /// string is permitted at the wire layer (so a malformed call from
    /// LiteLLM still reaches us as evidence); the service layer rejects
    /// records whose `ddd_session_id` is empty as `MissingSessionId`.
    pub ddd_session_id: String,
    /// Resolved model identifier (e.g. `frontier-reasoning`). This is
    /// the framework's capability tag, not the underlying provider
    /// model — LiteLLM resolves the latter from its `model_list`.
    pub model: String,
    /// Concrete provider LiteLLM routed through (e.g. `anthropic`).
    pub provider: String,
    /// Capability tag the worker asked for at call time. Equal to
    /// `model` in the common path; preserved as a separate field so
    /// future model-rebinding does not erase the call's original
    /// routing intent.
    pub capability_tag: String,
    /// Prompt tokens reported by LiteLLM.
    pub input_tokens: u64,
    /// Completion tokens reported by LiteLLM.
    pub output_tokens: u64,
    /// LiteLLM's authoritative cost figure in USD. Source of truth per
    /// ADR-064.
    pub cost_usd: f64,
    /// End-to-end LiteLLM call latency in milliseconds.
    pub latency_ms: u64,
    /// Number of retries LiteLLM attempted before settling on the
    /// returned response.
    pub retry_count: u32,
    /// Ordered list of fallback model groups LiteLLM walked. Empty
    /// when the first attempt succeeded.
    #[serde(default)]
    pub fallback_chain: Vec<String>,
}

impl TelemetryPayload {
    /// Whether the payload's `ddd_session_id` is non-empty after trim.
    #[must_use]
    pub fn has_session_id(&self) -> bool {
        !self.ddd_session_id.trim().is_empty()
    }
}
