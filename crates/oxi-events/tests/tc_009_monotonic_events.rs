//! TC-009 — `GraphWriter` events have monotonic, contiguous sequence numbers.
//!
//! Validates: FT-001, FT-002, FT-003, FT-005 · ADR-002, ADR-004.
//! Spec: `.product/tests/TC-009-graphwriter-events-have-monotonic-sequence-numbers.md`

use std::sync::Arc;

use oxi_events::vocab::{
    IRI_OXI_EVENT, IRI_OXI_GRAPH_EVENTS, IRI_OXI_MUTATION, IRI_OXI_SEQ, IRI_PROV_WAS_GENERATED_BY,
};
use oxi_events::{GraphWriter, Mutation, Subscription};
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const N_MUTATIONS: usize = 6;

#[test]
fn graphwriter_events_have_monotonic_sequence_numbers() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let writer = GraphWriter::open(Arc::clone(&store)).expect("writer opens");

    let sub_id = NamedNode::new("urn:oxi-events:test:sub:all").expect("sub iri");
    writer
        .register_subscription(&Subscription::new(
            sub_id.clone(),
            "ASK { ?s ?p ?o }".to_string(),
        ))
        .expect("subscription registers");

    for i in 0..N_MUTATIONS {
        let s = NamedNode::new(format!("urn:oxi-events:test:subject:{i}")).expect("s iri");
        let p = NamedNode::new("urn:oxi-events:test:p").expect("p iri");
        let o = NamedNode::new(format!("urn:oxi-events:test:object:{i}")).expect("o iri");
        let quad = Quad::new(s, p, o, GraphName::DefaultGraph);
        let result = writer
            .commit(Mutation::insert([quad]).with_cause("test-mutation"))
            .expect("commit succeeds");
        assert!(
            !result.events.is_empty(),
            "at least one event per mutation expected"
        );
    }

    let events = select_events(&store);
    assert_eq!(
        events.len(),
        N_MUTATIONS,
        "one matching subscription per mutation → one event per mutation"
    );

    // Sequence numbers are strictly increasing (and overall contiguous across
    // mutation+event seq issuance, modulo the per-mutation seq that gets a slot too).
    for window in events.windows(2) {
        assert!(
            window[1].seq > window[0].seq,
            "event seq must be strictly increasing: {events:?}",
        );
    }

    // No gaps among event seqs (we issued exactly one event per mutation; events
    // share the seq pool with mutations, so events occupy alternating even slots
    // starting at 2 — see contract in writer.rs).
    let expected_seqs: Vec<u64> = (1..=N_MUTATIONS as u64).map(|i| 2 * i).collect();
    let observed_seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(observed_seqs, expected_seqs, "contiguous seq issuance");

    // Each event has prov:wasGeneratedBy → an oxi:Mutation that exists in the store.
    for ev in &events {
        let mutation = mutation_for_event(&store, &ev.iri).expect("event has prov:wasGeneratedBy");
        assert!(
            store_has_mutation_class(&store, &mutation),
            "mutation node {mutation} is typed oxi:Mutation"
        );
    }

    // ADR-002: events are persistent graph data. Drop the writer, open a fresh
    // one over the same store, re-run the SPARQL; same events, same order.
    drop(writer);
    let writer2 = GraphWriter::open(Arc::clone(&store)).expect("re-open writer");
    let events2 = select_events(&store);
    assert_eq!(
        events, events2,
        "events persist across writer re-open (slice 1 stand-in for process restart)"
    );

    // Re-opening must NOT mutate counter or emit new events.
    let next_seq = writer2.current_sequence().expect("current seq");
    assert_eq!(
        next_seq,
        2 * N_MUTATIONS as u64,
        "seq counter survives re-open"
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EventRow {
    iri: NamedNode,
    seq: u64,
}

fn select_events(store: &Store) -> Vec<EventRow> {
    let graph = IRI_OXI_GRAPH_EVENTS;
    let cls = IRI_OXI_EVENT;
    let seq_pred = IRI_OXI_SEQ;
    let q = format!(
        "SELECT ?event ?seq FROM <{graph}> WHERE {{ \
           ?event a <{cls}> ; \
                  <{seq_pred}> ?seq . \
         }} ORDER BY ?seq",
    );
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("select events") else {
        panic!("expected solutions");
    };
    for sol in sols {
        let sol = sol.expect("solution row");
        let Term::NamedNode(iri) = sol.get("event").expect("event term").clone() else {
            panic!("event must be a named node");
        };
        let Term::Literal(lit) = sol.get("seq").expect("seq term").clone() else {
            panic!("seq must be a literal");
        };
        let seq: u64 = lit.value().parse().expect("seq parses as u64");
        out.push(EventRow { iri, seq });
    }
    out
}

fn mutation_for_event(store: &Store, event: &NamedNode) -> Option<NamedNode> {
    let pred = NamedNodeRef::new(IRI_PROV_WAS_GENERATED_BY).expect("pred iri");
    let graph = NamedNodeRef::new(IRI_OXI_GRAPH_EVENTS).expect("events graph iri");
    let mut iter = store.quads_for_pattern(
        Some(event.as_ref().into()),
        Some(pred),
        None,
        Some(graph.into()),
    );
    let quad = iter.next()?.expect("quad ok");
    if let Term::NamedNode(n) = quad.object {
        Some(n)
    } else {
        None
    }
}

fn store_has_mutation_class(store: &Store, mutation: &NamedNode) -> bool {
    let q = format!(
        "ASK {{ GRAPH ?g {{ <{m}> a <{cls}> }} }}",
        m = mutation.as_str(),
        cls = IRI_OXI_MUTATION,
    );
    matches!(
        store.query(q.as_str()).expect("ask query"),
        QueryResults::Boolean(true)
    )
}
