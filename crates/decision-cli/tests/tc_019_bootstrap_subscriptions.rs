//! TC-019 — `dec init` seeds the v0 bootstrap subscriptions.
//! Validates: FT-009 · ADR-003.
//! Spec: .product/tests/TC-019-dec-init-seeds-v0-bootstrap-subscriptions.md
//!
//! The v0 bootstrap subscriptions ("dispatch available for code-writer"
//! and "code-writer dispatch completed") are part of FT-009's §Behaviour
//! step 4. Without them the orchestration substrate is silent: every
//! mutation through `StreamWriter` commits, but no `oxi:Event` is minted
//! because no subscription matches. This test fails if a future init
//! path ships without persisting the subscriptions in the graph, the
//! way it did before this regression was caught.
//!
//! The test does three things:
//!
//! 1. After `dec init`, queries the persisted store dump for the two
//!    `oxi:Subscription` IRIs — exact count, exact graph, exact
//!    `oxi:mode`, exact query class (SELECT, not ASK).
//! 2. Re-opens a `GraphWriter` over the store and asserts the registry
//!    rehydrates with 2 subscriptions. This proves the persisted form is
//!    consumable by FT-002's `load_from_store`, not just well-formed.
//! 3. Drives one stub `dec implement` run and replays from seq 0 —
//!    at least 2 events must be present, one per seeded subscription.
//!    This is the round-trip: dormant subscriptions are worthless;
//!    they have to fire on real commits.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxi_events::vocab::{
    IRI_OXI_GRAPH_SUBSCRIPTIONS, IRI_OXI_SUBSCRIPTION, IRI_OXI_SUB_ASK_QUERY, IRI_OXI_SUB_MODE,
    IRI_OXI_SUB_SELECT_QUERY, SUB_MODE_INLINE,
};
use oxi_events::{replay, GraphWriter, ReplayRequest};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const SUB_DISPATCH_AVAILABLE: &str =
    "https://decision-cli.dev/ns/subscription/dispatch-available-code-writer";
const SUB_DISPATCH_COMPLETED: &str =
    "https://decision-cli.dev/ns/subscription/dispatch-completed-code-writer";

#[test]
fn dec_init_seeds_v0_bootstrap_subscriptions() {
    let workdir = tempdir("tc-019");

    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");

    let store = load_store_from_dump(&workdir);

    // -- 1. Exactly two Subscription instances, with the expected IRIs.
    let count_q = format!(
        "SELECT (COUNT(?s) AS ?n) WHERE {{ GRAPH <{g}> {{ ?s a <{c}> }} }}",
        g = IRI_OXI_GRAPH_SUBSCRIPTIONS,
        c = IRI_OXI_SUBSCRIPTION,
    );
    let count = scalar_int(&store, &count_q);
    assert_eq!(
        count, 2,
        "expected exactly 2 seeded Subscriptions in <{IRI_OXI_GRAPH_SUBSCRIPTIONS}>, found {count}"
    );

    for iri in [SUB_DISPATCH_AVAILABLE, SUB_DISPATCH_COMPLETED] {
        let exists_q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <{c}> }} }}",
            g = IRI_OXI_GRAPH_SUBSCRIPTIONS,
            c = IRI_OXI_SUBSCRIPTION,
        );
        assert!(
            ask(&store, &exists_q),
            "expected seeded Subscription <{iri}> in subscriptions graph"
        );

        // SELECT, not ASK — FT-009 spec mandates SELECT so each new
        // dispatch / completion emits one event rather than firing on
        // every commit. A regression to ASK would still satisfy "fires",
        // but break delta semantics — fail loudly.
        let mode_q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> <{m}> \"{mode}\" }} }}",
            g = IRI_OXI_GRAPH_SUBSCRIPTIONS,
            m = IRI_OXI_SUB_MODE,
            mode = SUB_MODE_INLINE,
        );
        assert!(
            ask(&store, &mode_q),
            "<{iri}> should carry oxi:mode \"inline\""
        );

        let select_q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> <{p}> ?body }} FILTER(STR(?body) != \"\") }}",
            g = IRI_OXI_GRAPH_SUBSCRIPTIONS,
            p = IRI_OXI_SUB_SELECT_QUERY,
        );
        assert!(
            ask(&store, &select_q),
            "<{iri}> should carry a non-empty oxi:selectQuery literal"
        );

        let no_ask_q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> <{p}> ?body }} }}",
            g = IRI_OXI_GRAPH_SUBSCRIPTIONS,
            p = IRI_OXI_SUB_ASK_QUERY,
        );
        assert!(
            !ask(&store, &no_ask_q),
            "<{iri}> should NOT carry oxi:askQuery — FT-009 mandates SELECT semantics"
        );
    }

    // -- 2. GraphWriter rehydrates the registry from the persisted form.
    let store_for_writer = Arc::new(load_store_from_dump(&workdir));
    let writer =
        GraphWriter::open(Arc::clone(&store_for_writer)).expect("open GraphWriter over store");
    assert_eq!(
        writer.registry().len(),
        2,
        "registry should rehydrate with both seeded subscriptions"
    );

    // -- 3. Round-trip: a real dispatch produces events for both subs.
    env::set_var("CODE_WRITER_STUB", "1");
    let args = ImplementArgs::new("FT-013");
    implement_run(&workdir, &args).expect("dec implement (stub)");

    let post_store = load_store_from_dump(&workdir);
    let events = replay(&post_store, &ReplayRequest::since(0)).expect("replay");
    assert!(
        events.len() >= 2,
        "expected ≥2 events after one stub dispatch (one per seeded subscription), got {}",
        events.len()
    );
    let subs_seen: std::collections::BTreeSet<&str> =
        events.iter().map(|e| e.subscription.as_str()).collect();
    assert!(
        subs_seen.contains(SUB_DISPATCH_AVAILABLE),
        "no event from dispatch-available subscription; saw {subs_seen:?}"
    );
    assert!(
        subs_seen.contains(SUB_DISPATCH_COMPLETED),
        "no event from dispatch-completed subscription; saw {subs_seen:?}"
    );
}

fn load_store_from_dump(workdir: &PathBuf) -> Store {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump).expect("read store dump");
    let store = Store::new().expect("store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store");
    store
}

fn ask(store: &Store, q: &str) -> bool {
    match store.query(q).expect("ASK") {
        QueryResults::Boolean(b) => b,
        _ => panic!("expected ASK to return boolean"),
    }
}

fn scalar_int(store: &Store, q: &str) -> i64 {
    match store.query(q).expect("SELECT count") {
        QueryResults::Solutions(mut sols) => {
            let sol = sols
                .next()
                .expect("count returned no rows")
                .expect("count solution");
            let term = sol.get("n").expect("missing ?n");
            let oxigraph::model::Term::Literal(lit) = term else {
                panic!("expected literal count, got {term:?}");
            };
            lit.value().parse().expect("count literal not an integer")
        }
        _ => panic!("expected SELECT to return solutions"),
    }
}

fn tempdir(label: &str) -> PathBuf {
    let mut p = env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("{label}-{pid}-{nonce}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}
