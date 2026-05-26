//! TC-166 — OntologyDescription enforces the single-active invariant.
//!
//! Validates: FT-101 · ADR-066.
//! Spec: `.product/tests/TC-166-ontologydescription-enforces-single-active-invaria.md`
//!
//! A parallel non-superseded OntologyDescription write is refused by
//! the catalog SHACL validator wired into the `StreamWriter` chokepoint.

use std::sync::Arc;

use decision_cli::core::ontology::catalog::OntologyDescription;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-166";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn od(id: &str, version: &str) -> OntologyDescription {
    OntologyDescription {
        id: id.to_string(),
        namespace: "https://decision-cli.dev/ns#".to_string(),
        prefix: "dec".to_string(),
        ontology_version: version.to_string(),
        body: r#"{"namespace":"https://decision-cli.dev/ns#","classes":[]}"#.to_string(),
        supersedes: None,
    }
}

#[test]
fn tc_166_ontologydescription_enforces_single_active_invariant() {
    scenario_a_first_ontology_description_succeeds();
    scenario_b_parallel_active_write_is_rejected();
    scenario_c_supersession_unblocks_a_fresh_active_author();
}

#[test]
fn scenario_a_first_ontology_description_succeeds() {
    let (store, w) = writer();
    let first = od("OD-001", "0.3.0");
    commit(&w, first.to_quads()).expect("first OD commits cleanly");
    assert_eq!(active_ods(&store), vec!["https://decision-cli.dev/ns/od/OD-001".to_string()]);
}

#[test]
fn scenario_b_parallel_active_write_is_rejected() {
    let (store, w) = writer();
    let first = od("OD-001", "0.3.0");
    commit(&w, first.to_quads()).expect("first OD commits");

    let second = od("OD-002", "0.3.1");
    let err =
        commit(&w, second.to_quads()).expect_err("parallel active OD must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "error must be tagged as SHACL violation; got: {err}"
    );
    assert!(
        err.contains("single-active") || err.contains("OntologyDescription"),
        "error must reference the single-active rule or OntologyDescription; got: {err}"
    );

    // The active set must remain unchanged.
    assert_eq!(active_ods(&store), vec!["https://decision-cli.dev/ns/od/OD-001".to_string()]);
}

#[test]
fn scenario_c_supersession_unblocks_a_fresh_active_author() {
    let (store, w) = writer();
    let first = od("OD-001", "0.3.0");
    commit(&w, first.to_quads()).expect("first OD commits");

    let second = OntologyDescription {
        supersedes: Some("OD-001".to_string()),
        ..od("OD-002", "0.3.1")
    };
    commit(&w, second.to_quads()).expect("OD-002 superseding OD-001 commits");

    assert_eq!(active_ods(&store), vec!["https://decision-cli.dev/ns/od/OD-002".to_string()]);
    let all = all_ods(&store);
    assert!(all.contains(&"https://decision-cli.dev/ns/od/OD-001".to_string()));
    assert!(all.contains(&"https://decision-cli.dev/ns/od/OD-002".to_string()));
}

// ---------------------------------------------------------------------------

fn active_ods(store: &Store) -> Vec<String> {
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               GRAPH <https://decision-cli.dev/ns/graph/catalog> { \
                 ?s a dec:OntologyDescription . \
                 FILTER NOT EXISTS { ?s dec:supersededBy ?_ } \
               } \
             } ORDER BY ?s";
    let mut out = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(q) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get(0) {
                out.push(n.as_str().to_string());
            }
        }
    }
    out
}

fn all_ods(store: &Store) -> Vec<String> {
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               GRAPH <https://decision-cli.dev/ns/graph/catalog> { \
                 ?s a dec:OntologyDescription \
               } \
             } ORDER BY ?s";
    let mut out = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(q) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get(0) {
                out.push(n.as_str().to_string());
            }
        }
    }
    out
}
