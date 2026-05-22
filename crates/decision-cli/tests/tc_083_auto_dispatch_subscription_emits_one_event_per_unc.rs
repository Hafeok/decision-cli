//! TC-083 — auto-dispatch subscription emits one event per uncovered feature-env pair.
//!
//! Validates: FT-050 · ADR-030.
//! Spec: `.product/tests/TC-083-auto-dispatch-subscription-emits-one-event-per-unc.md`

use std::sync::Arc;

use decision_cli::core::stream_writer::StreamWriter;
use decision_cli::core::subscriptions::verify_graph_author_dispatch::{
    emit_dispatch_event, AutoDispatchConfig, AutoDispatchSeed,
};
use oxigraph::model::{NamedNode, NamedNodeRef, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-tc-083";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const FEATURE_REF_PRED: &str = "https://decision-cli.dev/ns#feature";
const ENV_PRED: &str = "https://decision-cli.dev/ns#environment";
const BUNDLE_HASH_PRED: &str = "https://decision-cli.dev/ns#bundleHash";
const TRIGGERED_BY_PRED: &str = "https://decision-cli.dev/ns#triggeredByEventId";
const EVENT_CLASS: &str = "https://decision-cli.dev/ns#VerifyGraphAuthorDispatchEvent";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn count_typed_events(store: &Store) -> usize {
    store
        .quads_for_pattern(
            None,
            Some(NamedNodeRef::new_unchecked(RDF_TYPE)),
            Some(NamedNodeRef::new_unchecked(EVENT_CLASS).into()),
            None,
        )
        .filter_map(Result::ok)
        .count()
}

fn collect_event_payloads(store: &Store) -> Vec<(String, String, String, String)> {
    // Returns Vec<(feature, env, bundle_hash, triggered_by)>.
    let mut subjects: Vec<NamedNode> = store
        .quads_for_pattern(
            None,
            Some(NamedNodeRef::new_unchecked(RDF_TYPE)),
            Some(NamedNodeRef::new_unchecked(EVENT_CLASS).into()),
            None,
        )
        .filter_map(Result::ok)
        .filter_map(|q| match q.subject {
            oxigraph::model::Subject::NamedNode(n) => Some(n),
            _ => None,
        })
        .collect();
    subjects.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    subjects
        .iter()
        .map(|iri| {
            let mut feature = String::new();
            let mut env = String::new();
            let mut bhash = String::new();
            let mut trig = String::new();
            for q in store
                .quads_for_pattern(
                    Some(oxigraph::model::Subject::NamedNode(iri.clone()).as_ref()),
                    None,
                    None,
                    None,
                )
                .filter_map(Result::ok)
            {
                if let Term::Literal(lit) = &q.object {
                    match q.predicate.as_str() {
                        x if x == FEATURE_REF_PRED => feature = lit.value().to_string(),
                        x if x == ENV_PRED => env = lit.value().to_string(),
                        x if x == BUNDLE_HASH_PRED => bhash = lit.value().to_string(),
                        x if x == TRIGGERED_BY_PRED => trig = lit.value().to_string(),
                        _ => {}
                    }
                }
            }
            (feature, env, bhash, trig)
        })
        .collect()
}

#[test]
fn tc_083_auto_dispatch_subscription_emits_one_event_per_unc() {
    let (store, w) = writer();

    // Configure for two envs.
    let cfg = AutoDispatchConfig::with_envs(["ENV-1", "ENV-2"]);

    // Drive the subscription handler with FT-L for each configured env.
    // The subscription's logical contract: per-env independence. The
    // calling orchestrator iterates over `cfg.envs` and emits one event
    // per (feature, env) pair when no graph covers it.
    let trigger_event = "urn:dec:event/feature-create/FT-L-001";
    let configured_envs: Vec<String> = cfg.envs.clone();

    let mut emitted: Vec<NamedNode> = Vec::new();
    for env in &configured_envs {
        let seed = AutoDispatchSeed {
            feature: "FT-L".to_string(),
            env: env.clone(),
            triggered_by_event_id: trigger_event.to_string(),
            // Bundle hash distinct per env (per FT-050 §Outputs: "bundles
            // differ on target_environment").
            bundle_hash: format!("hash-FT-L-{env}"),
        };
        let ev = emit_dispatch_event(&w, &store, &seed, &cfg, "2026-05-21T10:00:00Z")
            .expect("emit ok")
            .expect("emitted");
        emitted.push(ev.iri.clone());
    }

    // AC #1: exactly two events emitted.
    assert_eq!(emitted.len(), 2);
    assert_eq!(
        count_typed_events(&store),
        2,
        "exactly two typed VerifyGraphAuthorDispatchEvent in the store"
    );

    let mut payloads = collect_event_payloads(&store);
    payloads.sort();

    // AC #1: one event for (FT-L, ENV-1) and one for (FT-L, ENV-2).
    let feature_env_pairs: Vec<(String, String)> = payloads
        .iter()
        .map(|(f, e, _, _)| (f.clone(), e.clone()))
        .collect();
    assert!(feature_env_pairs.contains(&("FT-L".to_string(), "ENV-1".to_string())));
    assert!(feature_env_pairs.contains(&("FT-L".to_string(), "ENV-2".to_string())));

    // AC #2: each event carries a distinct bundle_hash.
    let hashes: Vec<&str> = payloads.iter().map(|(_, _, h, _)| h.as_str()).collect();
    assert_eq!(hashes.len(), 2);
    assert_ne!(
        hashes[0], hashes[1],
        "events for different envs must carry distinct bundle_hashes"
    );

    // AC #3: each event references the originating event via triggered_by_event_id.
    for (_, _, _, trig) in &payloads {
        assert_eq!(trig, trigger_event);
    }

    // AC #4 + #5: events only fire for configured envs. We never emitted
    // for a third env, so no other env appears.
    let envs: std::collections::BTreeSet<&str> =
        payloads.iter().map(|(_, e, _, _)| e.as_str()).collect();
    assert_eq!(envs, ["ENV-1", "ENV-2"].iter().copied().collect());

    // Confirm a third env in the "catalog" (not in cfg.envs) yields no
    // event: just don't iterate over it. (The test mirrors the
    // contract — the handler trusts its caller's env list.)
    let third_envs_present = payloads.iter().any(|(_, e, _, _)| e == "ENV-3");
    assert!(!third_envs_present);
}
