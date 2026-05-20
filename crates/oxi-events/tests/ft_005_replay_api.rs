//! FT-005 — Replay API integration tests.
//!
//! Validates: FT-005 · ADR-001, ADR-002, ADR-004.
//! Spec: `.product/features/FT-005-oxi-events-replay-api.md`.
//!
//! These tests stand up the full FT-001 commit path (writer +
//! subscription registry) so the rows that [`oxi_events::replay`]
//! observes are the very rows the writer produced. That keeps the
//! replay contract bound to the real persisted shape, not to a
//! hand-crafted fixture.

use std::sync::Arc;

use oxi_events::{
    replay, GraphWriter, Mutation, ReplayError, ReplayRequest, ReplayedEvent, SparqlFilterFragment,
    Subscription,
};
use oxigraph::model::{GraphName, NamedNode, Quad};
use oxigraph::store::Store;

const N_MUTATIONS: usize = 5;

fn seed(store: &Arc<Store>) -> Vec<ReplayedEvent> {
    let writer = GraphWriter::open(Arc::clone(store)).expect("writer opens");
    let sub = NamedNode::new("urn:oxi-events:ft-005:sub:all").expect("sub iri");
    writer
        .register_subscription(&Subscription::new(
            sub.clone(),
            "ASK { ?s ?p ?o }".to_string(),
        ))
        .expect("subscription registers");

    for i in 0..N_MUTATIONS {
        let s = NamedNode::new(format!("urn:oxi-events:ft-005:subject:{i}")).expect("s iri");
        let p = NamedNode::new("urn:oxi-events:ft-005:p").expect("p iri");
        let o = NamedNode::new(format!("urn:oxi-events:ft-005:object:{i}")).expect("o iri");
        let quad = Quad::new(s, p, o, GraphName::DefaultGraph);
        writer
            .commit(Mutation::insert([quad]).with_cause("ft-005-seed"))
            .expect("commit succeeds");
    }

    // Drop the writer; replay must work against the bare store.
    drop(writer);
    replay(store, &ReplayRequest::default()).expect("baseline replay")
}

#[test]
fn replay_returns_events_in_seq_ascending_order() {
    let store = Arc::new(Store::new().expect("store"));
    let events = seed(&store);

    assert_eq!(
        events.len(),
        N_MUTATIONS,
        "one matching subscription per mutation → one event per mutation"
    );
    for window in events.windows(2) {
        assert!(
            window[1].seq > window[0].seq,
            "replay output must be strictly seq-ascending: {events:?}"
        );
    }
    // FT-001 writer interleaves mutation+event seq numbers, so events
    // land on the even slots: 2, 4, 6, 8, 10 for N=5.
    let observed: Vec<u64> = events.iter().map(|e| e.seq).collect();
    let expected: Vec<u64> = (1..=N_MUTATIONS as u64).map(|i| 2 * i).collect();
    assert_eq!(observed, expected, "writer's contiguous seq pool");
}

#[test]
fn replay_is_deterministic_against_unchanged_graph() {
    let store = Arc::new(Store::new().expect("store"));
    let a = seed(&store);
    let b = replay(&store, &ReplayRequest::default()).expect("second replay");
    assert_eq!(a, b, "replay is deterministic on an unchanged graph");
}

#[test]
fn replay_respects_since_seq_inclusive_lower_bound() {
    let store = Arc::new(Store::new().expect("store"));
    let all = seed(&store);

    // since_seq = 4 should drop the first event (seq=2) and keep the rest.
    let from_4 = replay(&store, &ReplayRequest::since(4)).expect("since 4");
    let expected: Vec<&ReplayedEvent> = all.iter().filter(|e| e.seq >= 4).collect();
    assert_eq!(from_4.len(), expected.len());
    for (got, want) in from_4.iter().zip(expected.iter()) {
        assert_eq!(&got, want);
    }

    // since_seq beyond the highest seq returns no rows.
    let beyond = replay(&store, &ReplayRequest::since(9_999)).expect("empty");
    assert!(beyond.is_empty());
}

#[test]
fn replay_respects_until_seq_inclusive_upper_bound() {
    let store = Arc::new(Store::new().expect("store"));
    let all = seed(&store);

    let window = replay(
        &store,
        &ReplayRequest::since(0).with_until(6), // events at seq 2,4,6
    )
    .expect("until 6");

    assert_eq!(window.len(), 3, "events at seq 2, 4, 6 included");
    assert_eq!(window.last().expect("non-empty").seq, 6);
    assert_eq!(window.first().expect("non-empty").seq, 2);
    assert_eq!(window[0], all[0]);
    assert_eq!(window[2], all[2]);
}

#[test]
fn replay_respects_limit() {
    let store = Arc::new(Store::new().expect("store"));
    let all = seed(&store);

    let three = replay(&store, &ReplayRequest::since(0).with_limit(3)).expect("limit 3");
    assert_eq!(three.len(), 3);
    assert_eq!(&three[..], &all[..3]);
}

#[test]
fn replay_filter_clause_is_applied() {
    let store = Arc::new(Store::new().expect("store"));
    let all = seed(&store);

    // Filter: seq must be even (it always is in this fixture, so this
    // returns everything) AND seq < 8 (drops the last two events).
    let filter = SparqlFilterFragment::new("?seq < 8 && (?seq = 2 * (?seq / 2))");
    let filtered = replay(&store, &ReplayRequest::since(0).with_filter(filter)).expect("filter ok");

    let expected: Vec<&ReplayedEvent> = all.iter().filter(|e| e.seq < 8).collect();
    assert_eq!(filtered.len(), expected.len());
    for (got, want) in filtered.iter().zip(expected.iter()) {
        assert_eq!(&got, want);
    }
}

#[test]
fn invalid_filter_is_reported_at_request_time() {
    let store = Arc::new(Store::new().expect("store"));
    let _ = seed(&store);

    let bad = ReplayRequest::since(0).with_filter("this is not sparql ::::");
    let err = replay(&store, &bad).expect_err("invalid filter must error");
    assert!(
        matches!(err, ReplayError::InvalidFilter(_)),
        "filter parse error must surface as InvalidFilter; got: {err:?}"
    );
}

#[test]
fn replay_carries_prov_link_and_published_flag() {
    let store = Arc::new(Store::new().expect("store"));
    let events = seed(&store);

    // Every replayed row points at a non-empty mutation IRI and a
    // non-empty subscription IRI (PROV-O reachability invariant —
    // ADR-004 / TC-009).
    for ev in &events {
        assert!(
            !ev.mutation.is_empty(),
            "event must carry a mutation IRI: {ev:?}"
        );
        assert!(
            !ev.subscription.is_empty(),
            "event must carry a subscription IRI: {ev:?}"
        );
        assert_eq!(ev.status, "ok", "ASK subscription fires with status=ok");
        // Outbox hasn't run; events are still `published = false`.
        assert!(
            !ev.published,
            "events freshly written by GraphWriter are unpublished until the outbox sweeps"
        );
    }
}

#[test]
fn replay_is_read_only() {
    let store = Arc::new(Store::new().expect("store"));
    let before = seed(&store);
    let quad_count_before = store.len().expect("len");

    // Several replay calls (with various shapes) must not mutate the store.
    let _ = replay(&store, &ReplayRequest::default()).expect("a");
    let _ = replay(&store, &ReplayRequest::since(4).with_until(8)).expect("b");
    let _ = replay(
        &store,
        &ReplayRequest::since(0)
            .with_filter("?seq > 0")
            .with_limit(2),
    )
    .expect("c");

    let after = replay(&store, &ReplayRequest::default()).expect("d");
    assert_eq!(before, after, "replay must not mutate event state");
    assert_eq!(
        quad_count_before,
        store.len().expect("len"),
        "replay must not add or remove quads"
    );
}
