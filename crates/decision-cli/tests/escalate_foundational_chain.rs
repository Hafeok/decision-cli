//! TC-110 — Verifier dispatch foundational+0.4 confidence escalates
//! twice into deep-reasoning.
//!
//! Validates FT-062 §Outputs and §Trigger evaluation: a verifier
//! dispatch with `stakes = foundational` and confidence ladder
//! 0.4 → 0.45 → 0.95 escalates twice — first to
//! `standard-reasoning-frontier`, then to `deep-reasoning` — with a
//! three-session chain bidirectionally linked. The Anthropic third
//! tier records cache-write token counts (per FT-057 / FT-065).

use std::sync::Arc;

use decision_cli::core::bundle::{Bundle, Stakes};
use decision_cli::core::dispatch::{
    capability_resolver::ResolvedCapability,
    dispatch_role,
    escalation::{AttemptTokens, DispatchAttempt, EscalationError, SessionId, WorkerResult},
    WorkerRunner,
};
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::{EscalationStep, RoleBinding, TriggerSignal};
use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::vocab::{capability_graph, role_binding_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-110";

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

fn deep_reasoning() -> Capability {
    Capability {
        id: "deep-reasoning".to_string(),
        endpoint: Endpoint::Anthropic,
        model_identifier: "claude-opus-4-7".to_string(),
        tier: Some(3),
        context_window: 200_000,
        max_output: 32_000,
        supports_vision: false,
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

fn verifier_binding() -> RoleBinding {
    RoleBinding {
        role_id: "verifier".to_string(),
        default_capability: cap_iri("code-writer", 1),
        escalation_steps: vec![
            EscalationStep {
                step_capability: cap_iri("standard-reasoning-frontier", 1),
                triggers: vec![TriggerSignal::ConfidenceBelow07],
            },
            EscalationStep {
                step_capability: cap_iri("deep-reasoning", 1),
                triggers: vec![
                    TriggerSignal::StakesFoundational,
                    TriggerSignal::ConfidenceBelow05,
                ],
            },
        ],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit(w: &StreamWriter, quads: Vec<Quad>) {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .expect("commit");
}

fn seed(w: &StreamWriter) {
    commit(w, code_writer().to_quads(capability_graph()));
    commit(
        w,
        standard_reasoning_frontier().to_quads(capability_graph()),
    );
    commit(w, deep_reasoning().to_quads(capability_graph()));
    commit(w, verifier_binding().to_quads(role_binding_graph()));
}

struct CannedVerifier {
    calls: Vec<(Verdict, Option<f32>)>,
}

impl WorkerRunner for CannedVerifier {
    fn run(
        &mut self,
        _role_id: &str,
        _bundle: &Bundle,
        capability: &ResolvedCapability,
        prior: &[DispatchAttempt],
        session_id: &SessionId,
    ) -> Result<DispatchAttempt, EscalationError> {
        let idx = prior.len();
        let (kind, confidence) = self
            .calls
            .get(idx)
            .copied()
            .unwrap_or((Verdict::Approved, Some(1.0)));
        Ok(DispatchAttempt {
            session_id: session_id.clone(),
            capability: capability.clone(),
            result: WorkerResult::Verdict { kind, confidence },
            feedback: vec![],
            audit_outcome: None,
        })
    }
}

fn foundational_bundle() -> Bundle {
    Bundle {
        hash: "tc110hash".to_string(),
        focal: NamedNode::new_unchecked("https://example.com/focal"),
        stakes: Stakes::Foundational,
    }
}

#[test]
fn foundational_low_confidence_escalates_twice() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.4)),
            (Verdict::AmendmentRequired, Some(0.45)),
            (Verdict::Approved, Some(0.95)),
        ],
    };
    // Different cache token counts per tier to validate FT-057 fields.
    let chain = dispatch_role(
        &store,
        &w,
        "verifier",
        foundational_bundle(),
        &mut runner,
        |a| {
            // Scaleway tiers have no cache; Anthropic third tier writes cache.
            if a.capability.endpoint == Endpoint::Anthropic {
                AttemptTokens {
                    input_base: 200,
                    input_cache_write: 3000,
                    input_cache_hit: 0,
                    output: 100,
                }
            } else {
                AttemptTokens {
                    input_base: 100,
                    input_cache_write: 0,
                    input_cache_hit: 0,
                    output: 50,
                }
            }
        },
    )
    .expect("dispatch ok");

    assert_eq!(chain.attempts.len(), 3, "expected three sessions");
    assert_eq!(chain.attempts[0].capability.capability_id, "code-writer");
    assert_eq!(
        chain.attempts[1].capability.capability_id,
        "standard-reasoning-frontier"
    );
    assert_eq!(chain.attempts[2].capability.capability_id, "deep-reasoning");

    // Endpoint progression.
    assert_eq!(chain.attempts[0].capability.endpoint, Endpoint::Scaleway);
    assert_eq!(chain.attempts[1].capability.endpoint, Endpoint::Scaleway);
    assert_eq!(chain.attempts[2].capability.endpoint, Endpoint::Anthropic);
}

#[test]
fn three_session_chain_is_bidirectionally_linked() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.4)),
            (Verdict::AmendmentRequired, Some(0.45)),
            (Verdict::Approved, Some(0.95)),
        ],
    };
    let chain = dispatch_role(
        &store,
        &w,
        "verifier",
        foundational_bundle(),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    let s1 = chain.attempts[0].session_id.as_str();
    let s2 = chain.attempts[1].session_id.as_str();
    let s3 = chain.attempts[2].session_id.as_str();

    // S1 → S2, S2 → S3 (forward)
    for (a, b) in &[(s1, s2), (s2, s3)] {
        let q = format!(
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             ASK {{ {{ <{a}> dec:escalated_to <{b}> }} UNION \
                    {{ GRAPH ?g {{ <{a}> dec:escalated_to <{b}> }} }} }}",
            a = a,
            b = b,
        );
        match store.query(q.as_str()).expect("ask") {
            QueryResults::Boolean(b) => assert!(b, "missing forward edge {a} → {b}"),
            _ => panic!("expected boolean"),
        }
    }

    // S2 → S1, S3 → S2 (reverse)
    for (a, b) in &[(s2, s1), (s3, s2)] {
        let q = format!(
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             ASK {{ {{ <{a}> dec:escalated_from <{b}> }} UNION \
                    {{ GRAPH ?g {{ <{a}> dec:escalated_from <{b}> }} }} }}",
            a = a,
            b = b,
        );
        match store.query(q.as_str()).expect("ask") {
            QueryResults::Boolean(b) => assert!(b, "missing reverse edge {a} → {b}"),
            _ => panic!("expected boolean"),
        }
    }
}

#[test]
fn deep_reasoning_step_records_cache_write_tokens() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.4)),
            (Verdict::AmendmentRequired, Some(0.45)),
            (Verdict::Approved, Some(0.95)),
        ],
    };
    let chain = dispatch_role(
        &store,
        &w,
        "verifier",
        foundational_bundle(),
        &mut runner,
        |a| {
            if a.capability.endpoint == Endpoint::Anthropic {
                AttemptTokens {
                    input_base: 200,
                    input_cache_write: 3000,
                    input_cache_hit: 0,
                    output: 100,
                }
            } else {
                AttemptTokens::default()
            }
        },
    )
    .expect("dispatch ok");

    let s3 = chain.attempts[2].session_id.as_str();
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?w WHERE {{ {{ <{s3}> dec:input_tokens_cache_write ?w }} UNION \
                            {{ GRAPH ?g {{ <{s3}> dec:input_tokens_cache_write ?w }} }} }}",
        s3 = s3,
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("select") else {
        panic!("expected solutions");
    };
    let mut found: u64 = 0;
    for sol in sols {
        let sol = sol.expect("sol");
        if let Some(Term::Literal(lit)) = sol.get("w") {
            if let Ok(n) = lit.value().parse() {
                found = found.max(n);
            }
        }
    }
    assert_eq!(
        found, 3000,
        "deep-reasoning session must record cache-write tokens"
    );

    // Scaleway tiers have zero cache writes.
    for s in &[
        chain.attempts[0].session_id.as_str(),
        chain.attempts[1].session_id.as_str(),
    ] {
        let q = format!(
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?w WHERE {{ {{ <{s}> dec:input_tokens_cache_write ?w }} UNION \
                                {{ GRAPH ?g {{ <{s}> dec:input_tokens_cache_write ?w }} }} }}",
            s = s,
        );
        let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("select") else {
            panic!("expected solutions");
        };
        for sol in sols {
            let sol = sol.expect("sol");
            if let Some(Term::Literal(lit)) = sol.get("w") {
                let n: u64 = lit.value().parse().unwrap_or(0);
                assert_eq!(n, 0, "Scaleway session must have zero cache_write tokens");
            }
        }
    }
}
