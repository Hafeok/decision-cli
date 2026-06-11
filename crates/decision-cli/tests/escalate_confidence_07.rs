//! TC-109 — Verifier dispatch with confidence 0.6 escalates to
//! standard-reasoning-frontier with bidirectional linkage.
//!
//! Validates FT-062 §Outputs and §Trigger evaluation: a verifier
//! dispatch whose worker returns `confidence = 0.6` produces a second
//! session with the `standard-reasoning-frontier` capability binding;
//! the two sessions are linked via `escalated_to` / `escalated_from`;
//! the escalation reason is `confidence_below_0.7`. The escalated
//! session's bundle is enriched with the prior attempt per ADR-034.

use std::sync::Arc;

use decision_cli::core::bundle::{Bundle, Stakes};
use decision_cli::core::dispatch::{
    capability_resolver::ResolvedCapability,
    dispatch_role,
    escalation::{
        bundle_enrich::render_prior_attempt_block, types::FeedbackArtifact, AttemptTokens,
        DispatchAttempt, EscalationError, SessionId, WorkerResult,
    },
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

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-109";

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

fn verifier_binding() -> RoleBinding {
    RoleBinding {
        role_id: "verifier".to_string(),
        default_capability: cap_iri("code-writer", 1),
        escalation_steps: vec![EscalationStep {
            step_capability: cap_iri("standard-reasoning-frontier", 1),
            triggers: vec![TriggerSignal::ConfidenceBelow07],
        }],
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
    commit(w, verifier_binding().to_quads(role_binding_graph()));
}

/// Canned verifier worker: low confidence on first call, high on second.
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

fn bundle() -> Bundle {
    Bundle {
        hash: "tc109hash".to_string(),
        focal: NamedNode::new_unchecked("https://example.com/focal"),
        stakes: Stakes::Routine,
    }
}

#[test]
fn confidence_06_escalates_to_standard_reasoning_frontier() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.6)),
            (Verdict::Approved, Some(0.92)),
        ],
    };
    let chain = dispatch_role(&store, &w, "verifier", bundle(), &mut runner, |_| {
        AttemptTokens::default()
    })
    .expect("dispatch ok");

    // Exactly two sessions.
    assert_eq!(chain.attempts.len(), 2, "expected two sessions");
    // First session has code-writer capability; second has standard-reasoning-frontier.
    assert_eq!(chain.attempts[0].capability.capability_id, "code-writer");
    assert_eq!(
        chain.attempts[1].capability.capability_id,
        "standard-reasoning-frontier"
    );

    // chain_head is the first session.
    assert_eq!(chain.chain_head, chain.attempts[0].session_id);

    // Final result is the approved one.
    match chain.final_result {
        WorkerResult::Verdict { kind, .. } => assert_eq!(kind, Verdict::Approved),
        _ => panic!("expected verdict"),
    }
}

#[test]
fn escalation_edges_are_bidirectional_and_carry_reason() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.6)),
            (Verdict::Approved, Some(0.92)),
        ],
    };
    let chain = dispatch_role(&store, &w, "verifier", bundle(), &mut runner, |_| {
        AttemptTokens::default()
    })
    .expect("dispatch ok");

    let s1 = &chain.attempts[0].session_id;
    let s2 = &chain.attempts[1].session_id;

    // Forward edge S1 → S2 via dec:escalated_to.
    let q_forward = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ {{ <{s1}> dec:escalated_to <{s2}> }} UNION \
                {{ GRAPH ?g {{ <{s1}> dec:escalated_to <{s2}> }} }} }}",
        s1 = s1.as_str(),
        s2 = s2.as_str(),
    );
    match store.query(q_forward.as_str()).expect("ask") {
        QueryResults::Boolean(b) => assert!(b, "expected S1 dec:escalated_to S2"),
        _ => panic!("expected boolean result"),
    }

    // Reverse edge S2 → S1 via dec:escalated_from.
    let q_back = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ {{ <{s2}> dec:escalated_from <{s1}> }} UNION \
                {{ GRAPH ?g {{ <{s2}> dec:escalated_from <{s1}> }} }} }}",
        s1 = s1.as_str(),
        s2 = s2.as_str(),
    );
    match store.query(q_back.as_str()).expect("ask") {
        QueryResults::Boolean(b) => assert!(b, "expected S2 dec:escalated_from S1"),
        _ => panic!("expected boolean result"),
    }

    // Escalation reason on S2 is confidence_below_0.7.
    let q_reason = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?r WHERE {{ {{ <{s2}> dec:escalation_reason ?r }} UNION \
                            {{ GRAPH ?g {{ <{s2}> dec:escalation_reason ?r }} }} }}",
        s2 = s2.as_str(),
    );
    let QueryResults::Solutions(sols) = store.query(q_reason.as_str()).expect("select") else {
        panic!("expected solutions");
    };
    let mut reasons: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(Term::Literal(lit)) = sol.get("r") {
            reasons.push(lit.value().to_string());
        }
    }
    assert!(
        reasons.contains(&"confidence_below_0.7".to_string()),
        "reasons = {:?}",
        reasons
    );

    // Root session (S1) carries no escalation_reason.
    let q_root = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ {{ <{s1}> dec:escalation_reason ?r }} UNION \
                {{ GRAPH ?g {{ <{s1}> dec:escalation_reason ?r }} }} }}",
        s1 = s1.as_str(),
    );
    match store.query(q_root.as_str()).expect("ask") {
        QueryResults::Boolean(b) => assert!(!b, "root session must not carry escalation_reason"),
        _ => panic!("expected boolean result"),
    }
}

#[test]
fn enriched_bundle_carries_prior_attempt_framing() {
    // Pure-function rendering: the enriched bundle's prior-attempt
    // markdown block contains the required ADR-034 framing.
    let prior = DispatchAttempt {
        session_id: NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1"),
        capability: ResolvedCapability {
            capability_id: "code-writer".to_string(),
            capability_version: 1,
            endpoint: Endpoint::Scaleway,
            model_identifier: "qwen3-coder-30b-a3b-instruct".to_string(),
            max_output: 16_384,
            supports_tool_calling: true,
            configurable_effort: false,
            binding_version: 1,
            cost_cache_hit_per_m: None,
        },
        result: WorkerResult::Verdict {
            kind: Verdict::AmendmentRequired,
            confidence: Some(0.6),
        },
        feedback: vec![],
        audit_outcome: None,
    };
    let block = render_prior_attempt_block(&prior, 1);
    assert!(
        block.contains(
            "## Prior attempt (tier 1, capability code-writer, model qwen3-coder-30b-a3b-instruct)"
        ),
        "missing prior-attempt header: {}",
        block
    );
    assert!(
        block.contains("agree, refute, or refine"),
        "missing ADR-034 framing: {}",
        block
    );
}

#[test]
fn no_third_session_when_confidence_high_after_escalation() {
    // The second session's high confidence terminates the chain — no
    // further escalation step matches.
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedVerifier {
        calls: vec![
            (Verdict::AmendmentRequired, Some(0.6)),
            (Verdict::Approved, Some(0.95)),
        ],
    };
    let chain = dispatch_role(&store, &w, "verifier", bundle(), &mut runner, |_| {
        AttemptTokens::default()
    })
    .expect("dispatch ok");
    assert_eq!(chain.attempts.len(), 2);
}

// Hidden import guard so the FeedbackArtifact type exists in scope.
#[allow(dead_code)]
fn _import_guard(_: FeedbackArtifact) {}
