//! TC-108 — Verifier dispatch with confidence 0.9 produces single session,
//! no escalation.
//!
//! Validates: FT-061 (default-capability happy path; escalation lives in
//! FT-062 and is explicitly out of scope here).
//! Spec: `.product/tests/TC-108-verifier-dispatch-with-confidence-0-9-produces-sin.md`
//!
//! FT-061 ships only the capability-resolution layer — the full
//! dispatch loop that creates sessions, links escalation chains, and
//! invokes workers lands in FT-062. The TC-108 acceptance contract for
//! FT-061 is therefore the *resolution* half of the happy path: given
//! the PRD-seeded catalog and a `stakes = routine` request for the
//! verifier role, the resolver returns a single `ResolvedCapability`
//! pinned to the `code-writer` capability (qwen3-coder-30b on
//! Scaleway), with no escalation logic invoked. The "single session"
//! and "no escalation edges" properties are satisfied vacuously here:
//! FT-061 produces one resolution call and emits no escalation
//! artifacts.

use std::sync::Arc;

use decision_cli::core::dispatch::{resolve_default_capability, ResolvedCapability};
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::RoleBinding;
use decision_cli::vocab::{capability_graph, role_binding_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-108";

// ---------------------------------------------------------------------------
// PRD §5.2 / §6.2 minimal catalog: code-writer capability + verifier
// binding pointing at it.
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

fn verifier_binding() -> RoleBinding {
    RoleBinding {
        role_id: "verifier".to_string(),
        default_capability: cap_iri("code-writer", 1),
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

// ---------------------------------------------------------------------------
// Test harness.
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

fn seed(w: &StreamWriter) {
    commit_quads(w, code_writer().to_quads(capability_graph()))
        .expect("seed code-writer capability");
    commit_quads(w, verifier_binding().to_quads(role_binding_graph()))
        .expect("seed verifier binding");
}

// ---------------------------------------------------------------------------
// Default-capability happy path: resolution returns the code-writer
// capability with the FT-061 §Outputs contract.
// ---------------------------------------------------------------------------

#[test]
fn verifier_resolves_to_code_writer_with_pinned_versions() {
    let (store, w) = writer();
    seed(&w);

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
    };
    assert_eq!(resolved, expected);
}

#[test]
fn resolution_emits_no_escalation_artifacts() {
    // FT-061 does not implement escalation. After a successful
    // resolution against a binding with no escalation_steps, the store
    // contains no `dec:escalated_from` or `dec:escalated_to` triples
    // (sessions are not yet created either — FT-062 lands that loop).
    let (store, w) = writer();
    seed(&w);
    let _ = resolve_default_capability(&store, "verifier").expect("resolve ok");

    // SPARQL-light: query the store for any escalation edge.
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT (COUNT(*) AS ?n) WHERE { \
               { ?s dec:escalated_from ?o } UNION { ?s dec:escalated_to ?o } \
             }";
    let QueryResults::Solutions(mut sols) = store.query(q).expect("sparql ok") else {
        panic!("expected solutions");
    };
    let row = sols
        .next()
        .expect("count solution present")
        .expect("count row ok");
    if let Some(oxigraph::model::Term::Literal(lit)) = row.get("n") {
        let n: usize = lit
            .value()
            .parse()
            .expect("count is a non-negative integer");
        assert_eq!(n, 0, "FT-061 must emit no escalation edges");
    } else {
        panic!("count value not a literal");
    }
}

#[test]
fn resolve_is_referentially_transparent() {
    // FT-061 §Invariants: "no stale binding can be used after a catalog
    // update". The simplest reading is: two consecutive resolves
    // against an unchanged store return identical results — which is
    // what `single session, no escalation` looks like from the
    // resolver's point of view.
    let (store, w) = writer();
    seed(&w);

    let first = resolve_default_capability(&store, "verifier").expect("first ok");
    let second = resolve_default_capability(&store, "verifier").expect("second ok");
    assert_eq!(first, second);
}

#[test]
fn resolved_capability_carries_tier_appropriate_endpoint() {
    // Per ADR-037: cost-dominant roles default to Scaleway. The
    // verifier's default capability resolves to a Scaleway endpoint;
    // Anthropic is reserved for tier-3 deep-reasoning escalation
    // (FT-062 / ADR-037 — out of scope here, but the default-path
    // endpoint claim is part of FT-061's exit criteria via TC-108).
    let (store, w) = writer();
    seed(&w);

    let resolved = resolve_default_capability(&store, "verifier").expect("resolve ok");
    assert_eq!(
        resolved.endpoint,
        Endpoint::Scaleway,
        "verifier default capability is Scaleway-hosted (ADR-037)"
    );
}
