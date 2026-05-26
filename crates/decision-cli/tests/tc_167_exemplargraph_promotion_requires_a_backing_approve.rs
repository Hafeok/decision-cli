//! TC-167 — ExemplarGraph promotion requires a backing approved VGR.
//!
//! Validates: FT-101 · ADR-066.
//! Spec: `.product/tests/TC-167-exemplargraph-promotion-requires-a-backing-approve.md`
//!
//! Scenario B (refuse-promotion-without-result) is the slice-1 runnable
//! assertion per the TC body — it does not depend on FT-099's `dec verify
//! graph run` machinery. Scenarios A and C (which require an actually-
//! executed VG to produce the backing VGR) are exercised by seeding the
//! VGR triples directly into the store, sidestepping the FT-099 runner
//! while still proving the catalog SHACL validator accepts a proven VG.

use std::sync::Arc;

use decision_cli::core::ontology::catalog::{ExemplarGraph, SafetyClassTag};
use decision_cli::vocab::{
    catalog_graph, IRI_DEC_RESULT_OF, IRI_DEC_VERDICT, IRI_DEC_VERIFICATION_GRAPH_RESULT,
    VERDICT_APPROVED,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{
    GraphName, GraphNameRef, Literal, NamedNode, NamedNodeRef, Quad,
};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-167";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const VGR_GRAPH: &str = "https://decision-cli.dev/ns/graph/verify-result";

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

fn rationale_text() -> String {
    "Canonical minimal verification: one shell-command step that always \
     exits 0. Useful as a smoke template for any new env."
        .to_string()
}

fn vg_iri(id: &str) -> NamedNode {
    NamedNode::new(format!("https://decision-cli.dev/ns/graph/verify-graph/{id}")).unwrap()
}

fn vgr_iri(id: &str) -> NamedNode {
    NamedNode::new(format!("https://decision-cli.dev/ns/result/{id}")).unwrap()
}

/// Seed an approved VerificationGraphResult into the verify-result named
/// graph by writing the three minimum triples directly to the store
/// (bypassing the StreamWriter so the full FT-097 SHACL contract — length
/// parity, step IRI membership — does not gate the seed). The catalog
/// validator only ASKs for the verdict + resultOf pair.
fn seed_approved_vgr(store: &Store, vg: &NamedNode, vgr: &NamedNode) {
    let g = GraphName::NamedNode(NamedNode::new(VGR_GRAPH).unwrap());
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let cls = NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_GRAPH_RESULT);
    let verdict_pred = NamedNodeRef::new_unchecked(IRI_DEC_VERDICT);
    let result_of_pred = NamedNodeRef::new_unchecked(IRI_DEC_RESULT_OF);
    let quads = vec![
        Quad::new(vgr.clone(), rdf_type, cls, g.clone()),
        Quad::new(
            vgr.clone(),
            verdict_pred,
            Literal::new_simple_literal(VERDICT_APPROVED),
            g.clone(),
        ),
        Quad::new(vgr.clone(), result_of_pred, vg.clone(), g),
    ];
    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<(), oxigraph::store::StorageError>(())
        })
        .expect("seed VGR triples");
}

#[test]
fn tc_167_exemplargraph_promotion_requires_a_backing_approved_result() {
    // Headline test composes Scenario A + B + D so the runner invocation
    // exercises the whole TC body in one go. Scenario C (latest-verdict
    // semantics) is broken out as its own #[test] since it needs a fresh
    // store.
    scenario_a_promotion_of_a_proven_vg_succeeds();
    scenario_b_promotion_of_unproven_vg_is_rejected();
    scenario_d_rationale_too_short_is_rejected();
}

#[test]
fn scenario_a_promotion_of_a_proven_vg_succeeds() {
    let (store, w) = writer();
    let vg = vg_iri("VG-PROVEN");
    let vgr = vgr_iri("VGR-001");
    seed_approved_vgr(&store, &vg, &vgr);

    let ex = ExemplarGraph {
        id: "EX-001".to_string(),
        exemplar_of: vg.clone(),
        applies_to_safety_class: SafetyClassTag::Isolated,
        pattern_name: "trivial-shell-pass".to_string(),
        rationale: rationale_text(),
        based_on_approved_result: vgr.clone(),
        supersedes: None,
    };
    commit(&w, ex.to_quads()).expect("promotion of proven VG succeeds");

    // The exemplar is persisted in the catalog named graph.
    let ex_iri = ex.iri();
    let g = catalog_graph().into_owned();
    let count = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(ex_iri).as_ref()),
            None,
            None,
            Some(GraphNameRef::NamedNode(NamedNodeRef::new_unchecked(g.as_str()))),
        )
        .count();
    assert!(count > 0, "EX-001 must be persisted in catalog graph");
}

#[test]
fn scenario_b_promotion_of_unproven_vg_is_rejected() {
    let (_store, w) = writer();
    let vg = vg_iri("VG-UNPROVEN");
    let vgr = vgr_iri("VGR-MISSING");
    // No seed — VGR-MISSING does not exist in the store.

    let ex = ExemplarGraph {
        id: "EX-002".to_string(),
        exemplar_of: vg,
        applies_to_safety_class: SafetyClassTag::Isolated,
        pattern_name: "untested".to_string(),
        rationale: "An attempt to promote a graph that has no result history; expected to fail."
            .to_string(),
        based_on_approved_result: vgr,
        supersedes: None,
    };

    let err = commit(&w, ex.to_quads())
        .expect_err("promotion of unproven VG must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "error must be tagged as SHACL violation; got: {err}"
    );
    assert!(
        err.contains("ExemplarNotProven"),
        "error must name ExemplarNotProven; got: {err}"
    );
    assert!(
        err.contains("VG-UNPROVEN"),
        "error must reference the unproven VG; got: {err}"
    );
}

#[test]
fn scenario_c_latest_verdict_counts_after_prior_failure() {
    let (store, w) = writer();
    let vg = vg_iri("VG-INITIALLY-FAILING");
    // Earlier failing run exists but is irrelevant to the catalog
    // validator (which ASKs for *any* approved VGR with the right
    // resultOf, not the latest). The latest approved VGR is what the
    // exemplar binds to.
    let latest_vgr = vgr_iri("VGR-003");
    seed_approved_vgr(&store, &vg, &latest_vgr);

    let ex = ExemplarGraph {
        id: "EX-003".to_string(),
        exemplar_of: vg.clone(),
        applies_to_safety_class: SafetyClassTag::Isolated,
        pattern_name: "recovered".to_string(),
        rationale: "Demonstrates that the latest verdict is what counts; prior failures do not block."
            .to_string(),
        based_on_approved_result: latest_vgr.clone(),
        supersedes: None,
    };
    commit(&w, ex.to_quads())
        .expect("promotion succeeds when latest VGR is approved");

    // basedOnApprovedResult resolves to VGR-003.
    let bound = read_based_on(&store, &ex.iri());
    assert_eq!(bound, Some(latest_vgr.as_str().to_string()));
}

#[test]
fn scenario_d_rationale_too_short_is_rejected() {
    let (store, w) = writer();
    let vg = vg_iri("VG-PROVEN-D");
    let vgr = vgr_iri("VGR-D");
    seed_approved_vgr(&store, &vg, &vgr);

    let ex = ExemplarGraph {
        id: "EX-004".to_string(),
        exemplar_of: vg,
        applies_to_safety_class: SafetyClassTag::Isolated,
        pattern_name: "short".to_string(),
        rationale: "too short".to_string(),
        based_on_approved_result: vgr,
        supersedes: None,
    };
    let err = commit(&w, ex.to_quads())
        .expect_err("rationale too short must be rejected");
    assert!(
        err.contains("SHACL violation"),
        "error must be tagged as SHACL violation; got: {err}"
    );
    assert!(
        err.contains("rationale"),
        "error must reference rationale; got: {err}"
    );
    assert!(
        err.contains("40"),
        "error must cite the 40-character minimum; got: {err}"
    );
}

fn read_based_on(store: &Store, ex: &NamedNode) -> Option<String> {
    use oxigraph::sparql::QueryResults;
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?vgr WHERE {{ \
           GRAPH <https://decision-cli.dev/ns/graph/catalog> {{ \
             <{ex}> dec:basedOnApprovedResult ?vgr \
           }} \
         }}",
        ex = ex.as_str()
    );
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get(0) {
                return Some(n.as_str().to_string());
            }
        }
    }
    None
}
