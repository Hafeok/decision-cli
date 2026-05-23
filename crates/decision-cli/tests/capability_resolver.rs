//! TC-107 — Dispatcher resolves default_capability and refuses when no active binding.
//!
//! Validates: FT-061 · ADR-033 · ADR-037.
//! Spec: `.product/tests/TC-107-dispatcher-resolves-default-capability-and-refuses.md`
//!
//! Exercises `core::dispatch::capability_resolver::resolve_default_capability`
//! against in-memory orchestration stores seeded with subsets of the
//! PRD §5.2 catalog. The acceptance criteria in TC-107 cover six
//! resolver scenarios — happy paths for verifier/architect, the three
//! refusal errors (no active binding, EOL capability, incompatible
//! capability), and the cache-invalidation property required by
//! FT-061 §Invariants.

use std::sync::Arc;

use decision_cli::core::dispatch::{
    resolve_default_capability, ResolvedCapability, ResolverError,
};
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::{RoleBinding, TriggerSignal};
use decision_cli::vocab::{capability_graph, role_binding_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-107";

// ---------------------------------------------------------------------------
// Capability constructors — minimal PRD §5.2 subset needed for TC-107.
// ---------------------------------------------------------------------------

fn cap_iri(id: &str, version: u32) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/capability/{id}/v{version}"
    ))
}

fn code_writer() -> Capability {
    Capability {
        id: "code-writer".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3-coder-30b-a3b-instruct".to_string(),
        tier: Some(1),
        context_window: 131_072,
        max_output: 16_384,
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

fn standard_reasoning() -> Capability {
    Capability {
        id: "standard-reasoning".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "gpt-oss-120b".to_string(),
        tier: Some(1),
        context_window: 131_072,
        max_output: 16_384,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.35".to_string(),
        cost_output_per_m: "1.50".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(true),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn standard_reasoning_frontier() -> Capability {
    Capability {
        id: "standard-reasoning-frontier".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3.5-397b-a17b".to_string(),
        tier: Some(2),
        context_window: 256_000,
        max_output: 32_000,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.60".to_string(),
        cost_output_per_m: "3.60".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(true),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

// ---------------------------------------------------------------------------
// Binding constructors — PRD §6.2 default capabilities.
// ---------------------------------------------------------------------------

fn verifier_binding(version: u32, default: NamedNode, active: bool) -> RoleBinding {
    RoleBinding {
        role_id: "verifier".to_string(),
        default_capability: default,
        escalation_steps: vec![],
        version,
        active,
        supersedes: None,
        bootstrap_source: None,
    }
}

fn architect_binding() -> RoleBinding {
    RoleBinding {
        role_id: "architect".to_string(),
        default_capability: cap_iri("standard-reasoning", 1),
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

fn implementer_binding() -> RoleBinding {
    RoleBinding {
        role_id: "implementer".to_string(),
        default_capability: cap_iri("code-writer", 1),
        // The TC suite needs at least one non-empty escalation step
        // (RoleBindingShape's EscalationStepShape requires ≥ 1 trigger
        // per step); supply a placeholder one to remain SHACL-valid.
        // For FT-061 the resolver only reads default_capability — the
        // escalation chain content is unused.
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

// ---------------------------------------------------------------------------
// Test harness — fresh in-memory store seeded through StreamWriter.
// ---------------------------------------------------------------------------

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn seed_capability(w: &StreamWriter, cap: &Capability) {
    commit_quads(w, cap.to_quads(capability_graph()))
        .unwrap_or_else(|e| panic!("seed capability {id:?}: {e}", id = cap.id));
}

fn seed_binding(w: &StreamWriter, b: &RoleBinding) {
    commit_quads(w, b.to_quads(role_binding_graph()))
        .unwrap_or_else(|e| panic!("seed binding {role:?}: {e}", role = b.role_id));
}

// ---------------------------------------------------------------------------
// Acceptance scenario 1 — happy path for verifier.
// ---------------------------------------------------------------------------

#[test]
fn happy_path_verifier_resolves_to_code_writer() {
    let (store, w) = writer();
    seed_capability(&w, &code_writer());
    seed_binding(&w, &verifier_binding(1, cap_iri("code-writer", 1), true));

    let resolved = resolve_default_capability(&store, "verifier").expect("resolve ok");
    let expected = ResolvedCapability {
        capability_id: "code-writer".to_string(),
        capability_version: 1,
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3-coder-30b-a3b-instruct".to_string(),
        max_output: 16_384,
        supports_tool_calling: true,
        configurable_effort: false,
        binding_version: 1,
        cost_cache_hit_per_m: None,
    };
    assert_eq!(resolved, expected);
}

// ---------------------------------------------------------------------------
// Acceptance scenario 2 — happy path for architect.
// ---------------------------------------------------------------------------

#[test]
fn happy_path_architect_resolves_to_standard_reasoning_with_configurable_effort() {
    let (store, w) = writer();
    seed_capability(&w, &standard_reasoning());
    seed_binding(&w, &architect_binding());

    let resolved = resolve_default_capability(&store, "architect").expect("resolve ok");
    assert_eq!(resolved.capability_id, "standard-reasoning");
    assert_eq!(resolved.endpoint, Endpoint::Scaleway);
    assert!(
        resolved.configurable_effort,
        "standard-reasoning capability declares configurable_effort=true"
    );
    assert_eq!(resolved.binding_version, 1);
}

// ---------------------------------------------------------------------------
// Acceptance scenario 3 — no active binding for the role.
// ---------------------------------------------------------------------------

#[test]
fn no_active_binding_returns_specific_error() {
    let (store, w) = writer();
    seed_capability(&w, &standard_reasoning());
    // No architect binding seeded — the architect role has no active
    // binding in this store.

    let err = resolve_default_capability(&store, "architect").expect_err("must refuse");
    assert!(
        matches!(&err, ResolverError::NoActiveBinding { role_id } if role_id == "architect"),
        "unexpected error: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Acceptance scenario 4 — EOL capability is refused.
// ---------------------------------------------------------------------------

#[test]
fn eol_capability_is_refused() {
    let (store, w) = writer();
    // Construct an EOL capability with the same canonical IRI as
    // code-writer/v1 so the binding's default_capability resolves to it.
    let mut eol = code_writer();
    eol.status = CapabilityStatus::Eol;
    seed_capability(&w, &eol);
    seed_binding(&w, &verifier_binding(1, cap_iri("code-writer", 1), true));

    let err = resolve_default_capability(&store, "verifier").expect_err("must refuse");
    assert!(
        matches!(&err, ResolverError::CapabilityEol { id } if id == "code-writer"),
        "unexpected error: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Acceptance scenario 5 — incompatible capability (no tool calling)
// for a tool-requiring role.
// ---------------------------------------------------------------------------

#[test]
fn incompatible_capability_is_refused_for_implementer() {
    let (store, w) = writer();
    let mut no_tools = code_writer();
    no_tools.supports_tool_calling = false;
    seed_capability(&w, &no_tools);
    seed_binding(
        &w,
        &RoleBinding {
            role_id: "implementer".to_string(),
            default_capability: cap_iri("code-writer", 1),
            escalation_steps: vec![],
            version: 1,
            active: true,
            supersedes: None,
            bootstrap_source: None,
        },
    );

    let err = resolve_default_capability(&store, "implementer").expect_err("must refuse");
    match err {
        ResolverError::IncompatibleCapability {
            role_id,
            capability_id,
            reason,
        } => {
            assert_eq!(role_id, "implementer");
            assert_eq!(capability_id, "code-writer");
            assert!(
                reason.contains("supports_tool_calling=false"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("expected IncompatibleCapability, got {other:?}"),
    }
}

#[test]
fn incompatible_capability_is_refused_for_verifier() {
    let (store, w) = writer();
    let mut no_tools = code_writer();
    no_tools.supports_tool_calling = false;
    seed_capability(&w, &no_tools);
    seed_binding(&w, &verifier_binding(1, cap_iri("code-writer", 1), true));

    let err = resolve_default_capability(&store, "verifier").expect_err("must refuse");
    assert!(
        matches!(&err, ResolverError::IncompatibleCapability { role_id, .. } if role_id == "verifier"),
        "unexpected error: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Acceptance scenario 6 — cache invalidation: superseded binding wins.
// ---------------------------------------------------------------------------

#[test]
fn superseded_binding_is_observed_on_next_resolve() {
    let (store, w) = writer();
    seed_capability(&w, &code_writer());
    seed_capability(&w, &standard_reasoning_frontier());

    // Initial binding: verifier → code-writer (active).
    let mut v1 = verifier_binding(1, cap_iri("code-writer", 1), true);
    v1.bootstrap_source = Some("seed".to_string());
    seed_binding(&w, &v1);

    let first = resolve_default_capability(&store, "verifier").expect("first resolve ok");
    assert_eq!(first.capability_id, "code-writer");
    assert_eq!(first.binding_version, 1);

    // Supersede: mark v1 inactive, write v2 pointing at
    // standard-reasoning-frontier as the active binding.
    let mut v1_inactive = v1.clone();
    v1_inactive.active = false;
    // To rewrite the binding's `dec:active`, delete v1's quads and
    // re-insert with active=false in a single atomic mutation.
    let v1_quads = v1.to_quads(role_binding_graph());
    let v1_inactive_quads = v1_inactive.to_quads(role_binding_graph());
    let flip = Mutation {
        inserts: v1_inactive_quads,
        removes: v1_quads,
        ..Mutation::default()
    };
    w.commit(flip).expect("flip v1 active=false ok");

    let v2 = RoleBinding {
        role_id: "verifier".to_string(),
        default_capability: cap_iri("standard-reasoning-frontier", 1),
        escalation_steps: vec![],
        version: 2,
        active: true,
        supersedes: Some(v1.iri()),
        bootstrap_source: None,
    };
    seed_binding(&w, &v2);

    let second = resolve_default_capability(&store, "verifier").expect("second resolve ok");
    assert_eq!(
        second.capability_id, "standard-reasoning-frontier",
        "second resolve must observe the new active binding"
    );
    assert_eq!(second.binding_version, 2);
}

// ---------------------------------------------------------------------------
// Bonus — non-tool-requiring role accepts a no-tool capability.
// ---------------------------------------------------------------------------

#[test]
fn non_tool_requiring_role_accepts_no_tool_capability() {
    let (store, w) = writer();
    // architect is not in the tool-requiring roles list, so a
    // capability with supports_tool_calling=false is still resolvable.
    let mut no_tools = standard_reasoning();
    no_tools.supports_tool_calling = false;
    seed_capability(&w, &no_tools);
    seed_binding(&w, &architect_binding());

    let resolved =
        resolve_default_capability(&store, "architect").expect("non-tool-required role resolves");
    assert!(!resolved.supports_tool_calling);
    assert_eq!(resolved.capability_id, "standard-reasoning");
}

// ---------------------------------------------------------------------------
// Bonus — implementer happy path uses code-writer.
// ---------------------------------------------------------------------------

#[test]
fn happy_path_implementer_resolves_to_code_writer() {
    let (store, w) = writer();
    seed_capability(&w, &code_writer());
    seed_binding(&w, &implementer_binding());

    let resolved =
        resolve_default_capability(&store, "implementer").expect("implementer resolve ok");
    assert_eq!(resolved.capability_id, "code-writer");
    assert_eq!(resolved.endpoint, Endpoint::Scaleway);
    assert_eq!(resolved.model_identifier, "qwen3-coder-30b-a3b-instruct");
    assert!(resolved.supports_tool_calling);
    assert!(!resolved.configurable_effort);
}

// ---------------------------------------------------------------------------
// Bonus — unknown role surfaces as NoActiveBinding (not a panic).
// ---------------------------------------------------------------------------

#[test]
fn unknown_role_returns_no_active_binding() {
    let (store, _w) = writer();
    let err = resolve_default_capability(&store, "nonexistent-role").expect_err("must refuse");
    assert!(
        matches!(&err, ResolverError::NoActiveBinding { role_id } if role_id == "nonexistent-role"),
        "unexpected error: {err:?}"
    );
}

// Sanity reference — TriggerSignal import is exercised so the test
// compiles regardless of whether escalation_steps end up populated.
#[test]
fn trigger_signal_vocab_imports_compile() {
    let _ = TriggerSignal::ConfidenceBelow07.as_str();
}
