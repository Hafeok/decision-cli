//! TC-165 — CapabilityReference enforces active-set command uniqueness;
//! supersession is the only path to evolve.
//!
//! Validates: FT-101 · ADR-066.
//! Spec: `.product/tests/TC-165-capabilityreference-enforces-active-set-command-un.md`
//!
//! Each scenario in the TC body lands as a `#[test]` here, exercising
//! the catalog SHACL validator through the `StreamWriter` chokepoint —
//! the same path the CLI verb would dispatch through.

use std::sync::Arc;

use decision_cli::core::ontology::catalog::{
    validate_quads_with_store, CapabilityReference,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-165";

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

fn cr(id: &str, command: &str, version: &str) -> CapabilityReference {
    CapabilityReference {
        id: id.to_string(),
        command: command.to_string(),
        capability_version: version.to_string(),
        body: r#"{"command":"placeholder","flags":[]}"#.to_string(),
        supersedes: None,
    }
}

/// Headline test pointed at by the TC frontmatter — composes scenarios A→D
/// inline so the runner only needs to invoke one function. Each scenario
/// is also broken out as its own `#[test]` below for `cargo test`
/// granularity.
#[test]
fn tc_165_capabilityreference_enforces_active_set_command_uniqueness() {
    scenario_a_first_author_succeeds();
    scenario_b_duplicate_active_command_is_rejected();
    scenario_c_supersession_unblocks_a_fresh_active_author();
    scenario_d_supersession_cycle_is_rejected();
}

#[test]
fn scenario_a_first_author_succeeds() {
    let (store, w) = writer();
    let r = cr("CR-001", "dec verify graph new", "0.3.0");
    commit(&w, r.to_quads()).expect("first CR commits cleanly");
    let iri = r.iri();
    let exists = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(iri).as_ref()),
            None,
            None,
            None,
        )
        .next()
        .is_some();
    assert!(exists, "CR-001 must be persisted after a successful commit");
}

#[test]
fn scenario_b_duplicate_active_command_is_rejected() {
    let (_store, w) = writer();
    let first = cr("CR-001", "dec verify graph new", "0.3.0");
    commit(&w, first.to_quads()).expect("first CR commits");

    let dup = cr("CR-002", "dec verify graph new", "0.3.1");
    let err = commit(&w, dup.to_quads()).expect_err("duplicate active command must fail");
    assert!(
        err.contains("SHACL violation"),
        "error should be tagged as a SHACL violation; got: {err}"
    );
    assert!(
        err.contains("DuplicateActive"),
        "error must name DuplicateActive; got: {err}"
    );
    assert!(
        err.contains("CR-001"),
        "error must reference the existing CR-001; got: {err}"
    );
}

#[test]
fn scenario_c_supersession_unblocks_a_fresh_active_author() {
    let (store, w) = writer();
    let first = cr("CR-001", "dec verify graph new", "0.3.0");
    commit(&w, first.to_quads()).expect("first CR commits");

    // Single transaction writes the new CR + the supersession edges
    // (the to_quads() helper does this when `supersedes` is Some).
    let second = CapabilityReference {
        supersedes: Some("CR-001".to_string()),
        ..cr("CR-002", "dec verify graph new", "0.3.1")
    };
    commit(&w, second.to_quads()).expect("supersession of CR-001 by CR-002 commits");

    // Active-set query: only CR-002 should remain active.
    let active = active_capability_refs(&store);
    assert_eq!(active, vec!["https://decision-cli.dev/ns/cr/CR-002".to_string()]);

    // include-superseded: both visible.
    let all = all_capability_refs(&store);
    assert!(all.contains(&"https://decision-cli.dev/ns/cr/CR-001".to_string()));
    assert!(all.contains(&"https://decision-cli.dev/ns/cr/CR-002".to_string()));
}

#[test]
fn scenario_d_supersession_cycle_is_rejected() {
    let (_store, w) = writer();
    let first = cr("CR-001", "dec verify graph new", "0.3.0");
    commit(&w, first.to_quads()).expect("first CR commits");
    let second = CapabilityReference {
        supersedes: Some("CR-001".to_string()),
        ..cr("CR-002", "dec verify graph new", "0.3.1")
    };
    commit(&w, second.to_quads()).expect("supersession commits");

    // Now attempt to write the reverse edge — CR-001 supersedes CR-002
    // — which closes a 2-cycle (CR-001 → CR-002 → CR-001).
    let g = decision_cli::vocab::catalog_graph().into_owned();
    let p = NamedNode::new("https://decision-cli.dev/ns#supersedes").unwrap();
    let s = NamedNode::new("https://decision-cli.dev/ns/cr/CR-001").unwrap();
    let o = NamedNode::new("https://decision-cli.dev/ns/cr/CR-002").unwrap();
    let cycle_quad = Quad::new(s, p, o, g);

    let err = commit(&w, vec![cycle_quad]).expect_err("supersession cycle must fail");
    assert!(
        err.contains("SupersessionCycle") || err.contains("cycle"),
        "error must name SupersessionCycle / cycle; got: {err}"
    );
}

/// Sanity check: a malformed reference missing dec:capabilityBody is
/// rejected by the validator with no store needed.
#[test]
fn store_less_validator_rejects_missing_body() {
    let mut r = cr("CR-009", "dec status", "0.3.0");
    r.body = String::new(); // empty body still serialises a literal — drop it.
    let quads: Vec<Quad> = r
        .to_quads()
        .into_iter()
        .filter(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#capabilityBody")
        .collect();
    let err = validate_quads_with_store(&quads, None).expect_err("missing body must fail");
    assert!(err.report.contains("dec:capabilityBody"), "{err:?}");
}

// ---------------------------------------------------------------------------
// SPARQL helpers — pulled inline so the test file is self-contained.
// ---------------------------------------------------------------------------

fn active_capability_refs(store: &Store) -> Vec<String> {
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               GRAPH <https://decision-cli.dev/ns/graph/catalog> { \
                 ?s a dec:CapabilityReference . \
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

fn all_capability_refs(store: &Store) -> Vec<String> {
    use oxigraph::sparql::QueryResults;
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { \
               GRAPH <https://decision-cli.dev/ns/graph/catalog> { \
                 ?s a dec:CapabilityReference \
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
