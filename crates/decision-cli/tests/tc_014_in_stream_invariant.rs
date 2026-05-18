//! TC-014 — Every Session/Goal/Dispatch/Event carries `dec:inStream`.
//!
//! Validates: FT-001, FT-010 · ADR-005.
//! Spec: `.product/tests/TC-014-orchestration-artifacts-carry-dec-in-stream.md`

use std::sync::Arc;

use decision_cli::vocab::{
    orchestration_graph, IRI_DEC_DISPATCH, IRI_DEC_EVENT, IRI_DEC_GOAL, IRI_DEC_IN_STREAM,
    IRI_DEC_SESSION, IRI_DEC_VALUE_STREAM,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

#[test]
fn orchestration_artifacts_carry_dec_in_stream() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new("https://decision-cli.dev/stream/test-stream").expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream.clone())
        .expect("stream writer bootstraps");

    // One artifact per scoped class, plus one Event-but-not-scoped-class
    // to make sure the middleware is selective.
    commit_typed(&writer, "urn:test:session/A", IRI_DEC_SESSION);
    commit_typed(&writer, "urn:test:goal/A", IRI_DEC_GOAL);
    commit_typed(&writer, "urn:test:dispatch/A", IRI_DEC_DISPATCH);
    commit_typed(&writer, "urn:test:event/A", IRI_DEC_EVENT);

    // A non-scoped class — must NOT be tagged. (Doubles as a control.)
    let non_scoped = NamedNode::new("urn:test:plain/A").expect("subject iri");
    let some_other_class =
        NamedNode::new("https://example.test/ns#PlainArtifact").expect("class iri");
    writer
        .commit(Mutation::insert([Quad::new(
            non_scoped.clone(),
            NamedNodeRef::new(RDF_TYPE).expect("rdf:type"),
            some_other_class,
            GraphName::NamedNode(orchestration_graph().into_owned()),
        )]))
        .expect("plain commit");

    // Audit — TC-014's negative SPARQL must be empty.
    assert!(
        scoped_orphans(&store).is_empty(),
        "every scoped artifact must carry dec:inStream → dec:ValueStream"
    );

    // Each tagged artifact points at the active stream specifically.
    for subject in [
        "urn:test:session/A",
        "urn:test:goal/A",
        "urn:test:dispatch/A",
        "urn:test:event/A",
    ] {
        let s = NamedNode::new(subject).expect("subject iri");
        let observed = stream_for(&store, &s).expect("stream link present");
        assert_eq!(observed, stream, "subject {subject} bound to active stream");
    }

    // Control: non-scoped subject is NOT tagged.
    assert!(
        stream_for(&store, &non_scoped).is_none(),
        "non-scoped subject must not be tagged"
    );
}

fn commit_typed(writer: &StreamWriter, subject: &str, class: &str) {
    let s = NamedNode::new(subject).expect("subject iri");
    let class = NamedNode::new(class).expect("class iri");
    let type_pred = NamedNodeRef::new(RDF_TYPE).expect("rdf:type");
    let graph: GraphName = orchestration_graph().into_owned().into();
    writer
        .commit(Mutation::insert([Quad::new(s, type_pred, class, graph)]))
        .expect("commit succeeds");
}

fn scoped_orphans(store: &Store) -> Vec<(String, String)> {
    let sess = IRI_DEC_SESSION;
    let goal = IRI_DEC_GOAL;
    let disp = IRI_DEC_DISPATCH;
    let evt = IRI_DEC_EVENT;
    let in_str = IRI_DEC_IN_STREAM;
    let vs = IRI_DEC_VALUE_STREAM;
    let q = format!(
        "SELECT ?a ?cls WHERE {{ \
           VALUES ?cls {{ <{sess}> <{goal}> <{disp}> <{evt}> }} \
           GRAPH ?g {{ ?a a ?cls }} \
           FILTER NOT EXISTS {{ \
             GRAPH ?h {{ ?a <{in_str}> ?stream }} \
             GRAPH ?h2 {{ ?stream a <{vs}> }} \
           }} \
         }}",
    );
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("audit query") else {
        panic!("expected solutions");
    };
    for sol in sols {
        let sol = sol.expect("solution");
        let a = term_str(sol.get("a").expect("a present"));
        let cls = term_str(sol.get("cls").expect("cls present"));
        out.push((a, cls));
    }
    out
}

fn term_str(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::BlankNode(b) => format!("_:{}", b.as_str()),
        Term::Literal(l) => l.value().to_string(),
        Term::Triple(_) => "<<triple>>".to_string(),
    }
}

fn stream_for(store: &Store, subject: &NamedNode) -> Option<NamedNode> {
    let pred = NamedNodeRef::new(IRI_DEC_IN_STREAM).expect("pred");
    let mut iter = store.quads_for_pattern(
        Some(Subject::NamedNode(subject.clone()).as_ref()),
        Some(pred),
        None,
        None,
    );
    let quad = iter.next()?.expect("quad ok");
    if let Term::NamedNode(s) = quad.object {
        Some(s)
    } else {
        None
    }
}
