//! TC-239 — Approved VGR closes prior open defects from same graph against same passing TC.

use chrono::Utc;
use oxigraph::model::{NamedNode, NamedNodeRef, Subject};
use oxigraph::store::Store;
use std::sync::Arc;

use decision_cli::core::feedback::artifact::{Feedback, Severity};
use decision_cli::core::feedback::lifecycle::LifecycleState;
use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::{
    EvidenceProjection, StepOutcome, VerificationGraphResult,
};
use decision_cli::core::vocab::{lifecycle_state as lifecycle_state_pred, orchestration_graph};
use decision_cli::core::StreamWriter;
use decision_cli::features::ft_116_retract_stale_defects::retract_stale_defects_in_transaction;

#[test]
fn tc_239_approved_vgr_closes_prior_open_defects() {
    let store = Store::new().unwrap();
    let store = Arc::new(store);

    let graph_iri = "https://decision-cli.dev/ns/verify/graph/VG-100";
    let tc_iri = "https://decision-cli.dev/ns/test/TC-200";
    let vgr_1_iri = "https://decision-cli.dev/ns/result/VGR-500";
    let vgr_2_iri = "https://decision-cli.dev/ns/result/VGR-501";
    let fb_iri = "https://decision-cli.dev/ns/feedback/fb-abc";
    let session_iri = "https://decision-cli.dev/ns/session/sess-1";

    // Seed VGR-1 (failing)
    let vgr_1 = VerificationGraphResult {
        id: vgr_1_iri.to_string(),
        result_of: graph_iri.to_string(),
        ran_in_environment: "https://decision-cli.dev/ns/env/ENV-001".to_string(),
        verdict: Verdict::Rejected,
        started_at: Utc::now().to_rfc3339(),
        ended_at: Utc::now().to_rfc3339(),
        step_traces: vec![],
        evidence_for: vec![EvidenceProjection {
            tc: tc_iri.to_string(),
            outcome: StepOutcome::Fail,
            from_step: format!("{vgr_1_iri}/step/0"),
        }],
        rationale: "TC failed".to_string(),
        was_generated_by: session_iri.to_string(),
        was_attributed_to: "https://decision-cli.dev/ns/agent/runner".to_string(),
        created_at: Utc::now().to_rfc3339(),
    };

    let quads = vgr_1.to_quads(NamedNodeRef::new_unchecked(
        "https://decision-cli.dev/ns/graph/verify-result",
    ));
    for quad in quads {
        store.insert(&quad).unwrap();
    }

    // Seed feedback fb-1
    let feedback = Feedback {
        iri: NamedNode::new_unchecked(fb_iri),
        class: "defect".to_string(),
        severity: Severity::Error,
        target_role: "implementer".to_string(),
        evidence: "TC-200 failed".to_string(),
        recommendation: None,
        lifecycle_state: LifecycleState::Produced.as_str().to_string(),
        source_session: NamedNode::new_unchecked(session_iri),
        source_artifact: Some(NamedNode::new_unchecked(tc_iri)),
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: NamedNode::new_unchecked("https://decision-cli.dev/ns/stream/default"),
    };

    let fb_quads = feedback.to_quads(orchestration_graph());
    for quad in fb_quads {
        store.insert(&quad).unwrap();
    }

    // Create VGR-2 (passing)
    let vgr_2 = VerificationGraphResult {
        id: vgr_2_iri.to_string(),
        result_of: graph_iri.to_string(),
        ran_in_environment: "https://decision-cli.dev/ns/env/ENV-001".to_string(),
        verdict: Verdict::Approved,
        started_at: Utc::now().to_rfc3339(),
        ended_at: Utc::now().to_rfc3339(),
        step_traces: vec![],
        evidence_for: vec![EvidenceProjection {
            tc: tc_iri.to_string(),
            outcome: StepOutcome::Pass,
            from_step: format!("{vgr_2_iri}/step/0"),
        }],
        rationale: "All TCs passed".to_string(),
        was_generated_by: "https://decision-cli.dev/ns/session/sess-2".to_string(),
        was_attributed_to: "https://decision-cli.dev/ns/agent/runner".to_string(),
        created_at: Utc::now().to_rfc3339(),
    };

    let vgr_2_quads = vgr_2.to_quads(NamedNodeRef::new_unchecked(
        "https://decision-cli.dev/ns/graph/verify-result",
    ));
    for quad in vgr_2_quads {
        store.insert(&quad).unwrap();
    }

    // Open a writer
    let stream_iri = NamedNode::new("https://decision-cli.dev/ns/stream/default").unwrap();
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).unwrap();

    // Invoke auto-close
    let closed_count = retract_stale_defects_in_transaction(&store, &writer, &vgr_2).unwrap();

    // Assertions
    assert_eq!(closed_count, 1, "should close 1 defect");

    // Read lifecycle state
    let fb = NamedNode::new_unchecked(fb_iri);
    let pred = lifecycle_state_pred();
    let mut state = None;
    for quad in store.quads_for_pattern(
        Some(Subject::NamedNode(fb).as_ref()),
        Some(pred),
        None,
        None,
    ) {
        if let Ok(q) = quad {
            if let oxigraph::model::Term::Literal(lit) = q.object {
                state = Some(lit.value().to_string());
                break;
            }
        }
    }

    assert_eq!(
        state,
        Some("closed".to_string()),
        "feedback should be closed"
    );
}
