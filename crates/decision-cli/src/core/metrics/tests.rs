//! Unit tests for [`super::agreement`] — counts and rate derivation.

use chrono::{TimeZone, Utc};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use super::agreement::{agreement, format_report, AgreementReport, MetricsError};
use crate::core::dispatch::DispatchStatus;
use crate::core::vocab::{
    dispatch_group_class, dispatch_status, dispatched_for, has_action_session,
    has_interpretation_session, orchestration_graph, verdict as verdict_pred,
    verification_verdict_class, IRI_DEC_IN_STREAM,
};

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
const PROV_USED: &str = "http://www.w3.org/ns/prov#used";
const DEC_ROLE: &str = "https://decision-cli.dev/ns#role";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";

/// Mint one synthetic terminal dispatch group:
/// - DispatchGroup with the given status
/// - linked action session (role = "implementer", optional `dec:status`)
/// - if a verdict is supplied, an InterpretationSession + VerificationVerdict
fn seed_group(
    store: &Store,
    idx: u32,
    status: DispatchStatus,
    action_status: Option<&str>,
    verdict_value: Option<&str>,
    started_at: &str,
    role: &str,
) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let group = NamedNode::new(format!("urn:dec:test:group:{idx}")).expect("group iri");
    let action = NamedNode::new(format!("urn:dec:test:action:{idx}")).expect("action iri");
    let interp = NamedNode::new(format!("urn:dec:test:interp:{idx}")).expect("interp iri");
    let verdict_iri =
        NamedNode::new(format!("urn:dec:test:verdict:{idx}")).expect("verdict iri");
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let in_stream = NamedNodeRef::new_unchecked(IRI_DEC_IN_STREAM);
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE);
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS);
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME);
    let was_gen = NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY);
    let used = NamedNodeRef::new_unchecked(PROV_USED);
    let stream_node = NamedNode::new(STREAM_IRI).expect("stream iri");

    let mut quads = vec![
        // DispatchGroup
        Quad::new(group.clone(), rdf_type, dispatch_group_class(), g.clone()),
        Quad::new(
            group.clone(),
            in_stream,
            stream_node.clone(),
            g.clone(),
        ),
        Quad::new(
            group.clone(),
            dispatch_status(),
            Literal::new_simple_literal(status.as_str()),
            g.clone(),
        ),
        Quad::new(
            group.clone(),
            dispatched_for(),
            Literal::new_simple_literal(format!("FT-test-{idx}")),
            g.clone(),
        ),
        Quad::new(group.clone(), has_action_session(), action.clone(), g.clone()),
        // ActionSession
        Quad::new(
            action.clone(),
            role_pred,
            Literal::new_simple_literal(role),
            g.clone(),
        ),
        Quad::new(
            action.clone(),
            at_time,
            Literal::new_typed_literal(
                started_at,
                NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            ),
            g.clone(),
        ),
    ];
    if let Some(s) = action_status {
        quads.push(Quad::new(
            action.clone(),
            status_pred,
            Literal::new_simple_literal(s),
            g.clone(),
        ));
    }
    if let Some(v) = verdict_value {
        quads.push(Quad::new(
            group.clone(),
            has_interpretation_session(),
            interp.clone(),
            g.clone(),
        ));
        quads.push(Quad::new(
            verdict_iri.clone(),
            rdf_type,
            verification_verdict_class(),
            g.clone(),
        ));
        quads.push(Quad::new(
            verdict_iri.clone(),
            was_gen,
            interp.clone(),
            g.clone(),
        ));
        quads.push(Quad::new(verdict_iri.clone(), used, action, g.clone()));
        quads.push(Quad::new(
            verdict_iri,
            verdict_pred(),
            Literal::new_simple_literal(v),
            g,
        ));
    }

    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("seed group");
}

fn fresh_store() -> Store {
    Store::new().expect("in-memory store")
}

#[test]
fn empty_store_returns_zero_report() {
    let store = fresh_store();
    let r = agreement(&store, None, None).expect("agreement OK on empty store");
    assert_eq!(r.total_terminal_groups, 0);
    assert_eq!(r.total_action_success, 0);
    assert_eq!(r.approved, 0);
    assert_eq!(r.amendment_required, 0);
    assert_eq!(r.rejected, 0);
    assert_eq!(r.agreement_rate, 0.0);
    assert_eq!(r.amendment_rate, 0.0);
    assert_eq!(r.rejection_rate, 0.0);
    assert_eq!(r.false_success_rate, 0.0);
    assert!(r.window.is_none());
    assert!(r.role_filter.is_none());
}

#[test]
fn approved_group_contributes_to_agreement_rate() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::Complete,
        Some("complete"),
        Some("approved"),
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 1);
    assert_eq!(r.total_action_success, 1);
    assert_eq!(r.approved, 1);
    assert_eq!(r.amendment_required, 0);
    assert_eq!(r.rejected, 0);
    assert!((r.agreement_rate - 1.0).abs() < 1e-9);
    assert_eq!(r.false_success_rate, 0.0);
    assert_eq!(r.amendment_rate, 0.0);
    assert_eq!(r.rejection_rate, 0.0);
}

#[test]
fn rejected_group_drives_rejection_and_false_success() {
    let store = fresh_store();
    // Action thought it succeeded; verifier rejected.
    seed_group(
        &store,
        1,
        DispatchStatus::InterpretationRejected,
        Some("complete"),
        Some("rejected"),
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 1);
    assert_eq!(r.total_action_success, 1);
    assert_eq!(r.rejected, 1);
    assert!((r.rejection_rate - 1.0).abs() < 1e-9);
    assert!((r.false_success_rate - 1.0).abs() < 1e-9);
    assert_eq!(r.agreement_rate, 0.0);
}

#[test]
fn amendment_group_drives_amendment_and_false_success() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::AwaitingAmendment,
        Some("complete"),
        Some("amendment-required"),
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_action_success, 1);
    assert_eq!(r.amendment_required, 1);
    assert!((r.amendment_rate - 1.0).abs() < 1e-9);
    assert!((r.false_success_rate - 1.0).abs() < 1e-9);
}

#[test]
fn action_failed_group_in_terminal_but_not_in_action_success() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::ActionFailed,
        Some("failed: worker crashed"),
        None,
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 1);
    assert_eq!(r.total_action_success, 0);
    // Rates have zero denominator → must be 0.0.
    assert_eq!(r.agreement_rate, 0.0);
    assert_eq!(r.false_success_rate, 0.0);
}

#[test]
fn mixed_population_produces_expected_rates() {
    let store = fresh_store();
    // 3 approved, 1 rejected, 1 amendment, 1 action-failed.
    for (i, (status, verdict)) in [
        (DispatchStatus::Complete, Some("approved")),
        (DispatchStatus::Complete, Some("approved")),
        (DispatchStatus::Complete, Some("approved")),
        (DispatchStatus::InterpretationRejected, Some("rejected")),
        (DispatchStatus::AwaitingAmendment, Some("amendment-required")),
    ]
    .iter()
    .enumerate()
    {
        let action_status = "complete";
        seed_group(
            &store,
            i as u32 + 1,
            *status,
            Some(action_status),
            *verdict,
            "2026-05-01T10:00:00Z",
            "implementer",
        );
    }
    seed_group(
        &store,
        99,
        DispatchStatus::ActionFailed,
        Some("failed"),
        None,
        "2026-05-01T10:00:00Z",
        "implementer",
    );

    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 6);
    assert_eq!(r.total_action_success, 5);
    assert_eq!(r.approved, 3);
    assert_eq!(r.amendment_required, 1);
    assert_eq!(r.rejected, 1);
    // 3 / 5 = 0.60
    assert!((r.agreement_rate - 0.6).abs() < 1e-9);
    // 1 / 5 = 0.20
    assert!((r.amendment_rate - 0.2).abs() < 1e-9);
    assert!((r.rejection_rate - 0.2).abs() < 1e-9);
    // (1 + 1) / 5 = 0.40
    assert!((r.false_success_rate - 0.4).abs() < 1e-9);
}

#[test]
fn awaiting_action_groups_are_ignored() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::AwaitingAction,
        None,
        None,
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    let r = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 0);
    assert_eq!(r.total_action_success, 0);
}

#[test]
fn role_filter_restricts_population() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::Complete,
        Some("complete"),
        Some("approved"),
        "2026-05-01T10:00:00Z",
        "implementer",
    );
    seed_group(
        &store,
        2,
        DispatchStatus::Complete,
        Some("complete"),
        Some("approved"),
        "2026-05-01T10:00:00Z",
        "other-role",
    );
    let r = agreement(&store, None, Some("implementer")).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 1);
    assert_eq!(r.total_action_success, 1);
    assert_eq!(r.approved, 1);
}

#[test]
fn unknown_role_is_an_error() {
    let store = fresh_store();
    let err = agreement(&store, None, Some("no-such-role"))
        .expect_err("unknown role must be rejected");
    assert!(matches!(err, MetricsError::UnknownRole { .. }));
}

#[test]
fn invalid_window_is_an_error() {
    let store = fresh_store();
    let since = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
    let until = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let err = agreement(&store, Some((since, until)), None)
        .expect_err("backwards window must be rejected");
    assert!(matches!(err, MetricsError::InvalidWindow { .. }));
}

#[test]
fn window_filters_groups_by_action_start_time() {
    let store = fresh_store();
    seed_group(
        &store,
        1,
        DispatchStatus::Complete,
        Some("complete"),
        Some("approved"),
        "2026-04-01T10:00:00Z",
        "implementer",
    );
    seed_group(
        &store,
        2,
        DispatchStatus::Complete,
        Some("complete"),
        Some("approved"),
        "2026-05-15T10:00:00Z",
        "implementer",
    );
    let since = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let until = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    let r = agreement(&store, Some((since, until)), None).expect("agreement ok");
    assert_eq!(r.total_terminal_groups, 1, "only the May group is in window");
    assert_eq!(r.window, Some((since, until)));
}

#[test]
fn format_report_renders_five_rate_lines() {
    let report = AgreementReport {
        total_terminal_groups: 5,
        total_action_success: 4,
        approved: 2,
        amendment_required: 1,
        rejected: 1,
        agreement_rate: 0.5,
        amendment_rate: 0.25,
        rejection_rate: 0.25,
        false_success_rate: 0.5,
        window: None,
        role_filter: None,
    };
    let rendered = format_report(&report);
    assert!(rendered.contains("Agreement rate"));
    assert!(rendered.contains("Amendment rate"));
    assert!(rendered.contains("Rejection rate"));
    assert!(rendered.contains("False-success rate"));
    assert!(rendered.contains("50.00%"));
}
