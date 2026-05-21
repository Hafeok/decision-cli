//! Unit tests for the verify-graph-author auto-dispatch handler (FT-050).

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::store::Store;

use super::*;
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    feature_ref, IRI_DEC_ENVIRONMENT, IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER,
    IRI_DEC_VERIFY_GRAPH_AUTHOR_DISPATCH_EVENT,
};

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-auto-dispatch";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn seed(feature: &str, env: &str) -> AutoDispatchSeed {
    AutoDispatchSeed {
        feature: feature.to_string(),
        env: env.to_string(),
        triggered_by_event_id: format!("urn:dec:event/feature-create/{feature}"),
        bundle_hash: format!("hash-for-{feature}-{env}"),
    }
}

#[test]
fn seed_quads_emit_select_query_and_async_mode() {
    let quads = seed_quads();
    let select_count = quads
        .iter()
        .filter(|q| q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY)
        .count();
    assert_eq!(select_count, 1, "expected exactly one selectQuery quad");

    let mode_async = quads.iter().any(|q| {
        q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_MODE
            && matches!(&q.object, Term::Literal(lit) if lit.value() == oxi_events::vocab::SUB_MODE_ASYNC)
    });
    assert!(mode_async, "subscription mode must be async");

    let handler = quads.iter().any(|q| {
        q.predicate.as_str() == oxi_events::vocab::IRI_OXI_SUB_HANDLER
            && matches!(&q.object, Term::Literal(lit) if lit.value() == VERIFY_GRAPH_AUTHOR_DISPATCH_HANDLER)
    });
    assert!(handler, "subscription must carry oxi:handler tag");
}

#[test]
fn dispatch_event_carries_required_predicates() {
    let (store, w) = writer();
    let s = seed("FT-A", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let ev = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");

    // Feature literal present.
    let feature_lit = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
            Some(feature_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
        .any(|q| matches!(&q.object, Term::Literal(lit) if lit.value() == "FT-A"));
    assert!(feature_lit, "expected dec:feature 'FT-A'");

    // Env literal present.
    let env_lit = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
            Some(NamedNodeRef::new_unchecked(IRI_DEC_ENVIRONMENT)),
            None,
            None,
        )
        .filter_map(Result::ok)
        .any(|q| matches!(&q.object, Term::Literal(lit) if lit.value() == "ENV-1"));
    assert!(env_lit, "expected dec:environment 'ENV-1'");

    // Class typing present.
    let typed = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
            Some(NamedNodeRef::new_unchecked(
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            )),
            Some(NamedNodeRef::new_unchecked(IRI_DEC_VERIFY_GRAPH_AUTHOR_DISPATCH_EVENT).into()),
            None,
        )
        .filter_map(Result::ok)
        .count();
    assert!(typed > 0, "event must be typed as dec:VerifyGraphAuthorDispatchEvent");

    // ADR-005: dec:inStream must be present (event is a scoped class).
    let stream_iri = NamedNode::new(STREAM_IRI).expect("stream iri");
    let in_stream = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ev.iri.clone()).as_ref()),
            Some(NamedNodeRef::new_unchecked(
                crate::core::vocab::IRI_DEC_IN_STREAM,
            )),
            None,
            None,
        )
        .filter_map(Result::ok)
        .any(|q| matches!(&q.object, Term::NamedNode(n) if n == &stream_iri));
    assert!(in_stream, "event must carry dec:inStream (ADR-005)");
}

#[test]
fn dedup_ttl_suppresses_repeat_emissions_within_window() {
    let (store, w) = writer();
    let s = seed("FT-K", "ENV-1");
    let cfg = AutoDispatchConfig::default();

    let first = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");
    assert_eq!(first.feature, "FT-K");

    // Within the TTL — second emission must be suppressed.
    let second = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:15:00Z")
        .expect("emit ok");
    assert!(second.is_none(), "second emission within TTL must be suppressed");
}

#[test]
fn dedup_ttl_zero_disables_dedup() {
    let (store, w) = writer();
    let s = seed("FT-Z", "ENV-1");
    let cfg = AutoDispatchConfig::default().with_ttl(0);

    let first = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");
    assert_eq!(first.feature, "FT-Z");

    let second = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:01Z")
        .expect("emit ok");
    assert!(second.is_some(), "ttl=0 must let repeat emissions through");
}

#[test]
fn ledger_records_after_emission() {
    let (store, w) = writer();
    let s = seed("FT-L", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let _ = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");

    let entry = ledger::get_entry(&store, "FT-L", "ENV-1")
        .expect("ledger query")
        .expect("ledger row present");
    assert_eq!(entry.feature, "FT-L");
    assert_eq!(entry.env, "ENV-1");
    assert_eq!(entry.last_dispatch_at, "2026-05-21T10:00:00Z");
}

#[test]
fn aged_ledger_allows_fresh_dispatch() {
    let (store, w) = writer();
    let s = seed("FT-M", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let _ = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");

    // Age the ledger past the TTL (default 3600s = 1 hour).
    ledger::debug_set_timestamp(&store, "FT-M", "ENV-1", "2026-05-21T08:00:00Z")
        .expect("aging the ledger");

    let now = Utc.with_ymd_and_hms(2026, 5, 21, 10, 0, 0).unwrap();
    let within = within_dedup_window(&store, "FT-M", "ENV-1", &cfg, now).expect("ttl query");
    assert!(!within, "aged ledger entry must fall outside the TTL window");

    let second = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok");
    assert!(second.is_some(), "aged ledger must allow a fresh dispatch");
}

#[test]
fn pending_review_session_writes_required_predicates() {
    let (store, w) = writer();
    let s = seed("FT-J", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let ev = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");

    let session = session::persist_pending_review_session(
        &w,
        &PendingReviewInput {
            feature: "FT-J",
            env: "ENV-1",
            proposal_document_json: "{\"kind\":\"new\"}",
            dispatch_event_iri: &ev.iri,
            started_at: "2026-05-21T10:00:01Z",
        },
    )
    .expect("session ok");

    // status = pending_review
    let pending = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(session.iri.clone()).as_ref()),
            Some(NamedNodeRef::new_unchecked(
                "https://decision-cli.dev/ns#status",
            )),
            None,
            None,
        )
        .filter_map(Result::ok)
        .any(|q| matches!(&q.object, Term::Literal(lit) if lit.value() == "pending_review"));
    assert!(pending, "session must carry status=pending_review");

    // dec:proposalDocument = JSON
    let doc = session::load_proposal_document(&store, &session.iri)
        .expect("doc query")
        .expect("doc present");
    assert_eq!(doc, "{\"kind\":\"new\"}");
}

#[test]
fn ledger_lives_in_dedicated_named_graph() {
    let (store, w) = writer();
    let s = seed("FT-N", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let _ = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("first emission");

    let ledger_quads: Vec<Quad> = store
        .quads_for_pattern(
            None,
            None,
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER),
            )),
        )
        .filter_map(Result::ok)
        .collect();
    assert!(
        !ledger_quads.is_empty(),
        "ledger named graph must carry at least one entry"
    );
}

#[test]
fn config_parser_returns_none_when_section_missing() {
    let body = "[other]\nfoo = 1\n";
    assert!(config::parse_from_str(body).is_none());
}

#[test]
fn config_default_envs_uses_wildcard() {
    let d = AutoDispatchConfig::default();
    assert!(d.envs_use_wildcard());
}

// A test that proves the ledger graph IRI is the constant string we
// documented in `core::vocab`.
#[test]
fn ledger_graph_iri_constant_stable() {
    assert_eq!(
        ledger_graph_iri_str(),
        "https://decision-cli.dev/ns/graph/auto-dispatch-ledger"
    );
    let n = ledger_graph_iri();
    assert_eq!(n.as_str(), ledger_graph_iri_str());
}

// Surface check: emit + record happen atomically — a ledger row exists
// only when an event was actually emitted.
#[test]
fn unemitted_pair_has_no_ledger_row() {
    let (store, _w) = writer();
    let entry = ledger::get_entry(&store, "FT-NOT-EMITTED", "ENV-1").expect("query");
    assert!(entry.is_none(), "ledger row should not exist before emission");
}

// Quick smoke that emit + dedup persistence is consistent under multiple
// distinct (feature, env) pairs (TC-083's "per-env independence").
#[test]
fn distinct_feature_env_pairs_emit_independently() {
    let (store, w) = writer();
    let cfg = AutoDispatchConfig::default();

    let s_a1 = seed("FT-X", "ENV-1");
    let s_a2 = seed("FT-X", "ENV-2");
    let s_b1 = seed("FT-Y", "ENV-1");

    let e1 = emit_dispatch_event(&w, &store, &s_a1, &cfg, "2026-05-21T10:00:00Z")
        .expect("ok")
        .expect("emitted");
    let e2 = emit_dispatch_event(&w, &store, &s_a2, &cfg, "2026-05-21T10:00:01Z")
        .expect("ok")
        .expect("emitted");
    let e3 = emit_dispatch_event(&w, &store, &s_b1, &cfg, "2026-05-21T10:00:02Z")
        .expect("ok")
        .expect("emitted");

    assert_ne!(e1.iri, e2.iri);
    assert_ne!(e1.iri, e3.iri);
    assert_ne!(e2.iri, e3.iri);
    assert_eq!(e1.env, "ENV-1");
    assert_eq!(e2.env, "ENV-2");
    assert_eq!(e3.env, "ENV-1");
}

// Test against a literal we wouldn't accept in real config — just a sanity check
// on the helper.
#[test]
fn within_dedup_window_with_no_entry_returns_false() {
    let (store, _w) = writer();
    let now = Utc.with_ymd_and_hms(2026, 5, 21, 10, 0, 0).unwrap();
    let cfg = AutoDispatchConfig::default();
    let within = within_dedup_window(&store, "FT-NEVER", "ENV-1", &cfg, now).expect("ok");
    assert!(!within);
}

// The auto-dispatch ledger graph name is reachable via the vocab module
// — keep this test so renaming the constant fails loudly.
#[test]
fn ledger_graph_iri_is_in_vocab_module() {
    use crate::core::vocab::auto_dispatch_ledger_graph;
    let g: GraphName = auto_dispatch_ledger_graph().into_owned().into();
    let GraphName::NamedNode(name) = g else {
        panic!("expected named graph");
    };
    assert_eq!(name.as_str(), IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER);
}

// Ensure ledger_record updates the timestamp on a repeat call so the
// "latest snapshot" invariant holds.
#[test]
fn ledger_record_updates_existing_row() {
    let (store, w) = writer();
    let _ = ledger::record_dispatch(&w, &store, "FT-O", "ENV-1", "2026-05-21T10:00:00Z")
        .expect("first record");
    let _ = ledger::record_dispatch(&w, &store, "FT-O", "ENV-1", "2026-05-21T11:00:00Z")
        .expect("second record");
    let entry = ledger::get_entry(&store, "FT-O", "ENV-1")
        .expect("ok")
        .expect("row");
    assert_eq!(entry.last_dispatch_at, "2026-05-21T11:00:00Z");
    // And there is only one row with the latest timestamp predicate.
    let count = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(entry.iri.clone()).as_ref()),
            Some(crate::core::vocab::last_dispatch_at()),
            None,
            None,
        )
        .filter_map(Result::ok)
        .count();
    assert_eq!(count, 1, "ledger row must keep exactly one last_dispatch_at");
}

// Validate the proposal document survives a JSON round-trip.
#[test]
fn proposal_document_round_trips_through_session_literal() {
    let (store, w) = writer();
    let s = seed("FT-Q", "ENV-1");
    let cfg = AutoDispatchConfig::default();
    let ev = emit_dispatch_event(&w, &store, &s, &cfg, "2026-05-21T10:00:00Z")
        .expect("ok")
        .expect("emit");
    let payload = serde_json::json!({
        "kind": "new",
        "bundle_hash": "deadbeef",
        "new": { "environment": "ENV-1", "steps": [], "rationale": "test" }
    });
    let s_json = payload.to_string();
    let session = session::persist_pending_review_session(
        &w,
        &PendingReviewInput {
            feature: "FT-Q",
            env: "ENV-1",
            proposal_document_json: &s_json,
            dispatch_event_iri: &ev.iri,
            started_at: "2026-05-21T10:00:01Z",
        },
    )
    .expect("session");
    let loaded = session::load_proposal_document(&store, &session.iri)
        .expect("ok")
        .expect("doc");
    let parsed: serde_json::Value = serde_json::from_str(&loaded).expect("parse");
    assert_eq!(parsed["kind"], "new");
    assert_eq!(parsed["bundle_hash"], "deadbeef");
}

// Sanity: silenced-warning suppression keeps `Literal` import in scope.
#[allow(dead_code)]
fn _keep_literal_used() {
    let _ = Literal::new_simple_literal("x");
}
