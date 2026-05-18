//! FT-002 — oxi-events subscription registry and evaluator.
//!
//! These tests exercise the evaluator contract beyond the surface that
//! TC-009 already pins down:
//!
//! * SELECT-query subscriptions emit only on non-empty diff and stay
//!   silent on subsequent commits that don't change the result set.
//! * `Mutation::triggers` short-circuits subscriptions whose declared
//!   trigger set is disjoint.
//! * Malformed SPARQL is rejected at `register_subscription` time as a
//!   `RegistryError::InvalidQuery`.
//! * A SPARQL evaluation error is isolated to the offending subscription
//!   as `Delta::Error` — the commit still succeeds and a `failed` event
//!   lands in the events graph.
//! * `SubscriptionRegistry::load_from_store` rehydrates the in-memory
//!   mirror from persisted subscription records (ADR-003).
//! * Removed subscriptions produce no further deltas after the next
//!   commit.

use std::collections::BTreeSet;
use std::sync::Arc;

use oxi_events::vocab::{IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS, IRI_OXI_MATCHED_SUBSCRIPTION};
use oxi_events::{
    Delta, GraphWriter, Mutation, RegistryError, Subscription, SubscriptionMode,
    SubscriptionRegistry, WriterError,
};
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

fn make_writer() -> (Arc<Store>, GraphWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let writer = GraphWriter::open(Arc::clone(&store)).expect("writer opens");
    (store, writer)
}

fn insert_typed(name: &str, ty: &str) -> Mutation {
    let s = NamedNode::new(name).expect("subject iri");
    let rdf_type =
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").expect("rdf:type iri");
    let t = NamedNode::new(ty).expect("type iri");
    Mutation::insert([Quad::new(s, rdf_type, t, GraphName::DefaultGraph)])
}

fn events_for(store: &Store, sub: &NamedNode) -> usize {
    let graph = IRI_OXI_GRAPH_EVENTS;
    let cls = IRI_OXI_EVENT;
    let pred = IRI_OXI_MATCHED_SUBSCRIPTION;
    let q = format!(
        "SELECT (COUNT(?event) AS ?n) FROM <{graph}> WHERE {{ \
            ?event a <{cls}> ; <{pred}> <{sub_iri}> . \
         }}",
        sub_iri = sub.as_str(),
    );
    let QueryResults::Solutions(mut sols) = store.query(q.as_str()).expect("count query") else {
        panic!("expected solutions");
    };
    let Some(Ok(sol)) = sols.next() else {
        return 0;
    };
    let Some(Term::Literal(lit)) = sol.get("n").cloned() else {
        return 0;
    };
    lit.value().parse::<usize>().unwrap_or(0)
}

#[test]
fn select_subscription_emits_only_on_non_empty_diff() {
    let (store, writer) = make_writer();
    let sub_id = NamedNode::new("urn:test:sub:foo").expect("sub iri");
    writer
        .register_subscription(&Subscription::select(
            sub_id.clone(),
            "SELECT ?s WHERE { ?s a <urn:test:cls:Foo> }",
        ))
        .expect("registers");

    // First commit introduces ex:a — diff Added{ex:a} → emit.
    let r1 = writer
        .commit(insert_typed("urn:test:ex:a", "urn:test:cls:Foo"))
        .expect("commit 1");
    assert_eq!(r1.events.len(), 1, "Added → emit");

    // Second commit inserts an unrelated triple (different type); SELECT
    // result is unchanged → no emit.
    let r2 = writer
        .commit(insert_typed("urn:test:ex:b", "urn:test:cls:Bar"))
        .expect("commit 2");
    assert_eq!(
        r2.events.len(),
        0,
        "unchanged bindings → no event for the SELECT sub"
    );

    // Third commit adds ex:c of class Foo → Added{ex:c} → emit.
    let r3 = writer
        .commit(insert_typed("urn:test:ex:c", "urn:test:cls:Foo"))
        .expect("commit 3");
    assert_eq!(r3.events.len(), 1, "new binding → emit");

    assert_eq!(events_for(&store, &sub_id), 2);
}

#[test]
fn select_subscription_emits_removed_on_diff_after_removal() {
    let (_store, writer) = make_writer();
    let sub_id = NamedNode::new("urn:test:sub:removed").expect("sub iri");
    writer
        .register_subscription(&Subscription::select(
            sub_id.clone(),
            "SELECT ?s WHERE { ?s a <urn:test:cls:Doomed> }",
        ))
        .expect("registers");

    let s = NamedNode::new("urn:test:doomed:1").expect("s iri");
    let rdf_type =
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").expect("rdf:type iri");
    let cls = NamedNode::new("urn:test:cls:Doomed").expect("type iri");
    let typed = Quad::new(
        s.clone(),
        rdf_type.clone(),
        cls.clone(),
        GraphName::DefaultGraph,
    );

    let r1 = writer
        .commit(Mutation::insert([typed.clone()]))
        .expect("commit 1");
    assert_eq!(r1.events.len(), 1, "first sighting fires Added delta");

    let removal = Mutation {
        removes: vec![typed],
        ..Mutation::default()
    };
    let r2 = writer.commit(removal).expect("commit 2");
    assert_eq!(r2.events.len(), 1, "removal fires Removed delta");
}

#[test]
fn trigger_filter_skips_disjoint_subscription() {
    let (store, writer) = make_writer();
    let sub_id = NamedNode::new("urn:test:sub:goals").expect("sub iri");
    writer
        .register_subscription(
            &Subscription::select(sub_id.clone(), "SELECT ?s WHERE { ?s ?p ?o }")
                .with_triggers(["goal", "dispatch"]),
        )
        .expect("registers");

    // Mutation declares an unrelated trigger; sub must be skipped even
    // though the SELECT would otherwise match.
    let m = insert_typed("urn:test:ex:1", "urn:test:cls:Anything").with_triggers(["session"]);
    let r = writer.commit(m).expect("commit");
    assert_eq!(
        r.events.len(),
        0,
        "disjoint triggers → registered sub skipped"
    );
    assert_eq!(events_for(&store, &sub_id), 0);

    // Mutation declaring an overlapping trigger fires the subscription.
    let m2 = insert_typed("urn:test:ex:2", "urn:test:cls:Anything").with_triggers(["goal"]);
    let r2 = writer.commit(m2).expect("commit");
    assert_eq!(r2.events.len(), 1, "overlapping trigger → emit");
}

#[test]
fn empty_mutation_triggers_match_every_subscription() {
    let (_, writer) = make_writer();
    let sub_id = NamedNode::new("urn:test:sub:default").expect("sub iri");
    writer
        .register_subscription(
            &Subscription::new(sub_id, "ASK { ?s ?p ?o }").with_triggers(["session"]),
        )
        .expect("registers");

    // Mutation declares no triggers. TC-009 / FT-001 callers may not
    // know about FT-002 trigger semantics yet; we treat "no triggers
    // declared" as "every registered subscription is a candidate" so
    // legacy callers continue to fire subscriptions.
    let r = writer
        .commit(insert_typed("urn:test:ex:x", "urn:test:cls:X"))
        .expect("commit");
    assert_eq!(r.events.len(), 1, "no-trigger mutation hits ASK sub");
}

#[test]
fn malformed_query_is_rejected_at_registration() {
    let registry = SubscriptionRegistry::new();
    let bad = Subscription::new(
        NamedNode::new("urn:test:sub:bad").expect("iri"),
        "ASK definitely not sparql",
    );
    let err = registry.register(bad).expect_err("must error");
    match err {
        RegistryError::InvalidQuery { subscription, .. } => {
            assert_eq!(subscription, "urn:test:sub:bad");
        }
        other => panic!("expected InvalidQuery, got {other:?}"),
    }
}

#[test]
fn malformed_query_through_writer_surfaces_as_registry_error() {
    let (_, writer) = make_writer();
    let bad = Subscription::select(
        NamedNode::new("urn:test:sub:bad2").expect("iri"),
        "SELECT not valid",
    );
    match writer.register_subscription(&bad) {
        Err(WriterError::Registry(RegistryError::InvalidQuery { subscription, .. })) => {
            assert_eq!(subscription, "urn:test:sub:bad2");
        }
        other => panic!("expected Registry(InvalidQuery), got {other:?}"),
    }
}

#[test]
fn registry_is_persisted_and_rehydrated_across_open() {
    let store = Arc::new(Store::new().expect("store"));
    let writer = GraphWriter::open(Arc::clone(&store)).expect("writer");
    let sub_id = NamedNode::new("urn:test:sub:persist").expect("iri");
    let sub = Subscription::select(sub_id.clone(), "SELECT ?s WHERE { ?s ?p ?o }")
        .with_label("persistence-check")
        .with_triggers(["session", "goal"])
        .with_mode(SubscriptionMode::Async)
        .with_handler("urn:test:handler:1");
    writer.register_subscription(&sub).expect("registers");

    drop(writer);
    let writer2 = GraphWriter::open(Arc::clone(&store)).expect("reopen");
    let subs = writer2.registry().snapshot().expect("snapshot");
    assert_eq!(subs.len(), 1, "exactly one persisted sub");
    let restored = &subs[0];
    assert_eq!(restored.id, sub_id);
    assert_eq!(restored.label.as_deref(), Some("persistence-check"));
    assert_eq!(restored.mode, SubscriptionMode::Async);
    assert_eq!(
        restored.triggers,
        BTreeSet::from(["session".to_string(), "goal".to_string()])
    );
    assert_eq!(
        restored
            .handler
            .as_ref()
            .expect("handler restored")
            .as_str(),
        "urn:test:handler:1"
    );
}

#[test]
fn removed_subscription_produces_no_further_events() {
    let (store, writer) = make_writer();
    let sub_id = NamedNode::new("urn:test:sub:gone").expect("iri");
    writer
        .register_subscription(&Subscription::new(sub_id.clone(), "ASK { ?s ?p ?o }"))
        .expect("registers");

    writer
        .commit(insert_typed("urn:test:ex:1", "urn:test:cls:X"))
        .expect("first commit");
    let baseline = events_for(&store, &sub_id);
    assert_eq!(baseline, 1, "first commit emits");

    let removed = writer.remove_subscription(&sub_id).expect("remove");
    assert!(removed, "subscription was registered");

    writer
        .commit(insert_typed("urn:test:ex:2", "urn:test:cls:X"))
        .expect("second commit");
    assert_eq!(
        events_for(&store, &sub_id),
        baseline,
        "no further events emitted after removal"
    );
}

#[test]
fn evaluation_error_isolates_to_subscription() {
    let (store, writer) = make_writer();

    // The sub's query parses cleanly but is a DESCRIBE rather than an
    // ASK. The registry stores it as `SubscriptionQuery::Ask` (the
    // `Subscription::new` constructor's contract) and the evaluator
    // surfaces the type-mismatch as `Delta::Error` for that
    // subscription only — the commit still completes and other subs
    // still fire (FT-002 §Error handling).
    let bad_id = NamedNode::new("urn:test:sub:bad-eval").expect("iri");
    let bad = Subscription::new(bad_id.clone(), "DESCRIBE <urn:test:any>");
    writer.register_subscription(&bad).expect("registers");

    let good_id = NamedNode::new("urn:test:sub:good").expect("iri");
    writer
        .register_subscription(&Subscription::new(good_id.clone(), "ASK { ?s ?p ?o }"))
        .expect("registers good sub");

    let r = writer
        .commit(insert_typed("urn:test:ex:e", "urn:test:cls:E"))
        .expect("commit completes despite bad sub");

    // The bad sub's evaluation failure should not block the commit; the
    // good sub still fires.
    assert!(
        r.events.iter().any(|e| e.subscription == good_id),
        "good sub fires"
    );

    // Confirm via the registry-level API that the bad sub surfaced as
    // a Delta::Error.
    let triggers = std::collections::BTreeSet::new();
    let matches = writer
        .registry()
        .evaluate(&store, &triggers)
        .expect("evaluate");
    let bad_match = matches
        .iter()
        .find(|m| m.subscription_id == bad_id)
        .expect("bad sub produced a match record");
    assert!(matches!(bad_match.delta, Delta::Error(_)));
}

#[test]
fn duplicate_registration_returns_duplicate_error() {
    let registry = SubscriptionRegistry::new();
    let id = NamedNode::new("urn:test:sub:dup").expect("iri");
    registry
        .register(Subscription::new(id.clone(), "ASK { ?s ?p ?o }"))
        .expect("first ok");
    let err = registry
        .register(Subscription::new(id.clone(), "ASK { ?s ?p ?o }"))
        .expect_err("duplicate rejected");
    match err {
        RegistryError::DuplicateSubscription(iri) => assert_eq!(iri, id.as_str()),
        other => panic!("expected DuplicateSubscription, got {other:?}"),
    }
}

#[test]
fn replace_resets_diff_baseline() {
    let registry = SubscriptionRegistry::new();
    let id = NamedNode::new("urn:test:sub:replaced").expect("iri");
    let sub = Subscription::select(id.clone(), "SELECT ?s WHERE { ?s ?p ?o }");
    registry.register(sub.clone()).expect("register");

    let store = Store::new().expect("store");
    // Seed one triple so the first eval fills the cache.
    let s = NamedNode::new("urn:test:s").expect("iri");
    let p = NamedNode::new("urn:test:p").expect("iri");
    let o = NamedNode::new("urn:test:o").expect("iri");
    store
        .insert(Quad::new(s, p, o, GraphName::DefaultGraph).as_ref())
        .expect("seed");

    let triggers = std::collections::BTreeSet::new();
    let _ = registry.evaluate(&store, &triggers).expect("eval 1");
    assert!(registry
        .cached_bindings(&id)
        .expect("cached lookup")
        .is_some());

    let replaced =
        Subscription::select(id.clone(), "SELECT ?s WHERE { ?s ?p ?o }").with_label("replacement");
    registry.replace(&replaced).expect("replace");

    assert!(
        registry
            .cached_bindings(&id)
            .expect("cached lookup post-replace")
            .is_none(),
        "replace clears the diff baseline"
    );
}

// Used so the test only references stable API surface, not internal helpers.
#[allow(dead_code)]
fn _api_signature_check() {
    let _: &dyn Fn(&SubscriptionRegistry) -> usize = &SubscriptionRegistry::len;
    let _ = NamedNodeRef::new_unchecked("urn:dummy");
}
