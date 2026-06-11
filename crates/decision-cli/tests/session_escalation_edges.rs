//! TC-103 — SessionRecord escalation edges round-trip and chain integrity.
//!
//! Validates FT-057 / ADR-034 / ADR-037. Exercises the eight acceptance
//! sub-scenarios in `.product/tests/TC-103-...md`:
//!
//! 1. Bidirectional consistency at write.
//! 2. `escalation_reason` ↔ `escalated_from` coupling.
//! 3. `escalation_chain` walks both directions and returns the chain in
//!    dispatch order regardless of which session id is passed in.
//! 4. Token-breakdown fields validated; Scaleway-no-cache enforced.
//! 5. Anthropic cache fields accepted in both write-only and read-only
//!    shapes.
//! 6. `cache_hit_rate` arithmetic; never NaN.
//! 7. `aggregate_chain_cost` per-currency rollup.
//! 8. Orphan chain surfaces a structured `SessionError::ChainBroken`.

use std::collections::BTreeMap;
use std::sync::Arc;

use decision_cli::core::graph::session::{
    aggregate_chain_cost, cache_hit_rate, escalation_chain, CapabilityCostRates, SessionError,
};
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::TriggerSignal;
use decision_cli::core::ontology::session_record::SessionRecord;
use decision_cli::vocab::{capability_graph, orchestration_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-103";

// --- Builders ---------------------------------------------------------------

fn session_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/session/{id}"))
}

fn scaleway_capability() -> Capability {
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

fn frontier_capability() -> Capability {
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

fn deep_capability() -> Capability {
    Capability {
        id: "deep-reasoning".to_string(),
        endpoint: Endpoint::Anthropic,
        model_identifier: "claude-opus-4-7".to_string(),
        tier: Some(3),
        context_window: 200_000,
        max_output: 32_000,
        supports_vision: true,
        supports_tool_calling: true,
        cost_input_per_m: "5.00".to_string(),
        cost_output_per_m: "25.00".to_string(),
        cost_cache_hit_per_m: Some("0.50".to_string()),
        cost_cache_write_5m: Some("6.25".to_string()),
        cost_currency: CostCurrency::Usd,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn seed_capabilities(w: &StreamWriter) {
    for cap in [
        scaleway_capability(),
        frontier_capability(),
        deep_capability(),
    ] {
        commit(w, cap.to_quads(capability_graph())).expect("seed capability");
    }
}

// --- TC-103 entry point — single canonical test fn the product runner looks
// up via `runner-args: tc_103_session_escalation_edges_round_trip_and_chain_integrity`.
// Invokes every per-scenario helper below.

#[test]
fn tc_103_session_escalation_edges_round_trip_and_chain_integrity() {
    bidirectional_links_are_required_at_write();
    escalation_reason_requires_escalated_from();
    escalated_from_requires_escalation_reason();
    escalation_chain_returns_full_chain_from_any_node();
    scaleway_session_with_zero_cache_fields_passes();
    scaleway_session_with_cache_hit_is_rejected();
    anthropic_cache_write_and_cache_read_sessions_pass();
    cache_hit_rate_arithmetic_is_bounded();
    aggregate_chain_cost_groups_per_currency();
    orphan_chain_returns_chain_broken_error();
}

// --- TC-103 §1: bidirectional consistency at write --------------------------

#[test]
fn bidirectional_links_are_required_at_write() {
    let (_store, w) = writer();
    seed_capabilities(&w);

    // S1: root.
    let s1 = SessionRecord {
        iri: session_iri("s1-bidi"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    commit(&w, s1.to_quads(orchestration_graph())).expect("root session writes");

    // S2 with escalated_from but WITHOUT mirroring escalated_to on S1.
    let s2_iri = session_iri("s2-bidi");
    let s2_quads = SessionRecord {
        iri: s2_iri.clone(),
        escalated_from: Some(s1.iri.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 100,
        capability: frontier_capability().iri(),
    }
    .to_quads(orchestration_graph());
    let err = commit(&w, s2_quads).expect_err("missing inverse must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("bidirectional"), "{err}");

    // Write S2 with the inverse triple included on S1 in the SAME mutation.
    let s2 = SessionRecord {
        iri: s2_iri.clone(),
        escalated_from: Some(s1.iri.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 100,
        capability: frontier_capability().iri(),
    };
    let mut quads = s2.to_quads(orchestration_graph());
    quads.extend(s2.escalated_to_quad(orchestration_graph()));
    commit(&w, quads).expect("bidirectional pair commits");
}

// --- TC-103 §2: escalation_reason ↔ escalated_from coupling -----------------

#[test]
fn escalation_reason_requires_escalated_from() {
    let (_store, w) = writer();
    seed_capabilities(&w);

    // Root session with a reason but no `escalated_from` — illegal.
    let bad_root = SessionRecord {
        iri: session_iri("bad-root"),
        escalated_from: None,
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    let err = commit(&w, bad_root.to_quads(orchestration_graph()))
        .expect_err("reason without from must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("escalation_reason"), "{err}");
}

#[test]
fn escalated_from_requires_escalation_reason() {
    let (_store, w) = writer();
    seed_capabilities(&w);

    // S1: root.
    let s1 = SessionRecord {
        iri: session_iri("s1-coupling"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    commit(&w, s1.to_quads(orchestration_graph())).expect("s1 writes");

    // S2 with `escalated_from` but no `escalation_reason`.
    let s2 = SessionRecord {
        iri: session_iri("s2-coupling"),
        escalated_from: Some(s1.iri.clone()),
        escalation_reason: None,
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 100,
        capability: frontier_capability().iri(),
    };
    let mut quads = s2.to_quads(orchestration_graph());
    quads.extend(s2.escalated_to_quad(orchestration_graph()));
    let err = commit(&w, quads).expect_err("missing reason must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("escalation_reason"), "{err}");
}

// --- TC-103 §3: escalation_chain walks both directions ---------------------

#[test]
fn escalation_chain_returns_full_chain_from_any_node() {
    let (store, w) = writer();
    seed_capabilities(&w);

    let s1 = SessionRecord {
        iri: session_iri("s1-chain"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    commit(&w, s1.to_quads(orchestration_graph())).expect("s1 writes");

    let s2 = SessionRecord {
        iri: session_iri("s2-chain"),
        escalated_from: Some(s1.iri.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 100,
        capability: frontier_capability().iri(),
    };
    let mut s2_quads = s2.to_quads(orchestration_graph());
    s2_quads.extend(s2.escalated_to_quad(orchestration_graph()));
    commit(&w, s2_quads).expect("s2 writes");

    let s3 = SessionRecord {
        iri: session_iri("s3-chain"),
        escalated_from: Some(s2.iri.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow05),
        input_tokens_base: 300,
        input_tokens_cache_write: 2000,
        input_tokens_cache_hit: 0,
        output_tokens: 200,
        capability: deep_capability().iri(),
    };
    let mut s3_quads = s3.to_quads(orchestration_graph());
    s3_quads.extend(s3.escalated_to_quad(orchestration_graph()));
    commit(&w, s3_quads).expect("s3 writes");

    // Calling from any node returns [s1, s2, s3] in order.
    for anchor in [&s1.iri, &s2.iri, &s3.iri] {
        let chain = escalation_chain(&store, anchor).expect("chain walks");
        let iris: Vec<&str> = chain.iter().map(|s| s.iri.as_str()).collect();
        assert_eq!(
            iris,
            vec![s1.iri.as_str(), s2.iri.as_str(), s3.iri.as_str()],
            "chain order must be root → leaf regardless of anchor",
        );
    }
}

// --- TC-103 §4: token-breakdown + Scaleway-no-cache -----------------------

#[test]
fn scaleway_session_with_zero_cache_fields_passes() {
    let (_store, w) = writer();
    seed_capabilities(&w);
    let s = SessionRecord {
        iri: session_iri("scaleway-clean"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    commit(&w, s.to_quads(orchestration_graph())).expect("scaleway-clean writes");
}

#[test]
fn scaleway_session_with_cache_hit_is_rejected() {
    let (_store, w) = writer();
    seed_capabilities(&w);
    let s = SessionRecord {
        iri: session_iri("scaleway-bad"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 50,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    let err = commit(&w, s.to_quads(orchestration_graph()))
        .expect_err("Scaleway with cache_hit must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("scaleway"), "{err}");
}

// --- TC-103 §5: Anthropic cache fields populated ---------------------------

#[test]
fn anthropic_cache_write_and_cache_read_sessions_pass() {
    let (_store, w) = writer();
    seed_capabilities(&w);

    // Cache write: write_tokens > 0, hit_tokens = 0.
    let write_session = SessionRecord {
        iri: session_iri("anthropic-write"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 2000,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: deep_capability().iri(),
    };
    commit(&w, write_session.to_quads(orchestration_graph())).expect("cache-write passes");

    // Cache read: hit_tokens > 0, write_tokens = 0.
    let read_session = SessionRecord {
        iri: session_iri("anthropic-read"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 2000,
        output_tokens: 50,
        capability: deep_capability().iri(),
    };
    commit(&w, read_session.to_quads(orchestration_graph())).expect("cache-read passes");
}

// --- TC-103 §6: cache_hit_rate computation ---------------------------------

#[test]
fn cache_hit_rate_arithmetic_is_bounded() {
    let (store, w) = writer();
    seed_capabilities(&w);

    let read = SessionRecord {
        iri: session_iri("rate-read"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 200,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 2000,
        output_tokens: 50,
        capability: deep_capability().iri(),
    };
    commit(&w, read.to_quads(orchestration_graph())).expect("write rate-read");

    let write = SessionRecord {
        iri: session_iri("rate-write"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 2000,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: deep_capability().iri(),
    };
    commit(&w, write.to_quads(orchestration_graph())).expect("write rate-write");

    let scaleway = SessionRecord {
        iri: session_iri("rate-scaleway"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    commit(&w, scaleway.to_quads(orchestration_graph())).expect("write scaleway");

    let chain_read = escalation_chain(&store, &read.iri).expect("rate-read chain");
    let chain_write = escalation_chain(&store, &write.iri).expect("rate-write chain");
    let chain_scaleway = escalation_chain(&store, &scaleway.iri).expect("rate-scaleway chain");

    let r_read = cache_hit_rate(&chain_read[0]);
    assert!((r_read - 0.909_090_9).abs() < 1e-4, "got {r_read}");

    let r_write = cache_hit_rate(&chain_write[0]);
    assert!((r_write - 0.0).abs() < 1e-6, "got {r_write}");

    let r_scaleway = cache_hit_rate(&chain_scaleway[0]);
    assert!(!r_scaleway.is_nan(), "never NaN");
    assert_eq!(r_scaleway, 0.0);
}

// --- TC-103 §7: aggregate_chain_cost per-currency rollup ------------------

#[test]
fn aggregate_chain_cost_groups_per_currency() {
    let (store, w) = writer();
    seed_capabilities(&w);

    let s1 = SessionRecord {
        iri: session_iri("s1-cost"),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: 1_000_000,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 100_000,
        capability: scaleway_capability().iri(),
    };
    commit(&w, s1.to_quads(orchestration_graph())).expect("s1-cost");

    let s2 = SessionRecord {
        iri: session_iri("s2-cost"),
        escalated_from: Some(s1.iri.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow05),
        input_tokens_base: 500_000,
        input_tokens_cache_write: 1_000_000,
        input_tokens_cache_hit: 0,
        output_tokens: 50_000,
        capability: deep_capability().iri(),
    };
    let mut s2_quads = s2.to_quads(orchestration_graph());
    s2_quads.extend(s2.escalated_to_quad(orchestration_graph()));
    commit(&w, s2_quads).expect("s2-cost");

    let chain = escalation_chain(&store, &s1.iri).expect("chain");

    // Build the cost-rate map from the seeded capabilities.
    let mut costs: BTreeMap<String, CapabilityCostRates> = BTreeMap::new();
    for cap in [scaleway_capability(), deep_capability()] {
        let rates = CapabilityCostRates {
            iri: cap.iri(),
            cost_input_per_m: cap.cost_input_per_m.parse().expect("parse input"),
            cost_output_per_m: cap.cost_output_per_m.parse().expect("parse output"),
            cost_cache_write_5m: cap
                .cost_cache_write_5m
                .as_ref()
                .map(|v| v.parse().expect("parse cw")),
            cost_cache_hit_per_m: cap
                .cost_cache_hit_per_m
                .as_ref()
                .map(|v| v.parse().expect("parse ch")),
            currency: cap.cost_currency.as_str().to_string(),
        };
        costs.insert(cap.iri().as_str().to_string(), rates);
    }

    let totals = aggregate_chain_cost(&chain, &costs);

    // Scaleway tier (EUR): 1M input * 0.20 + 100K output * 0.80 = 0.20 + 0.08 = 0.28
    let eur = totals.by_currency.get("EUR").copied().unwrap_or(0.0);
    assert!((eur - 0.28).abs() < 1e-6, "EUR rollup: {eur}");
    // Anthropic tier (USD): 500K * 5.00 + 1M * 6.25 + 50K * 25.00
    //   = 2.50 + 6.25 + 1.25 = 10.00
    let usd = totals.by_currency.get("USD").copied().unwrap_or(0.0);
    assert!((usd - 10.0).abs() < 1e-6, "USD rollup: {usd}");

    assert_eq!(totals.base_tokens, 1_500_000);
    assert_eq!(totals.cache_write_tokens, 1_000_000);
    assert_eq!(totals.cache_hit_tokens, 0);
    assert_eq!(totals.output_tokens, 150_000);
}

// --- TC-103 §8: orphan chain surfaces SessionError::ChainBroken ----------

#[test]
fn orphan_chain_returns_chain_broken_error() {
    let (store, w) = writer();
    seed_capabilities(&w);

    // Reference a non-existent S1 via escalated_from on S2.
    let missing_s1 = session_iri("missing-s1");
    let s2 = SessionRecord {
        iri: session_iri("orphan-s2"),
        escalated_from: Some(missing_s1.clone()),
        escalation_reason: Some(TriggerSignal::ConfidenceBelow07),
        input_tokens_base: 100,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 50,
        capability: scaleway_capability().iri(),
    };
    // The bidirectional check refuses commits that don't mirror the
    // inverse; bypass the writer chokepoint to plant the orphan triple
    // directly so we can exercise the read-side ChainBroken path.
    use oxigraph::model::GraphName;
    let g: GraphName = orchestration_graph().into_owned().into();
    for q in s2.to_quads(orchestration_graph()) {
        store
            .insert(q.as_ref())
            .expect("planting orphan session triple");
        let _ = &g;
    }

    let err = escalation_chain(&store, &s2.iri).expect_err("orphan must surface error");
    match err {
        SessionError::ChainBroken {
            session_id,
            missing_ref,
        } => {
            assert_eq!(session_id, s2.iri.as_str());
            assert_eq!(missing_ref, missing_s1.as_str());
        }
        other => panic!("expected ChainBroken, got {other:?}"),
    }
}
