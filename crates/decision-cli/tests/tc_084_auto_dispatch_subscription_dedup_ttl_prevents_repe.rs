//! TC-084 — auto-dispatch subscription dedup TTL prevents repeat dispatches on edits.
//!
//! Validates: FT-050 · ADR-030.
//! Spec: `.product/tests/TC-084-auto-dispatch-subscription-dedup-ttl-prevents-repe.md`

use std::sync::Arc;

use decision_cli::core::stream_writer::StreamWriter;
use decision_cli::core::subscriptions::verify_graph_author_dispatch::{
    emit_dispatch_event,
    ledger::{debug_set_timestamp, get_entry},
    AutoDispatchConfig, AutoDispatchSeed,
};
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-tc-084";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn seed_for(idx: u32) -> AutoDispatchSeed {
    AutoDispatchSeed {
        feature: "FT-K".to_string(),
        env: "ENV-1".to_string(),
        triggered_by_event_id: format!("urn:dec:event/feature-update/FT-K/{idx}"),
        bundle_hash: format!("hash-FT-K-update-{idx}"),
    }
}

fn count_events(store: &Store) -> usize {
    use oxigraph::model::NamedNodeRef;
    let event_class = NamedNodeRef::new_unchecked(
        "https://decision-cli.dev/ns#VerifyGraphAuthorDispatchEvent",
    );
    let rdf_type = NamedNodeRef::new_unchecked(
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
    );
    store
        .quads_for_pattern(None, Some(rdf_type), Some(event_class.into()), None)
        .filter_map(Result::ok)
        .count()
}

#[test]
fn tc_084_auto_dispatch_subscription_dedup_ttl_prevents_repe() {
    let (store, w) = writer();

    // Configure default 1-hour TTL.
    let cfg = AutoDispatchConfig::default();

    // 1. Feature creation triggers the first dispatch.
    let first = emit_dispatch_event(
        &w,
        &store,
        &seed_for(0),
        &cfg,
        "2026-05-21T10:00:00Z",
    )
    .expect("emit ok")
    .expect("first emission");
    assert_eq!(first.feature, "FT-K");

    // 2. Three feature-update events within the TTL window.
    for i in 1..=3u32 {
        // Minute offsets 5, 10, 15 — all well within the 60-minute TTL.
        let mins = (i * 5) as u8;
        let ts = format!("2026-05-21T10:{:02}:00Z", mins);
        let r = emit_dispatch_event(&w, &store, &seed_for(i), &cfg, &ts).expect("emit ok");
        assert!(
            r.is_none(),
            "update #{i} within TTL must be suppressed by dedup ledger"
        );
    }

    // AC #1: only the original dispatch event in the store.
    assert_eq!(
        count_events(&store),
        1,
        "exactly one dispatch event after three updates"
    );

    // AC #2: ledger entry timestamp is the *original* dispatch time.
    let entry = get_entry(&store, "FT-K", "ENV-1")
        .expect("ledger ok")
        .expect("row present");
    assert_eq!(entry.feature, "FT-K");
    assert_eq!(entry.env, "ENV-1");
    assert_eq!(entry.last_dispatch_at, "2026-05-21T10:00:00Z");

    // AC #3: after the TTL elapses (simulated via directly aging the
    // ledger), a feature-update event fires a fresh dispatch.
    debug_set_timestamp(&store, "FT-K", "ENV-1", "2026-05-21T08:00:00Z")
        .expect("aging ledger");
    let aged = emit_dispatch_event(
        &w,
        &store,
        &seed_for(4),
        &cfg,
        "2026-05-21T10:30:00Z",
    )
    .expect("emit ok")
    .expect("post-TTL emission must succeed");
    assert_eq!(aged.feature, "FT-K");
    assert_eq!(count_events(&store), 2, "second event after TTL elapses");

    // AC #4: setting TTL to 0 causes every event to dispatch (testing override).
    let cfg_zero = AutoDispatchConfig::default().with_ttl(0);
    // Even without aging the ledger, ttl=0 lets every event through.
    let no_dedup_1 = emit_dispatch_event(
        &w,
        &store,
        &seed_for(5),
        &cfg_zero,
        "2026-05-21T10:31:00Z",
    )
    .expect("emit ok")
    .expect("ttl=0 emits regardless");
    let no_dedup_2 = emit_dispatch_event(
        &w,
        &store,
        &seed_for(6),
        &cfg_zero,
        "2026-05-21T10:31:01Z",
    )
    .expect("emit ok")
    .expect("ttl=0 emits again");
    assert_ne!(no_dedup_1.iri, no_dedup_2.iri);
    assert_eq!(count_events(&store), 4, "ttl=0 produces 2 more events");
}
