//! TC-010 — Outbox resumes in-flight dispatch after crash.
//!
//! Validates: FT-003 (event emission and outbox) · ADR-002.
//! Spec: `.product/tests/TC-010-outbox-resumes-in-flight-dispatch-after-crash.md`
//!
//! The chaos spec calls for a SIGKILL on the `dec` binary; we simulate
//! that in-process by:
//!
//! 1. Opening a [`GraphWriter`] WITHOUT attaching an outbox — committing
//!    mutations therefore lands events in the store with
//!    `oxi:published = false`, and nothing flips them.
//! 2. Dropping the writer — equivalent to the process dying.
//! 3. Re-opening over the **same** store and attaching a fresh
//!    [`OutboxPublisher`] (the "restart" path).
//! 4. Asserting the publisher's initial sweep picks up every stranded
//!    event, fans it out on the broadcast bus, and flips the flag.
//!
//! The bounded-recovery clause (`T ≤ 5s`) is asserted by running the
//! whole recovery inside a tokio timeout.

use std::sync::Arc;
use std::time::Duration;

use oxi_events::vocab::{IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS, IRI_OXI_PUBLISHED, IRI_OXI_SEQ};
use oxi_events::{GraphWriter, Mutation, OutboxPublisher, Subscription};
use oxigraph::model::{GraphName, NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use tokio::time::timeout;

const N_DISPATCHES: usize = 3;
const RECOVERY_BUDGET: Duration = Duration::from_secs(5);

#[tokio::test]
async fn outbox_resumes_in_flight_dispatch_after_crash() {
    // ── pre-crash phase ───────────────────────────────────────────────
    let store = Arc::new(Store::new().expect("in-memory store"));
    let writer = GraphWriter::open(Arc::clone(&store)).expect("writer opens");
    let sub_id = NamedNode::new("urn:oxi-events:test:sub:dispatch").expect("sub iri");
    writer
        .register_subscription(&Subscription::new(sub_id, "ASK { ?s ?p ?o }".to_string()))
        .expect("register subscription");

    // No outbox attached → events land with `published = false` and the
    // process "crashes" before anyone can flip the flag.
    for i in 0..N_DISPATCHES {
        let s = NamedNode::new(format!("urn:oxi-events:test:dispatch:{i}")).expect("s iri");
        let p = NamedNode::new("urn:oxi-events:test:dispatched").expect("p iri");
        let o = NamedNode::new(format!("urn:oxi-events:test:artifact:{i}")).expect("o iri");
        let quad = Quad::new(s, p, o, GraphName::DefaultGraph);
        let result = writer
            .commit(Mutation::insert([quad]).with_cause("in-flight-dispatch"))
            .expect("commit succeeds");
        assert!(
            !result.events.is_empty(),
            "subscription must emit at least one event per dispatch"
        );
    }

    let pre_crash = unpublished_events(&store);
    assert_eq!(
        pre_crash.len(),
        N_DISPATCHES,
        "every pre-crash event must be marked oxi:published = false"
    );

    // SIGKILL stand-in: drop the writer, simulating process death.
    drop(writer);

    // ── recovery phase ────────────────────────────────────────────────
    let post_crash_store = Arc::clone(&store);
    let outcome = timeout(RECOVERY_BUDGET, async move {
        // Restart the writer over the same store and stand up an outbox.
        let writer2 = GraphWriter::open(Arc::clone(&post_crash_store)).expect("writer re-opens");
        let publisher = Arc::new(OutboxPublisher::with_defaults(Arc::clone(
            &post_crash_store,
        )));
        writer2
            .attach_outbox(Arc::clone(&publisher))
            .expect("attach outbox");

        // Subscribe BEFORE spawning the loop so we don't race the first
        // sweep — the broadcast channel only delivers to live receivers.
        let mut rx = publisher.subscribe();
        let handle = Arc::clone(&publisher).spawn();

        // Drain the receiver until every in-flight event has been re-
        // dispatched. Strictly increasing seq is verified inline.
        let mut received_seqs = Vec::new();
        for _ in 0..N_DISPATCHES {
            let env = rx.recv().await.expect("re-dispatched envelope");
            received_seqs.push(env.seq);
        }
        for window in received_seqs.windows(2) {
            assert!(
                window[1] > window[0],
                "delivery preserves seq ordering: {received_seqs:?}"
            );
        }

        handle.shutdown().await;
        (publisher, received_seqs)
    })
    .await
    .expect("recovery must complete within budget");

    let (_publisher, received_seqs) = outcome;
    assert_eq!(
        received_seqs.len(),
        N_DISPATCHES,
        "every pre-crash event must be re-dispatched"
    );

    // After successful re-delivery the persisted flag must flip to true.
    let remaining = unpublished_events(&store);
    assert!(
        remaining.is_empty(),
        "outbox must mark events oxi:published = true after delivery; remaining: {remaining:?}"
    );

    // No event was lost: every pre-crash IRI is still present in the store.
    for ev in &pre_crash {
        let present = store_contains_event(&store, ev);
        assert!(present, "event {ev} must survive recovery");
    }
}

fn unpublished_events(store: &Store) -> Vec<String> {
    let graph = IRI_OXI_GRAPH_EVENTS;
    let cls = IRI_OXI_EVENT;
    let published = IRI_OXI_PUBLISHED;
    let seq = IRI_OXI_SEQ;
    let q = format!(
        "SELECT ?e ?seq FROM <{graph}> WHERE {{ \
            ?e a <{cls}> ; \
               <{published}> false ; \
               <{seq}> ?seq \
         }} ORDER BY ?seq"
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("select unpublished") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution row");
        if let Some(Term::NamedNode(n)) = sol.get("e").cloned() {
            out.push(n.as_str().to_string());
        }
    }
    out
}

fn store_contains_event(store: &Store, iri: &str) -> bool {
    let graph = IRI_OXI_GRAPH_EVENTS;
    let cls = IRI_OXI_EVENT;
    let q = format!("ASK FROM <{graph}> {{ <{iri}> a <{cls}> }}");
    matches!(
        store.query(q.as_str()).expect("ask event"),
        QueryResults::Boolean(true)
    )
}
