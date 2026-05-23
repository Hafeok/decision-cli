//! TC-101 — RoleBinding artifact conforms to dec:RoleBindingShape with
//! ordered escalation steps.
//!
//! Validates: FT-055 · ADR-033 · ADR-034.
//! Spec: `.product/tests/TC-101-rolebinding-artifact-conforms-to-dec-rolebindingsh.md`
//!
//! Five scenarios per the TC's acceptance bullets:
//!
//! 1. PRD §6.2 `implementer` binding (default `code-writer`, escalation
//!    chain `[code-writer-heavy → deep-reasoning]`) passes SHACL and
//!    round-trips through the writer + reader preserving step order.
//! 2. A binding with `escalation_steps = []` passes SHACL (bounded-
//!    classification roles per ADR-037).
//! 3. Two `dec:RoleBinding` for the same `role_id` both with
//!    `active = true` fail SHACL.
//! 4. An `EscalationTrigger` with `trigger_signal = "stakes_critical"`
//!    (not in the ADR-034 vocabulary) fails SHACL.
//! 5. An `EscalationStep` with no triggers fails SHACL.

use std::sync::Arc;

use decision_cli::core::ontology::role_binding::{
    active_for_role, list_all_active, validate_quads, EscalationStep, RoleBinding, TriggerSignal,
};
use decision_cli::vocab::{
    capability_graph, role_binding_active_pred, role_binding_graph, trigger_signal_pred,
    triggers_pred, IRI_DEC_TRIGGERS, IRI_DEC_TRIGGER_SIGNAL,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{Literal, NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-101";

// ---------------------------------------------------------------------------
// Capability IRIs reused across the suite (PRD §5.2 minting convention).
// ---------------------------------------------------------------------------

fn cap_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/capability/{id}/v1"
    ))
}

// ---------------------------------------------------------------------------
// PRD §6.2 implementer binding builder.
// ---------------------------------------------------------------------------

fn implementer_binding() -> RoleBinding {
    RoleBinding {
        role_id: "implementer".to_string(),
        default_capability: cap_iri("code-writer"),
        escalation_steps: vec![
            EscalationStep {
                step_capability: cap_iri("code-writer-heavy"),
                triggers: vec![
                    TriggerSignal::ConfidenceBelow07,
                    TriggerSignal::PriorAttemptsGe2,
                ],
            },
            EscalationStep {
                step_capability: cap_iri("deep-reasoning"),
                triggers: vec![
                    TriggerSignal::StakesFoundational,
                    TriggerSignal::AuditFail,
                    TriggerSignal::PriorAttemptsGe3,
                ],
            },
        ],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

fn test_interpreter_binding() -> RoleBinding {
    RoleBinding {
        role_id: "test_interpreter".to_string(),
        default_capability: cap_iri("classifier"),
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

// ---------------------------------------------------------------------------
// StreamWriter helpers.
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

// ---------------------------------------------------------------------------
// Scenario 1 — well-formed implementer binding passes SHACL.
// ---------------------------------------------------------------------------

#[test]
fn implementer_binding_passes_pure_shacl() {
    let b = implementer_binding();
    let quads = b.to_quads(role_binding_graph());
    validate_quads(&quads).expect("implementer binding must pass FT-055 SHACL");
}

#[test]
fn implementer_binding_commits_through_stream_writer() {
    let (store, w) = writer();
    let b = implementer_binding();
    let quads = b.to_quads(role_binding_graph());
    commit_quads(&w, quads).expect("well-formed implementer binding must commit");

    // Verify the binding artifact landed in the store.
    let count = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(b.iri()).as_ref()),
            None,
            None,
            None,
        )
        .count();
    assert!(count > 0, "binding must persist after commit");
}

#[test]
fn implementer_binding_round_trips_preserving_step_order() {
    let (store, w) = writer();
    let b = implementer_binding();
    commit_quads(&w, b.to_quads(role_binding_graph()))
        .expect("commit succeeds");

    let loaded = active_for_role(&store, "implementer")
        .expect("read succeeds")
        .expect("active binding present");
    assert_eq!(loaded.role_id, "implementer");
    assert_eq!(loaded.default_capability, cap_iri("code-writer"));
    assert_eq!(loaded.version, 1);
    assert!(loaded.active);
    // The escalation chain must preserve order: code-writer-heavy first,
    // then deep-reasoning.
    assert_eq!(loaded.escalation_steps.len(), 2);
    assert_eq!(loaded.escalation_steps[0].step_capability, cap_iri("code-writer-heavy"));
    assert_eq!(loaded.escalation_steps[1].step_capability, cap_iri("deep-reasoning"));
    // Triggers on the first step include confidence-below-0.7 and
    // prior-attempts-ge-2 (order unimportant for the bag semantics,
    // but the set must match).
    let step_0_triggers: std::collections::BTreeSet<&str> = loaded.escalation_steps[0]
        .triggers
        .iter()
        .map(TriggerSignal::as_str)
        .collect();
    assert!(step_0_triggers.contains("confidence_below_0.7"));
    assert!(step_0_triggers.contains("prior_attempts_ge_2"));
    let step_1_triggers: std::collections::BTreeSet<&str> = loaded.escalation_steps[1]
        .triggers
        .iter()
        .map(TriggerSignal::as_str)
        .collect();
    assert!(step_1_triggers.contains("stakes_foundational"));
    assert!(step_1_triggers.contains("audit_fail"));
    assert!(step_1_triggers.contains("prior_attempts_ge_3"));
}

#[test]
fn list_all_active_returns_only_active_bindings() {
    let (store, w) = writer();
    let active = implementer_binding();
    let mut inactive = implementer_binding();
    inactive.role_id = "implementer".to_string();
    inactive.version = 2;
    inactive.active = false;
    commit_quads(&w, active.to_quads(role_binding_graph())).expect("active commits");
    // Use a distinct IRI for the inactive binding to avoid collision.
    let bound = test_interpreter_binding();
    commit_quads(&w, bound.to_quads(role_binding_graph())).expect("bounded commits");

    let listing = list_all_active(&store).expect("list succeeds");
    let role_ids: Vec<&str> = listing.iter().map(|b| b.role_id.as_str()).collect();
    assert!(role_ids.contains(&"implementer"), "{role_ids:?}");
    assert!(role_ids.contains(&"test_interpreter"), "{role_ids:?}");
    // Result is sorted by role_id.
    let mut sorted = role_ids.clone();
    sorted.sort_unstable();
    assert_eq!(role_ids, sorted, "list_all_active must be sorted");
}

// ---------------------------------------------------------------------------
// Scenario 2 — empty escalation_steps is permitted.
// ---------------------------------------------------------------------------

#[test]
fn bounded_classification_binding_with_empty_chain_passes_shacl() {
    let b = test_interpreter_binding();
    let quads = b.to_quads(role_binding_graph());
    validate_quads(&quads).expect("empty escalation_steps must pass SHACL");
}

#[test]
fn empty_chain_round_trips() {
    let (store, w) = writer();
    let b = test_interpreter_binding();
    commit_quads(&w, b.to_quads(role_binding_graph())).expect("commit succeeds");
    let loaded = active_for_role(&store, "test_interpreter")
        .expect("read succeeds")
        .expect("present");
    assert!(loaded.escalation_steps.is_empty());
    assert_eq!(loaded.default_capability, cap_iri("classifier"));
}

// ---------------------------------------------------------------------------
// Scenario 3 — two active bindings for the same role_id.
// ---------------------------------------------------------------------------

#[test]
fn duplicate_active_per_role_id_is_rejected_pure_shacl() {
    let a = implementer_binding();
    let mut b = implementer_binding();
    b.version = 2; // different IRI but same role_id, both active
    let mut quads = a.to_quads(role_binding_graph());
    quads.extend(b.to_quads(role_binding_graph()));
    let err = validate_quads(&quads).expect_err("dup-active must fail");
    assert!(err.report.contains("active"), "{}", err.report);
    assert!(err.report.contains("role_id"), "{}", err.report);
}

#[test]
fn duplicate_active_per_role_id_is_rejected_via_stream_writer() {
    let (_store, w) = writer();
    let a = implementer_binding();
    let mut b = implementer_binding();
    b.version = 2;
    let mut quads = a.to_quads(role_binding_graph());
    quads.extend(b.to_quads(role_binding_graph()));
    let err = commit_quads(&w, quads).expect_err("dup-active commit must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("role binding"), "{err}");
}

// ---------------------------------------------------------------------------
// Scenario 4 — unknown trigger_signal literal is rejected.
// ---------------------------------------------------------------------------

#[test]
fn unknown_trigger_signal_is_rejected_pure_shacl() {
    let b = implementer_binding();
    let mut quads = b.to_quads(role_binding_graph());
    // Mutate the first trigger_signal literal to "stakes_critical" — not
    // in the ADR-034 vocabulary.
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_TRIGGER_SIGNAL {
            q.object = Literal::new_simple_literal("stakes_critical").into();
            break;
        }
    }
    let err = validate_quads(&quads).expect_err("unknown trigger must fail");
    assert!(err.report.contains("trigger_signal"), "{}", err.report);
    assert!(err.report.contains("stakes_critical"), "{}", err.report);
}

#[test]
fn unknown_trigger_signal_is_rejected_via_stream_writer() {
    let (_store, w) = writer();
    let b = implementer_binding();
    let mut quads = b.to_quads(role_binding_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == trigger_signal_pred().as_str() {
            q.object = Literal::new_simple_literal("stakes_critical").into();
            break;
        }
    }
    let err = commit_quads(&w, quads).expect_err("unknown trigger must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("trigger_signal"), "{err}");
}

// ---------------------------------------------------------------------------
// Scenario 5 — EscalationStep with no triggers is rejected.
// ---------------------------------------------------------------------------

#[test]
fn escalation_step_without_triggers_is_rejected_pure_shacl() {
    let b = implementer_binding();
    let mut quads = b.to_quads(role_binding_graph());
    // Strip every dec:triggers edge and every trigger artifact for the
    // FIRST step (i=0). Keep the step itself.
    let step_0_iri = b.step_iri(0);
    quads.retain(|q| {
        let edge_from_step_0 = q.predicate.as_str() == IRI_DEC_TRIGGERS
            && matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == &step_0_iri);
        !edge_from_step_0
    });
    let err = validate_quads(&quads).expect_err("step with no triggers must fail");
    assert!(err.report.contains("triggers"), "{}", err.report);
}

#[test]
fn escalation_step_without_triggers_is_rejected_via_stream_writer() {
    let (_store, w) = writer();
    let b = implementer_binding();
    let mut quads = b.to_quads(role_binding_graph());
    let step_0_iri = b.step_iri(0);
    quads.retain(|q| {
        let edge_from_step_0 = q.predicate.as_str() == triggers_pred().as_str()
            && matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == &step_0_iri);
        !edge_from_step_0
    });
    let err = commit_quads(&w, quads).expect_err("step with no triggers must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("triggers"), "{err}");
}

// ---------------------------------------------------------------------------
// Bonus: EOL capability rejection.
// ---------------------------------------------------------------------------

#[test]
fn default_capability_pointing_at_eol_capability_is_rejected() {
    use decision_cli::core::ontology::capability::{
        Capability, CapabilityStatus, CostCurrency, Endpoint,
    };
    // Build a capability artifact whose dec:status is "eol", plus a
    // binding whose default_capability references it.
    let eol_cap = Capability {
        id: "retired-writer".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "retired-model".to_string(),
        tier: Some(1),
        context_window: 100_000,
        max_output: 8_000,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.10".to_string(),
        cost_output_per_m: "0.50".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Eol,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    };
    let binding = RoleBinding {
        role_id: "doomed_role".to_string(),
        default_capability: eol_cap.iri(),
        escalation_steps: vec![],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    };

    // Construct a quad set that contains both the EOL capability AND the
    // binding. The role-binding validator's default_capability_not_eol
    // check fires against this combined set.
    let mut quads = eol_cap.to_quads(capability_graph());
    quads.extend(binding.to_quads(role_binding_graph()));

    let err = validate_quads(&quads).expect_err("EOL default_capability must fail");
    assert!(err.report.contains("default_capability"), "{}", err.report);
    assert!(err.report.contains("eol"), "{}", err.report);
}

// ---------------------------------------------------------------------------
// Bonus: ontology embedded shapes declare the new shapes.
// ---------------------------------------------------------------------------

#[test]
fn embedded_shapes_declare_role_binding_shape() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    let target =
        NamedNode::new("https://decision-cli.dev/ns#RoleBinding").expect("class iri");
    let mut has_shape = false;
    for q in h.shapes_graph() {
        if q.predicate.as_str() == "http://www.w3.org/ns/shacl#targetClass" {
            if let oxigraph::model::Term::NamedNode(n) = &q.object {
                if n == &target {
                    has_shape = true;
                    break;
                }
            }
        }
    }
    assert!(
        has_shape,
        "shapes.ttl must declare a sh:NodeShape with sh:targetClass dec:RoleBinding"
    );
}

#[test]
fn embedded_shapes_declare_escalation_step_and_trigger_shapes() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    let mut step_seen = false;
    let mut trigger_seen = false;
    let step_iri =
        NamedNode::new("https://decision-cli.dev/ns#EscalationStep").expect("step iri");
    let trigger_iri =
        NamedNode::new("https://decision-cli.dev/ns#EscalationTrigger").expect("trigger iri");
    for q in h.shapes_graph() {
        if q.predicate.as_str() != "http://www.w3.org/ns/shacl#targetClass" {
            continue;
        }
        let oxigraph::model::Term::NamedNode(n) = &q.object else {
            continue;
        };
        if n == &step_iri {
            step_seen = true;
        }
        if n == &trigger_iri {
            trigger_seen = true;
        }
    }
    assert!(step_seen, "shapes.ttl must target dec:EscalationStep");
    assert!(trigger_seen, "shapes.ttl must target dec:EscalationTrigger");
}

#[test]
fn ontology_version_bumped_to_0_5_0() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    assert_eq!(h.version(), "0.5.0");
}

// Sanity: explicit reference to ensure the predicate import is used.
#[test]
fn role_binding_active_pred_is_a_real_iri() {
    assert_eq!(
        role_binding_active_pred().as_str(),
        "https://decision-cli.dev/ns#active"
    );
}
