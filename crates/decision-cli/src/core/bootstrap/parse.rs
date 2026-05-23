//! YAML→typed parsing for catalog bootstrap (FT-058).

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::core::ontology::capability::types::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use crate::core::ontology::role_binding::types::{EscalationStep, RoleBinding, TriggerSignal};

use super::catalog::BootstrapError;
use super::yaml::{BindingEntry, CapabilitiesDoc, CapabilityEntry, RoleBindingsDoc};

/// Read and SHA-256 a YAML file. The hash is canonicalised by normalising
/// CRLF→LF and stripping trailing whitespace from each line (FT-058
/// §Behaviour step 1).
pub(super) fn load_capabilities_doc(
    path: &Path,
) -> Result<(CapabilitiesDoc, String), BootstrapError> {
    let bytes = std::fs::read(path).map_err(|e| BootstrapError::ReadFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let hash = canonical_sha256(&bytes);
    let doc: CapabilitiesDoc =
        serde_yaml::from_slice(&bytes).map_err(|e| BootstrapError::YamlParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    Ok((doc, hash))
}

pub(super) fn load_bindings_doc(
    path: &Path,
) -> Result<(RoleBindingsDoc, String), BootstrapError> {
    let bytes = std::fs::read(path).map_err(|e| BootstrapError::ReadFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let hash = canonical_sha256(&bytes);
    let doc: RoleBindingsDoc =
        serde_yaml::from_slice(&bytes).map_err(|e| BootstrapError::YamlParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    Ok((doc, hash))
}

fn canonical_sha256(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let mut canonical = String::with_capacity(text.len());
    for line in text.lines() {
        canonical.push_str(line.trim_end());
        canonical.push('\n');
    }
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn parse_capabilities(
    doc: &CapabilitiesDoc,
    source: &Path,
    hash: &str,
) -> Result<Vec<Capability>, BootstrapError> {
    let mut out = Vec::with_capacity(doc.capabilities.len());
    for entry in &doc.capabilities {
        out.push(yaml_to_capability(entry, source, hash)?);
    }
    Ok(out)
}

fn yaml_to_capability(
    e: &CapabilityEntry,
    source: &Path,
    hash: &str,
) -> Result<Capability, BootstrapError> {
    let endpoint = parse_capability_endpoint(e, source)?;
    let currency = parse_capability_currency(e, source)?;
    let status = parse_capability_status(e, source)?;
    Ok(Capability {
        id: e.capability_id.clone(),
        endpoint,
        model_identifier: e.model_identifier.clone(),
        tier: e.tier,
        context_window: e.context_window,
        max_output: e.max_output,
        supports_vision: e.supports_vision,
        supports_tool_calling: e.supports_tool_calling,
        cost_input_per_m: e.cost_input_per_m.clone(),
        cost_output_per_m: e.cost_output_per_m.clone(),
        cost_cache_hit_per_m: e.cost_cache_hit_per_m.clone(),
        cost_cache_write_5m: e.cost_cache_write_5m.clone(),
        cost_currency: currency,
        configurable_effort: e.configurable_effort,
        exposes_reasoning_trace: e.exposes_reasoning_trace,
        status,
        version: e.version,
        supersedes: None,
        bootstrap_source: Some(hash.to_string()),
        notes: e.notes.clone(),
    })
}

fn parse_capability_endpoint(
    e: &CapabilityEntry,
    source: &Path,
) -> Result<Endpoint, BootstrapError> {
    Endpoint::try_from_str(&e.endpoint).ok_or_else(|| BootstrapError::InvalidValue {
        path: source.to_path_buf(),
        detail: format!(
            "capability `{}`: unknown endpoint `{}` (allowed: scaleway | anthropic)",
            e.capability_id, e.endpoint
        ),
    })
}

fn parse_capability_currency(
    e: &CapabilityEntry,
    source: &Path,
) -> Result<CostCurrency, BootstrapError> {
    CostCurrency::try_from_str(&e.cost_currency).ok_or_else(|| BootstrapError::InvalidValue {
        path: source.to_path_buf(),
        detail: format!(
            "capability `{}`: unknown cost_currency `{}` (allowed: EUR | USD)",
            e.capability_id, e.cost_currency
        ),
    })
}

fn parse_capability_status(
    e: &CapabilityEntry,
    source: &Path,
) -> Result<CapabilityStatus, BootstrapError> {
    CapabilityStatus::try_from_str(&e.status).ok_or_else(|| BootstrapError::InvalidValue {
        path: source.to_path_buf(),
        detail: format!(
            "capability `{}`: unknown status `{}` (allowed: active | preview | eol | candidate)",
            e.capability_id, e.status
        ),
    })
}

pub(super) fn parse_bindings(
    doc: &RoleBindingsDoc,
    source: &Path,
    capabilities: &[Capability],
    hash: &str,
) -> Result<Vec<RoleBinding>, BootstrapError> {
    let index: HashMap<&str, &Capability> =
        capabilities.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut out = Vec::with_capacity(doc.role_bindings.len());
    for entry in &doc.role_bindings {
        out.push(yaml_to_binding(entry, source, &index, hash)?);
    }
    Ok(out)
}

fn yaml_to_binding(
    e: &BindingEntry,
    source: &Path,
    index: &HashMap<&str, &Capability>,
    hash: &str,
) -> Result<RoleBinding, BootstrapError> {
    let default = resolve_capability_ref(index, &e.role_id, &e.default_capability)?;
    let mut steps = Vec::with_capacity(e.escalation_steps.len());
    for step in &e.escalation_steps {
        steps.push(yaml_to_step(e, step, source, index)?);
    }
    Ok(RoleBinding {
        role_id: e.role_id.clone(),
        default_capability: default.iri(),
        escalation_steps: steps,
        version: e.version,
        active: e.active,
        supersedes: None,
        bootstrap_source: Some(hash.to_string()),
    })
}

fn yaml_to_step(
    binding: &BindingEntry,
    step: &super::yaml::BindingStep,
    source: &Path,
    index: &HashMap<&str, &Capability>,
) -> Result<EscalationStep, BootstrapError> {
    let cap = resolve_capability_ref(index, &binding.role_id, &step.capability)?;
    let mut triggers = Vec::with_capacity(step.triggers.len());
    for t in &step.triggers {
        triggers.push(parse_trigger_signal(t, &binding.role_id, source)?);
    }
    Ok(EscalationStep {
        step_capability: cap.iri(),
        triggers,
    })
}

fn resolve_capability_ref<'a>(
    index: &'a HashMap<&str, &'a Capability>,
    binding_role: &str,
    cap_id: &str,
) -> Result<&'a Capability, BootstrapError> {
    index
        .get(cap_id)
        .copied()
        .ok_or_else(|| BootstrapError::UnresolvedReference {
            binding: binding_role.to_string(),
            missing_capability: cap_id.to_string(),
        })
}

fn parse_trigger_signal(
    raw: &str,
    binding_role: &str,
    source: &Path,
) -> Result<TriggerSignal, BootstrapError> {
    TriggerSignal::try_from_str(raw).ok_or_else(|| BootstrapError::InvalidValue {
        path: source.to_path_buf(),
        detail: format!(
            "binding `{}`: unknown trigger signal `{}` (see ADR-034 vocabulary)",
            binding_role, raw
        ),
    })
}
