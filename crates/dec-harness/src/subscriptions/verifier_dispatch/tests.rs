//! Unit tests for the verifier-dispatch handler (FT-022).

use std::sync::Arc;

use oxigraph::model::{NamedNode, NamedNodeRef, Term};
use oxigraph::store::Store;

use super::*;
use crate::dispatch::{DispatchEvent, DispatchGroup};
use dec_graph::stream_writer::StreamWriter;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-verifier-dispatch";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn mint_pending_group(writer: &StreamWriter, idx: usize) -> DispatchGroup {
    let group_iri = NamedNode::new(format!("urn:dec:test:vd:group:{idx}")).expect("group iri");
    let action_iri =
        NamedNode::new(format!("urn:dec:test:vd:session/action/{idx}")).expect("action iri");
    let mut group = DispatchGroup::mint(writer, group_iri, action_iri, "FT-022").expect("mint");
    group
        .transition(writer, DispatchEvent::ActionCompleted)
        .expect("action completed");
    group
}

#[test]
fn seed_quads_emit_select_query_and_async_mode() {
    let quads = seed_quads();
    let select_count = quads
        .iter()
        .filter(|q| q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY)
        .count();
    assert_eq!(select_count, 1, "expected exactly one selectQuery quad");

    // Mode is async per FT-022 §Outputs.
    let mode_async = quads.iter().any(|q| {
        q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_MODE
            && matches!(&q.object, Term::Literal(lit) if lit.value() == oxi_events::vocab::SUB_MODE_ASYNC)
    });
    assert!(mode_async, "subscription mode must be async");

    // Handler tag is the stable string the harness binds to.
    let handler = quads.iter().any(|q| {
        q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_HANDLER
            && matches!(&q.object, Term::Literal(lit) if lit.value() == VERIFIER_DISPATCH_HANDLER)
    });
    assert!(handler, "subscription must carry oxi:handler tag");
}

#[test]
fn seed_ttl_and_runtime_query_agree() {
    // The runtime handler does not parse the TTL — it uses
    // PENDING_GROUPS_QUERY directly. Assert the persisted seed contains
    // the same SPARQL body so a refactor that touches one and not the
    // other fails loudly.
    let ttl = VERIFIER_DISPATCH_SEED_TTL;
    for token in [
        "dec:DispatchGroup",
        "dec:dispatchStatus",
        "awaiting-interpretation",
        "dec:hasActionSession",
        "FILTER NOT EXISTS",
        "dec:hasInterpretationSession",
    ] {
        assert!(
            ttl.contains(token),
            "seed TTL missing token {token}; runtime query and seed have drifted"
        );
        assert!(
            PENDING_GROUPS_QUERY.contains(token),
            "runtime query missing token {token}"
        );
    }
}

#[test]
fn pending_groups_finds_awaiting_interpretation_groups() {
    let (store, w) = writer();
    let _g1 = mint_pending_group(&w, 1);
    let _g2 = mint_pending_group(&w, 2);
    let pending = dispatch_pending_groups(&store).expect("query");
    assert_eq!(pending.len(), 2);
}

#[test]
fn pending_groups_excludes_already_paired_groups() {
    let (store, w) = writer();
    let mut g = mint_pending_group(&w, 7);
    let interp = NamedNode::new("urn:dec:test:vd:session/interp/7").expect("interp iri");
    g.attach_interpretation_session(&w, interp).expect("attach");
    let pending = dispatch_pending_groups(&store).expect("query");
    assert!(
        pending.is_empty(),
        "paired group must not appear in pending list, got {pending:?}"
    );
}

#[test]
fn pending_groups_excludes_action_failed_groups() {
    let (store, w) = writer();
    let group_iri = NamedNode::new("urn:dec:test:vd:group:failed").expect("group iri");
    let action_iri = NamedNode::new("urn:dec:test:vd:session/action/failed").expect("action iri");
    let mut group = DispatchGroup::mint(&w, group_iri, action_iri, "FT-022").expect("mint");
    group
        .transition(&w, DispatchEvent::ActionFailed)
        .expect("action failed");
    let pending = dispatch_pending_groups(&store).expect("query");
    assert!(
        pending.is_empty(),
        "action-failed group must not appear in pending list"
    );
}

#[test]
fn emit_event_is_idempotent_for_same_group() {
    let (store, w) = writer();
    let group = mint_pending_group(&w, 11);
    let seed = VerifierDispatchSeed {
        group: group.iri.clone(),
        action_session: group.action_session.clone(),
    };
    let first = emit_verifier_dispatch_event(&w, &store, &seed, "2026-05-20T09:16:16Z")
        .expect("emit ok")
        .expect("first emission");
    assert_eq!(first.group, seed.group);
    // Second emission for the same group is a no-op.
    let second =
        emit_verifier_dispatch_event(&w, &store, &seed, "2026-05-20T09:16:20Z").expect("emit ok");
    assert!(second.is_none(), "second emission must be suppressed");
    assert!(
        already_dispatched(&store, &seed.group).expect("ask"),
        "already_dispatched must report true after first emission"
    );
}

#[test]
fn emit_event_writes_required_predicates() {
    let (store, w) = writer();
    let group = mint_pending_group(&w, 13);
    let seed = VerifierDispatchSeed {
        group: group.iri.clone(),
        action_session: group.action_session.clone(),
    };
    let ev = emit_verifier_dispatch_event(&w, &store, &seed, "2026-05-20T09:16:16Z")
        .expect("emit ok")
        .expect("first emission");
    // Required predicates per FT-022 §Outputs.
    for (pred, expect_value) in [
        (super::IRI_DEC_EVENT, None),
        (
            dec_ontology::vocab::IRI_DEC_EVENT_CLASS,
            Some(EVENT_CLASS_VERIFIER_DISPATCH.to_string()),
        ),
        (
            dec_ontology::vocab::IRI_DEC_TARGET_ROLE,
            Some(VERIFIER_TARGET_ROLE.to_string()),
        ),
    ] {
        if let Some(expected) = expect_value {
            let found = store
                .quads_for_pattern(
                    Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
                    Some(NamedNodeRef::new_unchecked(pred)),
                    None,
                    None,
                )
                .filter_map(Result::ok)
                .any(|q| matches!(&q.object, Term::Literal(lit) if lit.value() == expected));
            assert!(found, "expected literal {expected} on predicate {pred}");
        } else {
            // For rdf:type dec:Event — assert it's present.
            let typed = store
                .quads_for_pattern(
                    Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
                    Some(NamedNodeRef::new_unchecked(super::RDF_TYPE)),
                    Some(NamedNodeRef::new_unchecked(pred).into()),
                    None,
                )
                .filter_map(Result::ok)
                .count();
            assert!(typed > 0, "expected rdf:type {pred} on event");
        }
    }
    // dec:dispatchGroup and dec:bundleSeed must reference the seed.
    for (pred, expected) in [
        (dec_ontology::vocab::IRI_DEC_DISPATCH_GROUP_REF, &seed.group),
        (
            dec_ontology::vocab::IRI_DEC_BUNDLE_SEED,
            &seed.action_session,
        ),
    ] {
        let found = store
            .quads_for_pattern(
                Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
                Some(NamedNodeRef::new_unchecked(pred)),
                None,
                None,
            )
            .filter_map(Result::ok)
            .any(|q| matches!(&q.object, Term::NamedNode(n) if n == expected));
        assert!(found, "expected predicate {pred} pointing to {expected}");
    }

    // ADR-005: the event must carry dec:inStream.
    let stream_iri = NamedNode::new(STREAM_IRI).expect("stream iri");
    let in_stream_present = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
            Some(NamedNodeRef::new_unchecked(
                dec_ontology::vocab::IRI_DEC_IN_STREAM,
            )),
            None,
            None,
        )
        .filter_map(Result::ok)
        .any(|q| matches!(&q.object, Term::NamedNode(n) if n == &stream_iri));
    assert!(in_stream_present, "event must carry dec:inStream (ADR-005)");
}
