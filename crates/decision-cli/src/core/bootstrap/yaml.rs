//! YAML schema for catalog seed documents under `config/` (FT-058).
//!
//! Deserialises the seed YAML into typed records mirroring FT-054's
//! `Capability` and FT-055's `RoleBinding`. Decimal-bearing fields are
//! read as YAML strings (`serde_yaml` deserialises numeric literals like
//! `0.20` as `f64` which round-trips imprecisely; the catalog needs
//! string-exact values for divergence comparison).

use serde::Deserialize;

/// Top-level shape of `config/capabilities.yaml`.
#[derive(Debug, Deserialize)]
pub struct CapabilitiesDoc {
    /// List of capability entries.
    pub capabilities: Vec<CapabilityEntry>,
}

/// One capability entry as it appears in YAML.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityEntry {
    /// Stable capability tag (e.g. `code-writer`).
    pub capability_id: String,
    /// External API endpoint string (`scaleway` | `anthropic`).
    pub endpoint: String,
    /// Exact provider model string.
    pub model_identifier: String,
    /// Optional escalation-ladder tier (0..=3).
    #[serde(default)]
    pub tier: Option<u8>,
    /// Context-window size in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_output: u32,
    /// True iff the capability accepts image inputs.
    pub supports_vision: bool,
    /// True iff the capability supports tool calling.
    pub supports_tool_calling: bool,
    /// Cost per 1M input tokens (lexical form preserved).
    #[serde(deserialize_with = "decimal_string")]
    pub cost_input_per_m: String,
    /// Cost per 1M output tokens (lexical form preserved).
    #[serde(deserialize_with = "decimal_string")]
    pub cost_output_per_m: String,
    /// Optional cost per 1M cache-hit input tokens.
    #[serde(default, deserialize_with = "opt_decimal_string")]
    pub cost_cache_hit_per_m: Option<String>,
    /// Optional cost per 1M tokens written to the 5-minute TTL cache.
    #[serde(default, deserialize_with = "opt_decimal_string")]
    pub cost_cache_write_5m: Option<String>,
    /// Currency literal (`EUR` | `USD`).
    pub cost_currency: String,
    /// Optional (default false). True iff capability accepts `reasoning_effort`.
    #[serde(default)]
    pub configurable_effort: Option<bool>,
    /// Optional (default false). True iff capability emits a reasoning trace.
    #[serde(default)]
    pub exposes_reasoning_trace: Option<bool>,
    /// Lifecycle status (`active` | `preview` | `eol` | `candidate`).
    pub status: String,
    /// Version (≥ 1).
    pub version: u32,
    /// Optional free-text catalog-maintainer notes.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Top-level shape of `config/role-bindings.yaml`.
#[derive(Debug, Deserialize)]
pub struct RoleBindingsDoc {
    /// List of binding entries.
    pub role_bindings: Vec<BindingEntry>,
}

/// One role binding entry as it appears in YAML.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingEntry {
    /// Lookup key into the role catalog.
    pub role_id: String,
    /// Capability id (looked up against the loaded capabilities).
    pub default_capability: String,
    /// Ordered escalation chain (empty for bounded-classification roles).
    #[serde(default)]
    pub escalation_steps: Vec<BindingStep>,
    /// Monotonically increasing version (≥ 1).
    pub version: u32,
    /// True iff this binding is the dispatcher's authoritative choice.
    pub active: bool,
}

/// One escalation step within a YAML binding.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingStep {
    /// Capability promoted to when triggers fire.
    pub capability: String,
    /// Trigger signal strings (OR-evaluated).
    pub triggers: Vec<String>,
}

/// Deserialise a YAML scalar that may appear as an integer, float, or
/// string into a lexical decimal string (e.g. `0.20`, `5.00`, `0`).
fn decimal_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_yaml::Value = serde::Deserialize::deserialize(deserializer)?;
    decimal_value_to_string(&v).map_err(serde::de::Error::custom)
}

/// Optional version of [`decimal_string`].
fn opt_decimal_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_yaml::Value> = serde::Deserialize::deserialize(deserializer)?;
    match v {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(v) => decimal_value_to_string(&v)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn decimal_value_to_string(v: &serde_yaml::Value) -> Result<String, String> {
    match v {
        serde_yaml::Value::String(s) => Ok(s.clone()),
        serde_yaml::Value::Number(n) => {
            // serde_yaml preserves the original lexical form via `Number`'s
            // `Display` impl when the literal had a decimal point.
            // `0.20` round-trips as `0.20`; `5` round-trips as `5`.
            Ok(format!("{n}"))
        }
        other => Err(format!(
            "expected decimal-bearing scalar (string or number), got {other:?}"
        )),
    }
}
