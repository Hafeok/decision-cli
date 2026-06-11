//! TC-111 — Implementer dispatch with unimplementable-critical feedback
//! escalates to code-writer-heavy.
//!
//! Validates FT-062 §Trigger evaluation: an implementer dispatch whose
//! worker emits feedback `class = unimplementable, severity = critical`
//! escalates to `code-writer-heavy` per the
//! `feedback_unimplementable_critical` trigger in the implementer's
//! seed binding.

use std::sync::Arc;

use decision_cli::core::bundle::{Bundle, Stakes};
use decision_cli::core::dispatch::{
    capability_resolver::ResolvedCapability,
    dispatch_role,
    escalation::{AttemptTokens, DispatchAttempt, EscalationError, SessionId, WorkerResult},
    WorkerRunner,
};
use decision_cli::core::feedback::FeedbackClass;
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::{EscalationStep, RoleBinding, TriggerSignal};
use decision_cli::vocab::{capability_graph, role_binding_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::dispatch::escalation::types::FeedbackArtifact;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-111";

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

fn code_writer_heavy() -> Capability {
    Capability {
        id: "code-writer-heavy".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "devstral-2-123b".to_string(),
        tier: Some(2),
        context_window: 256_000,
        max_output: 32_000,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.40".to_string(),
        cost_output_per_m: "2.00".to_string(),
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

fn implementer_binding() -> RoleBinding {
    RoleBinding {
        role_id: "implementer".to_string(),
        default_capability: cap_iri("code-writer", 1),
        escalation_steps: vec![EscalationStep {
            step_capability: cap_iri("code-writer-heavy", 1),
            triggers: vec![TriggerSignal::FeedbackUnimplementableCritical],
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
    commit(w, code_writer_heavy().to_quads(capability_graph()));
    commit(w, implementer_binding().to_quads(role_binding_graph()));
}

/// Canned implementer worker — first call emits unimplementable-critical
/// feedback and an unapplied CodeChange; second call returns success.
struct CannedImplementer;

impl WorkerRunner for CannedImplementer {
    fn run(
        &mut self,
        _role_id: &str,
        _bundle: &Bundle,
        capability: &ResolvedCapability,
        prior: &[DispatchAttempt],
        session_id: &SessionId,
    ) -> Result<DispatchAttempt, EscalationError> {
        let idx = prior.len();
        if idx == 0 {
            Ok(DispatchAttempt {
                session_id: session_id.clone(),
                capability: capability.clone(),
                result: WorkerResult::CodeChange { applied: false },
                feedback: vec![FeedbackArtifact {
                    class: FeedbackClass::Unimplementable,
                    critical: true,
                }],
                audit_outcome: None,
            })
        } else {
            Ok(DispatchAttempt {
                session_id: session_id.clone(),
                capability: capability.clone(),
                result: WorkerResult::CodeChange { applied: true },
                feedback: vec![],
                audit_outcome: None,
            })
        }
    }
}

fn routine_bundle() -> Bundle {
    Bundle {
        hash: "tc111hash".to_string(),
        focal: NamedNode::new_unchecked("https://example.com/focal"),
        stakes: Stakes::Routine,
    }
}

#[test]
fn unimplementable_critical_escalates_to_code_writer_heavy() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedImplementer;
    let chain = dispatch_role(
        &store,
        &w,
        "implementer",
        routine_bundle(),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    // Exactly two sessions.
    assert_eq!(chain.attempts.len(), 2);
    assert_eq!(chain.attempts[0].capability.capability_id, "code-writer");
    assert_eq!(
        chain.attempts[1].capability.capability_id,
        "code-writer-heavy"
    );
    // Both Scaleway (no Anthropic tier in this scenario).
    assert_eq!(chain.attempts[0].capability.endpoint, Endpoint::Scaleway);
    assert_eq!(chain.attempts[1].capability.endpoint, Endpoint::Scaleway);

    // Final result: applied CodeChange.
    match chain.final_result {
        WorkerResult::CodeChange { applied } => assert!(applied),
        _ => panic!("expected CodeChange"),
    }
}

#[test]
fn escalation_reason_is_feedback_unimplementable_critical() {
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedImplementer;
    let chain = dispatch_role(
        &store,
        &w,
        "implementer",
        routine_bundle(),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    let s2 = chain.attempts[1].session_id.as_str();
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?r WHERE {{ {{ <{s2}> dec:escalation_reason ?r }} UNION \
                            {{ GRAPH ?g {{ <{s2}> dec:escalation_reason ?r }} }} }}",
        s2 = s2,
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("select") else {
        panic!("expected solutions");
    };
    let mut reasons: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("sol");
        if let Some(Term::Literal(lit)) = sol.get("r") {
            reasons.push(lit.value().to_string());
        }
    }
    assert!(
        reasons.contains(&"feedback_unimplementable_critical".to_string()),
        "reasons = {:?}",
        reasons
    );
}

#[test]
fn second_call_with_no_feedback_does_not_trigger_third_escalation() {
    // Termination behavior: after the heavy-tier call returns no feedback
    // and no verdict, no escalation step matches; chain terminates.
    let (store, w) = writer();
    seed(&w);
    let mut runner = CannedImplementer;
    let chain = dispatch_role(
        &store,
        &w,
        "implementer",
        routine_bundle(),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");
    assert_eq!(chain.attempts.len(), 2);
    // chain_head is the first session.
    assert_eq!(chain.chain_head, chain.attempts[0].session_id);
    // Feedback was emitted on first session, absent on second.
    assert_eq!(chain.attempts[0].feedback.len(), 1);
    assert_eq!(chain.attempts[1].feedback.len(), 0);
}

#[test]
fn non_critical_unimplementable_does_not_escalate() {
    // Validate the critical-required half of the trigger: feedback with
    // critical=false should NOT escalate.
    let (store, w) = writer();
    seed(&w);
    struct NonCritical;
    impl WorkerRunner for NonCritical {
        fn run(
            &mut self,
            _role_id: &str,
            _bundle: &Bundle,
            capability: &ResolvedCapability,
            _prior: &[DispatchAttempt],
            session_id: &SessionId,
        ) -> Result<DispatchAttempt, EscalationError> {
            Ok(DispatchAttempt {
                session_id: session_id.clone(),
                capability: capability.clone(),
                result: WorkerResult::CodeChange { applied: true },
                feedback: vec![FeedbackArtifact {
                    class: FeedbackClass::Unimplementable,
                    critical: false,
                }],
                audit_outcome: None,
            })
        }
    }
    let mut runner = NonCritical;
    let chain = dispatch_role(
        &store,
        &w,
        "implementer",
        routine_bundle(),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");
    assert_eq!(chain.attempts.len(), 1, "non-critical must not escalate");
}
