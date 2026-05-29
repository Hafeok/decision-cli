//! FT-068 — verify-graph-author role + capability + binding seed quads.
//!
//! Extracted from `seeds.rs` to honour ADR-013 file-length limits.
//! Mirrors the verifier / implementer seed pattern but additionally
//! emits a `dec:Capability` and `dec:RoleBinding` so
//! `core::dispatch::capability_resolver::resolve_default_capability`
//! finds an active binding immediately after `dec init`.
//!
//! The capability is Scaleway-hosted (`qwen3-coder-30b-a3b-instruct`)
//! under a dedicated capability id (`verify-graph-author`) so the init
//! seed cannot diff against the YAML-driven `code-writer` row when the
//! operator runs `bootstrap_catalog.py`. Operators who want to rebind
//! verify-graph-author to a different capability supersede this row via
//! YAML in the usual way (FT-058 idempotency).

use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad};

use crate::core::ontology::capability::types::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use crate::core::ontology::role_binding::types::RoleBinding;
use crate::core::vocab::{capability_graph, orchestration_graph, role_binding_graph};

use super::role::{ROLE_INPUT_TYPE_IRI, ROLE_OUTPUT_TYPE_IRI};
use super::seeds::base_role_typing_quads;

/// Stable IRI minted for the verify-graph-author role catalog entry
/// (FT-068). Mirrors the verifier seed convention.
pub const VERIFY_GRAPH_AUTHOR_ROLE_IRI: &str =
    "https://decision-cli.dev/ns/role/verify-graph-author";

/// Stable string id used to look up the verify-graph-author binding via
/// `core::dispatch::capability_resolver::resolve_default_capability`.
/// Mirrors the worker manifest entry from FT-067.
pub const VERIFY_GRAPH_AUTHOR_ROLE_ID: &str = "verify-graph-author";

/// Stable capability id the verify-graph-author binding resolves to.
/// Distinct from the `code-writer` catalog row so init-time seeding
/// never collides with the YAML-driven `code-writer` row when
/// `bootstrap_catalog.py` runs later.
pub const VERIFY_GRAPH_AUTHOR_CAPABILITY_ID: &str = "verify-graph-author";

/// Build the quads that seed the verify-graph-author role catalog entry
/// alongside a minimal `dec:Capability` and `dec:RoleBinding`.
#[must_use]
pub fn verify_graph_author_seed_quads() -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let role = NamedNode::new_unchecked(VERIFY_GRAPH_AUTHOR_ROLE_IRI);
    let mut quads = verify_graph_author_role_quads(&role, &g);
    let capability = verify_graph_author_capability();
    quads.extend(capability.to_quads(capability_graph()));
    let binding = verify_graph_author_binding(&capability.iri());
    quads.extend(binding.to_quads(role_binding_graph()));
    quads
}

fn verify_graph_author_role_quads(role: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let mut quads = base_role_typing_quads(role, VERIFY_GRAPH_AUTHOR_ROLE_ID, g);
    let role_in = NamedNodeRef::new_unchecked(ROLE_INPUT_TYPE_IRI).into_owned();
    let feature_spec = NamedNode::new_unchecked("https://decision-cli.dev/ns#FeatureSpec");
    let test_criterion = NamedNode::new_unchecked("https://decision-cli.dev/ns#TestCriterion");
    let verification_bench =
        NamedNode::new_unchecked("https://decision-cli.dev/ns#VerificationBench");
    quads.push(Quad::new(
        role.clone(),
        role_in.clone(),
        feature_spec,
        g.clone(),
    ));
    quads.push(Quad::new(
        role.clone(),
        role_in.clone(),
        test_criterion,
        g.clone(),
    ));
    quads.push(Quad::new(role.clone(), role_in, verification_bench, g.clone()));
    // Output is the proposal artifact; no authority link (ADR-030 §7
    // makes Level-3 review the only acceptance path, so the
    // verify-graph-author role does not carry an authority declaration).
    let role_out = NamedNodeRef::new_unchecked(ROLE_OUTPUT_TYPE_IRI).into_owned();
    let proposal = NamedNode::new_unchecked("https://decision-cli.dev/ns#GraphProposal");
    quads.push(Quad::new(role.clone(), role_out, proposal, g.clone()));
    quads
}

fn verify_graph_author_capability() -> Capability {
    Capability {
        id: VERIFY_GRAPH_AUTHOR_CAPABILITY_ID.to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3-coder-30b-a3b-instruct".to_string(),
        tier: Some(1),
        context_window: 128_000,
        max_output: 32_000,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.20".to_string(),
        cost_output_per_m: "0.80".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn verify_graph_author_binding(capability_iri: &NamedNode) -> RoleBinding {
    RoleBinding {
        role_id: VERIFY_GRAPH_AUTHOR_ROLE_ID.to_string(),
        default_capability: capability_iri.clone(),
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}
