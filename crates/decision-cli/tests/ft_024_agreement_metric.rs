//! FT-024 — Action-interpretation agreement metric over a persisted store.
//!
//! Validates the slice-2 exit criteria for TC-032: a freshly-initialised
//! orchestration store starts with the zero-data marker (no dispatches,
//! all rates 0.0, `total_terminal_groups = 0`), and a fully-realised
//! mixed population produces the four ADR-021 rates the way the spec
//! prescribes.
//!
//! Mirrors the verdict_shacl integration test in shape: an in-memory
//! oxigraph store seeded by hand with the same RDF shape `dec implement`
//! and the verifier worker would produce in production.

use chrono::Utc;
use decision_cli::core::dispatch::DispatchStatus;
use decision_cli::core::metrics::{agreement, format_report};
use decision_cli::core::vocab::{
    dispatch_group_class, dispatch_status, dispatched_for, has_action_session,
    has_interpretation_session, orchestration_graph, verdict as verdict_pred,
    verification_verdict_class, IRI_DEC_IN_STREAM,
};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
const PROV_USED: &str = "http://www.w3.org/ns/prov#used";
const DEC_ROLE: &str = "https://decision-cli.dev/ns#role";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";

#[test]
fn empty_store_yields_zero_data_marker() {
    let store = Store::new().expect("in-memory store");
    let report = agreement(&store, None, None).expect("agreement runs on empty store");
    assert_eq!(report.total_terminal_groups, 0);
    assert_eq!(report.total_action_success, 0);
    assert_eq!(report.agreement_rate, 0.0);
    assert_eq!(report.amendment_rate, 0.0);
    assert_eq!(report.rejection_rate, 0.0);
    assert_eq!(report.false_success_rate, 0.0);
}

#[test]
fn populated_store_produces_adr_021_rates() {
    let store = Store::new().expect("in-memory store");
    // Population: 2 approved, 1 rejected, 1 amendment, 1 action-failed.
    seed(&store, 1, DispatchStatus::Complete, "complete", Some("approved"));
    seed(&store, 2, DispatchStatus::Complete, "complete", Some("approved"));
    seed(
        &store,
        3,
        DispatchStatus::InterpretationRejected,
        "complete",
        Some("rejected"),
    );
    seed(
        &store,
        4,
        DispatchStatus::AwaitingAmendment,
        "complete",
        Some("amendment-required"),
    );
    seed(&store, 5, DispatchStatus::ActionFailed, "failed", None);

    let report = agreement(&store, None, None).expect("agreement ok");
    assert_eq!(report.total_terminal_groups, 5);
    assert_eq!(report.total_action_success, 4);
    assert_eq!(report.approved, 2);
    assert_eq!(report.amendment_required, 1);
    assert_eq!(report.rejected, 1);
    // |A ∩ approved| / |A| = 2/4 = 0.5
    assert!((report.agreement_rate - 0.5).abs() < 1e-9);
    // |amendment| / |A| = 1/4 = 0.25
    assert!((report.amendment_rate - 0.25).abs() < 1e-9);
    // |rejected| / |A| = 1/4 = 0.25
    assert!((report.rejection_rate - 0.25).abs() < 1e-9);
    // |A ∩ (rejected ∪ amendment)| / |A| = 2/4 = 0.5
    assert!((report.false_success_rate - 0.5).abs() < 1e-9);

    // format_report() emits all four rate rows in human-readable form.
    let rendered = format_report(&report);
    for label in [
        "Agreement rate",
        "Amendment rate",
        "Rejection rate",
        "False-success rate",
    ] {
        assert!(rendered.contains(label), "missing row: {label}");
    }
}

#[test]
fn implementer_role_filter_passes_through() {
    let store = Store::new().expect("in-memory store");
    seed(&store, 1, DispatchStatus::Complete, "complete", Some("approved"));
    // The implementer role is the slice-1 hardcoded surface; the
    // metric must accept it without requiring a graph-resident catalog
    // entry (FT-030 lands that in slice 3).
    let report = agreement(&store, None, Some("implementer")).expect("implementer filter ok");
    assert_eq!(report.total_terminal_groups, 1);
    assert_eq!(report.role_filter.as_deref(), Some("implementer"));
}

#[test]
fn window_argument_is_threaded_through_to_the_report() {
    let store = Store::new().expect("in-memory store");
    seed(&store, 1, DispatchStatus::Complete, "complete", Some("approved"));
    let since = Utc::now() - chrono::Duration::days(1);
    let until = Utc::now() + chrono::Duration::days(1);
    let report = agreement(&store, Some((since, until)), None).expect("windowed ok");
    assert_eq!(report.window, Some((since, until)));
}

/// Seed a synthetic terminal `DispatchGroup` complete with action
/// session, optional interpretation session + verdict, and the
/// `dec:inStream` link required by ADR-005.
fn seed(
    store: &Store,
    idx: u32,
    status: DispatchStatus,
    action_status: &str,
    verdict_value: Option<&str>,
) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let group = NamedNode::new(format!("urn:dec:test:ft-024:group:{idx}")).expect("group iri");
    let action = NamedNode::new(format!("urn:dec:test:ft-024:action:{idx}")).expect("action iri");
    let interp = NamedNode::new(format!("urn:dec:test:ft-024:interp:{idx}")).expect("interp iri");
    let verdict_iri =
        NamedNode::new(format!("urn:dec:test:ft-024:verdict:{idx}")).expect("verdict iri");
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let in_stream = NamedNodeRef::new_unchecked(IRI_DEC_IN_STREAM);
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE);
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS);
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME);
    let was_gen = NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY);
    let used = NamedNodeRef::new_unchecked(PROV_USED);

    let mut quads = vec![
        Quad::new(group.clone(), rdf_type, dispatch_group_class(), g.clone()),
        Quad::new(group.clone(), in_stream, stream, g.clone()),
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
        Quad::new(
            action.clone(),
            role_pred,
            Literal::new_simple_literal("implementer"),
            g.clone(),
        ),
        Quad::new(
            action.clone(),
            status_pred,
            Literal::new_simple_literal(action_status),
            g.clone(),
        ),
        Quad::new(
            action.clone(),
            at_time,
            Literal::new_typed_literal(
                "2026-05-01T10:00:00Z",
                NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            ),
            g.clone(),
        ),
    ];
    if let Some(v) = verdict_value {
        quads.push(Quad::new(
            group,
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
            interp,
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
