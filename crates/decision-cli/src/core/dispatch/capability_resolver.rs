//! Graph-driven capability resolution per FT-061 / ADR-033.
//!
//! Given a `role_id` and an Oxigraph orchestration store, this module
//! resolves the role's active `dec:RoleBinding` to a concrete
//! `dec:Capability` artifact, performs the FT-061 §Behaviour
//! compatibility check, and returns a [`ResolvedCapability`] the
//! dispatcher injects into the dispatch payload. The worker layer
//! (FT-060 ModelRouter) consumes the `(endpoint, model_identifier,
//! parameters)` triple verbatim — workers stay ignorant of bindings
//! and capabilities per ADR-008.
//!
//! Scope of FT-061 is the *default-capability* path. Escalation (FT-062)
//! is out of scope here; this module just returns the binding's
//! `default_capability` plus the version pins the dispatcher records on
//! the session for reproducibility.

use oxigraph::store::Store;
use thiserror::Error;

use crate::core::ontology::capability::{
    query_by_iri, Capability, CapabilityReadError, CapabilityStatus, Endpoint,
};
use crate::core::ontology::role_binding::{active_for_role, RoleBindingReadError};

/// Output of [`resolve_default_capability`] — the concrete (endpoint,
/// model_identifier, parameters) triple plus the version pins the
/// dispatcher records on the session record for reproducibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCapability {
    /// `dec:capability_id` literal (the stable tag, e.g. `code-writer`).
    pub capability_id: String,
    /// Version of the `dec:Capability` artifact that produced this
    /// resolution.
    pub capability_version: u32,
    /// External API endpoint the worker dispatches to (`scaleway` |
    /// `anthropic`).
    pub endpoint: Endpoint,
    /// Exact provider model identifier the worker forwards to the SDK.
    pub model_identifier: String,
    /// Maximum output tokens supported by this capability.
    pub max_output: u32,
    /// True iff capability supports function/tool calling. Bindings for
    /// roles whose workers require tool calling (`implementer`,
    /// `verifier`) refuse to resolve to a capability with this `false`.
    pub supports_tool_calling: bool,
    /// True iff capability accepts `reasoning_effort` parameter
    /// (consumed by FT-063 to populate `parameters.reasoning_effort`).
    pub configurable_effort: bool,
    /// Version of the `dec:RoleBinding` artifact that produced this
    /// resolution.
    pub binding_version: u32,
}

/// Structured failures from [`resolve_default_capability`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ResolverError {
    /// No active `dec:RoleBinding` exists for `role_id`. The dispatcher
    /// refuses to fall back to a hardcoded model per ADR-037.
    #[error(
        "role {role_id:?} has no active binding; run `dec init` or supersede manually"
    )]
    NoActiveBinding {
        /// Role the dispatcher tried to resolve.
        role_id: String,
    },

    /// The binding's `default_capability` points at a `dec:Capability`
    /// artifact that doesn't exist in the store. Indicates a corrupted
    /// catalog (the binding was written without its capability).
    #[error("binding for role {role_id:?} references unknown capability <{iri}>")]
    UnknownCapability {
        /// Role being resolved.
        role_id: String,
        /// IRI of the missing capability artifact.
        iri: String,
    },

    /// The resolved capability's `dec:status` is `eol` — dispatch
    /// refused (FT-061 §Error handling).
    #[error("capability {id:?} is end-of-life; dispatch refused")]
    CapabilityEol {
        /// Capability id (the stable tag, not the IRI).
        id: String,
    },

    /// The resolved capability is incompatible with the role's worker
    /// tool requirements. Currently fires when `implementer` or
    /// `verifier` bind to a capability with `supports_tool_calling =
    /// false` per FT-061 §Outputs.
    #[error("capability {capability_id:?} incompatible with role {role_id:?}: {reason}")]
    IncompatibleCapability {
        /// Role being resolved.
        role_id: String,
        /// Capability id (the stable tag).
        capability_id: String,
        /// Human-readable diagnostic.
        reason: String,
    },

    /// SPARQL / graph read failure. Bubbles from the role-binding or
    /// capability readers.
    #[error("graph read error: {detail}")]
    GraphError {
        /// Diagnostic from the underlying reader.
        detail: String,
    },
}

impl From<RoleBindingReadError> for ResolverError {
    fn from(value: RoleBindingReadError) -> Self {
        Self::GraphError {
            detail: value.to_string(),
        }
    }
}

impl From<CapabilityReadError> for ResolverError {
    fn from(value: CapabilityReadError) -> Self {
        Self::GraphError {
            detail: value.to_string(),
        }
    }
}

/// Roles whose workers structurally require tool calling. A capability
/// resolved to one of these roles with `supports_tool_calling = false`
/// is rejected at resolution time.
const TOOL_REQUIRING_ROLES: &[&str] = &["implementer", "verifier"];

/// Resolve the default capability for `role_id` against `store`.
///
/// Walks `dec:RoleBinding (active=true, role_id=role_id)` → its
/// `dec:default_capability` IRI → the `dec:Capability` artifact, checks
/// status and tool-calling compatibility, and returns the resolved
/// triple.
///
/// Errors per FT-061 §Error handling:
///
/// - [`ResolverError::NoActiveBinding`] — no active binding for the
///   role.
/// - [`ResolverError::CapabilityEol`] — resolved capability is EOL.
/// - [`ResolverError::IncompatibleCapability`] — the role's worker
///   requires tool calling and the capability does not support it.
/// - [`ResolverError::GraphError`] — underlying graph read failed.
pub fn resolve_default_capability(
    store: &Store,
    role_id: &str,
) -> Result<ResolvedCapability, ResolverError> {
    let binding = active_for_role(store, role_id)?.ok_or_else(|| ResolverError::NoActiveBinding {
        role_id: role_id.to_string(),
    })?;

    let capability = query_by_iri(store, &binding.default_capability)?.ok_or_else(|| {
        ResolverError::UnknownCapability {
            role_id: role_id.to_string(),
            iri: binding.default_capability.as_str().to_string(),
        }
    })?;

    enforce_status(&capability)?;
    enforce_tool_compatibility(role_id, &capability)?;

    Ok(build_resolved(&capability, binding.version))
}

fn enforce_status(capability: &Capability) -> Result<(), ResolverError> {
    if capability.status == CapabilityStatus::Eol {
        return Err(ResolverError::CapabilityEol {
            id: capability.id.clone(),
        });
    }
    Ok(())
}

fn enforce_tool_compatibility(role_id: &str, capability: &Capability) -> Result<(), ResolverError> {
    if !TOOL_REQUIRING_ROLES.contains(&role_id) {
        return Ok(());
    }
    if capability.supports_tool_calling {
        return Ok(());
    }
    Err(ResolverError::IncompatibleCapability {
        role_id: role_id.to_string(),
        capability_id: capability.id.clone(),
        reason: "supports_tool_calling=false".to_string(),
    })
}

fn build_resolved(capability: &Capability, binding_version: u32) -> ResolvedCapability {
    ResolvedCapability {
        capability_id: capability.id.clone(),
        capability_version: capability.version,
        endpoint: capability.endpoint,
        model_identifier: capability.model_identifier.clone(),
        max_output: capability.max_output,
        supports_tool_calling: capability.supports_tool_calling,
        configurable_effort: capability.configurable_effort.unwrap_or(false),
        binding_version,
    }
}
