//! Field-level divergence comparison for catalog bootstrap (FT-058).
//!
//! Compares a YAML-derived `Capability` / `RoleBinding` against the
//! corresponding stored artifact. `dec:bootstrap_source` is intentionally
//! ignored — the source hash is metadata about *how* the artifact was
//! created, not part of its content.

use dec_graph::ontology::capability::types::Capability;
use dec_graph::ontology::role_binding::types::{EscalationStep, RoleBinding};

/// One field-level disagreement between a YAML entry and the stored artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    /// Field name (e.g. `cost_input_per_m`).
    pub field: String,
    /// Value loaded from YAML.
    pub yaml_value: String,
    /// Value found in the graph.
    pub graph_value: String,
}

/// Compare a YAML-derived capability against the stored one. Returns the
/// list of diverging fields; an empty vec means the two are equivalent
/// up to `dec:bootstrap_source` (which is intentionally not compared).
#[must_use]
pub fn diff_capability(yaml: &Capability, stored: &Capability) -> Vec<FieldDiff> {
    let mut out = Vec::new();
    diff_capability_strings(yaml, stored, &mut out);
    diff_capability_numerics(yaml, stored, &mut out);
    diff_capability_decimals(yaml, stored, &mut out);
    diff_capability_booleans(yaml, stored, &mut out);
    diff_capability_enums(yaml, stored, &mut out);
    out
}

fn diff_capability_strings(yaml: &Capability, stored: &Capability, out: &mut Vec<FieldDiff>) {
    push_if(out, "capability_id", &yaml.id, &stored.id);
    push_if(
        out,
        "model_identifier",
        &yaml.model_identifier,
        &stored.model_identifier,
    );
}

fn diff_capability_numerics(yaml: &Capability, stored: &Capability, out: &mut Vec<FieldDiff>) {
    push_if(
        out,
        "tier",
        &opt_string(yaml.tier),
        &opt_string(stored.tier),
    );
    push_if(
        out,
        "context_window",
        &yaml.context_window.to_string(),
        &stored.context_window.to_string(),
    );
    push_if(
        out,
        "max_output",
        &yaml.max_output.to_string(),
        &stored.max_output.to_string(),
    );
    push_if(
        out,
        "version",
        &yaml.version.to_string(),
        &stored.version.to_string(),
    );
}

fn diff_capability_decimals(yaml: &Capability, stored: &Capability, out: &mut Vec<FieldDiff>) {
    push_if(
        out,
        "cost_input_per_m",
        &canon_decimal(&yaml.cost_input_per_m),
        &canon_decimal(&stored.cost_input_per_m),
    );
    push_if(
        out,
        "cost_output_per_m",
        &canon_decimal(&yaml.cost_output_per_m),
        &canon_decimal(&stored.cost_output_per_m),
    );
    push_if(
        out,
        "cost_cache_hit_per_m",
        &opt_decimal(&yaml.cost_cache_hit_per_m),
        &opt_decimal(&stored.cost_cache_hit_per_m),
    );
    push_if(
        out,
        "cost_cache_write_5m",
        &opt_decimal(&yaml.cost_cache_write_5m),
        &opt_decimal(&stored.cost_cache_write_5m),
    );
}

fn diff_capability_booleans(yaml: &Capability, stored: &Capability, out: &mut Vec<FieldDiff>) {
    push_if(
        out,
        "supports_vision",
        &yaml.supports_vision.to_string(),
        &stored.supports_vision.to_string(),
    );
    push_if(
        out,
        "supports_tool_calling",
        &yaml.supports_tool_calling.to_string(),
        &stored.supports_tool_calling.to_string(),
    );
    push_if(
        out,
        "configurable_effort",
        &opt_bool(yaml.configurable_effort),
        &opt_bool(stored.configurable_effort),
    );
    push_if(
        out,
        "exposes_reasoning_trace",
        &opt_bool(yaml.exposes_reasoning_trace),
        &opt_bool(stored.exposes_reasoning_trace),
    );
}

fn diff_capability_enums(yaml: &Capability, stored: &Capability, out: &mut Vec<FieldDiff>) {
    push_if(
        out,
        "endpoint",
        yaml.endpoint.as_str(),
        stored.endpoint.as_str(),
    );
    push_if(
        out,
        "cost_currency",
        yaml.cost_currency.as_str(),
        stored.cost_currency.as_str(),
    );
    push_if(out, "status", yaml.status.as_str(), stored.status.as_str());
}

/// Compare a YAML-derived binding against the stored one. The escalation
/// chain is compared as an ordered list — re-ordering counts as divergence.
#[must_use]
pub fn diff_binding(yaml: &RoleBinding, stored: &RoleBinding) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    push_if(&mut diffs, "role_id", &yaml.role_id, &stored.role_id);
    push_if(
        &mut diffs,
        "default_capability",
        yaml.default_capability.as_str(),
        stored.default_capability.as_str(),
    );
    push_if(
        &mut diffs,
        "version",
        &yaml.version.to_string(),
        &stored.version.to_string(),
    );
    push_if(
        &mut diffs,
        "active",
        &yaml.active.to_string(),
        &stored.active.to_string(),
    );
    push_if(
        &mut diffs,
        "escalation_steps",
        &steps_signature(&yaml.escalation_steps),
        &steps_signature(&stored.escalation_steps),
    );
    diffs
}

fn push_if(out: &mut Vec<FieldDiff>, field: &str, yaml: &str, graph: &str) {
    if yaml != graph {
        out.push(FieldDiff {
            field: field.to_string(),
            yaml_value: yaml.to_string(),
            graph_value: graph.to_string(),
        });
    }
}

fn opt_string<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "<none>".into())
}

/// Canonicalise an xsd:decimal lexical form to match oxigraph's
/// serialisation: trim trailing zeros after the decimal point, then drop
/// the trailing `.` if nothing follows. E.g. `0.10` → `0.1`, `5.00` → `5`.
fn canon_decimal(s: &str) -> String {
    let s = s.trim();
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    let trimmed = trimmed.trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn opt_decimal(v: &Option<String>) -> String {
    v.as_ref()
        .map(|x| canon_decimal(x))
        .unwrap_or_else(|| "<none>".into())
}

fn opt_bool(v: Option<bool>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "<none>".into())
}

fn steps_signature(steps: &[EscalationStep]) -> String {
    let mut sig = String::new();
    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            sig.push_str("; ");
        }
        sig.push_str(step.step_capability.as_str());
        sig.push_str(" → [");
        for (j, t) in step.triggers.iter().enumerate() {
            if j > 0 {
                sig.push(',');
            }
            sig.push_str(t.as_str());
        }
        sig.push(']');
    }
    sig
}
