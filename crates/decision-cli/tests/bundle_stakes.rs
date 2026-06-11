//! TC-102 — Bundle.stakes enum constraint and default ladder.
//!
//! Validates: FT-056 · ADR-035.
//! Spec: `.product/tests/TC-102-bundle-stakes-enum-constraint-and-default-ladder.md`
//!
//! Four scenarios per the TC's acceptance bullets:
//!
//! 1. SHACL conformance — build bundles with each of the three valid
//!    stakes; assert SHACL passes. Build a bundle with `stakes =
//!    "critical"`; assert SHACL violation (enum). Build a bundle with
//!    `stakes` absent; assert SHACL violation (`sh:minCount 1`).
//! 2. Default ladder — every entry in ADR-035 §"Who sets it".
//! 3. Override path — `BundleBuilder::with_stakes(Stakes::Foundational)`
//!    overrides the ladder.
//! 4. Migration idempotency — `migrate_bundle_stakes` backfills routine
//!    on first run and is a no-op on re-runs.

use std::sync::Arc;

use decision_cli::bundle::{
    default_stakes_for, migrate_bundle_stakes, validate_quads, AdrScope, Bundle, BundleBuilder,
    FocalContext, Stakes,
};
use decision_cli::vocab::{
    bundle_graph, focal_pred, stakes_pred, IRI_DEC_BUNDLE, IRI_DEC_STAKES, STAKES_ROUTINE,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-102";

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

fn focal() -> NamedNode {
    NamedNode::new("https://decision-cli.dev/ns/capability/code-writer/v1").expect("focal iri")
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

// ---------------------------------------------------------------------------
// Scenario 1 — SHACL conformance for each value, plus enum + missing failures.
// ---------------------------------------------------------------------------

#[test]
fn well_formed_routine_bundle_passes_pure_shacl() {
    let b = BundleBuilder::new("hash-routine", focal())
        .with_stakes(Stakes::Routine)
        .build();
    let quads = b.to_quads(bundle_graph());
    validate_quads(&quads).expect("routine bundle must pass SHACL");
}

#[test]
fn well_formed_elevated_bundle_passes_pure_shacl() {
    let b = BundleBuilder::new("hash-elevated", focal())
        .with_stakes(Stakes::Elevated)
        .build();
    let quads = b.to_quads(bundle_graph());
    validate_quads(&quads).expect("elevated bundle must pass SHACL");
}

#[test]
fn well_formed_foundational_bundle_passes_pure_shacl() {
    let b = BundleBuilder::new("hash-foundational", focal())
        .with_stakes(Stakes::Foundational)
        .build();
    let quads = b.to_quads(bundle_graph());
    validate_quads(&quads).expect("foundational bundle must pass SHACL");
}

#[test]
fn well_formed_bundle_commits_through_stream_writer() {
    let (store, w) = writer();
    let b = BundleBuilder::new("hash-commit", focal())
        .with_stakes(Stakes::Routine)
        .build();
    commit_quads(&w, b.to_quads(bundle_graph())).expect("bundle commits");
    let n = store
        .quads_for_pattern(Some(Subject::NamedNode(b.iri()).as_ref()), None, None, None)
        .count();
    assert!(n > 0, "bundle must persist after commit");
}

#[test]
fn bundle_with_invalid_critical_stakes_is_rejected_pure_shacl() {
    let b = BundleBuilder::new("hash-critical", focal()).build();
    let mut quads = b.to_quads(bundle_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_STAKES {
            q.object = Literal::new_simple_literal("critical").into();
        }
    }
    let err = validate_quads(&quads).expect_err("critical stakes must fail");
    assert!(err.report.contains("critical"), "{}", err.report);
    assert!(err.report.contains("stakes"), "{}", err.report);
}

#[test]
fn bundle_with_invalid_stakes_is_rejected_via_stream_writer() {
    let (_store, w) = writer();
    let b = BundleBuilder::new("hash-critical-w", focal()).build();
    let mut quads = b.to_quads(bundle_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == stakes_pred().as_str() {
            q.object = Literal::new_simple_literal("critical").into();
        }
    }
    let err = commit_quads(&w, quads).expect_err("critical stakes must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("bundle"), "{err}");
}

#[test]
fn bundle_with_missing_stakes_is_rejected_pure_shacl() {
    let b = BundleBuilder::new("hash-missing", focal()).build();
    let quads = b.to_quads(bundle_graph());
    let pruned: Vec<Quad> = quads
        .into_iter()
        .filter(|q| q.predicate.as_str() != IRI_DEC_STAKES)
        .collect();
    let err = validate_quads(&pruned).expect_err("missing stakes must fail");
    assert!(err.report.contains("missing"), "{}", err.report);
    assert!(err.report.contains("stakes"), "{}", err.report);
}

#[test]
fn bundle_with_missing_stakes_is_rejected_via_stream_writer() {
    let (_store, w) = writer();
    let b = BundleBuilder::new("hash-missing-w", focal()).build();
    let quads: Vec<Quad> = b
        .to_quads(bundle_graph())
        .into_iter()
        .filter(|q| q.predicate.as_str() != stakes_pred().as_str())
        .collect();
    let err = commit_quads(&w, quads).expect_err("missing stakes must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("bundle"), "{err}");
}

// ---------------------------------------------------------------------------
// Scenario 2 — Default ladder per ADR-035 §"Who sets it".
// ---------------------------------------------------------------------------

#[test]
fn capability_focal_yields_foundational() {
    let cls = NamedNode::new("https://decision-cli.dev/ns#Capability").expect("class iri");
    let ctx = FocalContext::new(cls);
    assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
}

#[test]
fn role_binding_focal_yields_foundational() {
    let cls = NamedNode::new("https://decision-cli.dev/ns#RoleBinding").expect("class iri");
    let ctx = FocalContext::new(cls);
    assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
}

#[test]
fn ontology_change_focal_yields_foundational() {
    let cls = NamedNode::new("https://example.com/ns/OntologyChange").expect("class iri");
    let ctx = FocalContext::new(cls).ontology_change();
    assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
}

#[test]
fn new_artifact_type_focal_yields_foundational() {
    let cls = NamedNode::new("https://example.com/ns/Whatever").expect("class iri");
    let ctx = FocalContext::new(cls).new_artifact_type();
    assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
}

#[test]
fn feature_spec_with_two_cross_cutting_adrs_yields_elevated() {
    let cls = NamedNode::new("https://example.com/ns/FeatureSpec").expect("class iri");
    let ctx = FocalContext::new(cls)
        .with_adr_scope(AdrScope::CrossCutting)
        .with_adr_scope(AdrScope::CrossCutting);
    assert_eq!(default_stakes_for(&ctx), Stakes::Elevated);
}

#[test]
fn feature_spec_with_three_cross_cutting_adrs_yields_elevated() {
    let cls = NamedNode::new("https://example.com/ns/FeatureSpec").expect("class iri");
    let ctx = FocalContext::new(cls)
        .with_adr_scope(AdrScope::CrossCutting)
        .with_adr_scope(AdrScope::CrossCutting)
        .with_adr_scope(AdrScope::CrossCutting);
    assert_eq!(default_stakes_for(&ctx), Stakes::Elevated);
}

#[test]
fn cross_cutting_adr_focal_yields_elevated() {
    let cls = NamedNode::new("https://example.com/ns/Adr").expect("class iri");
    let ctx = FocalContext::new(cls).cross_cutting_adr();
    assert_eq!(default_stakes_for(&ctx), Stakes::Elevated);
}

#[test]
fn feature_spec_with_one_cross_cutting_adr_yields_routine() {
    let cls = NamedNode::new("https://example.com/ns/FeatureSpec").expect("class iri");
    let ctx = FocalContext::new(cls).with_adr_scope(AdrScope::CrossCutting);
    assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
}

#[test]
fn feature_spec_with_zero_cross_cutting_adrs_yields_routine() {
    let cls = NamedNode::new("https://example.com/ns/FeatureSpec").expect("class iri");
    let ctx = FocalContext::new(cls)
        .with_adr_scope(AdrScope::FeatureSpecific)
        .with_adr_scope(AdrScope::FeatureSpecific);
    assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
}

#[test]
fn unrecognised_class_falls_through_to_routine() {
    let cls = NamedNode::new("https://example.com/ns/UnknownThing").expect("class iri");
    let ctx = FocalContext::new(cls);
    assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
}

#[test]
fn default_ladder_is_pure_and_deterministic() {
    // Same input → same output across N calls.
    let ctx = FocalContext::new(
        NamedNode::new("https://decision-cli.dev/ns#Capability").expect("class iri"),
    );
    let runs: Vec<Stakes> = (0..5).map(|_| default_stakes_for(&ctx)).collect();
    for r in &runs {
        assert_eq!(*r, Stakes::Foundational);
    }
}

// ---------------------------------------------------------------------------
// Scenario 3 — Override path.
// ---------------------------------------------------------------------------

#[test]
fn builder_with_stakes_overrides_default() {
    // A focal artifact whose default would be Routine — overridden to
    // Foundational by the explicit call.
    let unknown_focal = NamedNode::new("https://example.com/focal/unknown").expect("focal iri");
    let b = BundleBuilder::new("hash-override", unknown_focal)
        .with_stakes(Stakes::Foundational)
        .build();
    assert_eq!(b.stakes, Stakes::Foundational);
}

#[test]
fn builder_without_override_uses_default_routine() {
    let b = BundleBuilder::new("hash-default", focal()).build();
    assert_eq!(b.stakes, Stakes::Routine);
}

#[test]
fn builder_with_stakes_overrides_for_each_value() {
    for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
        let b = BundleBuilder::new("hash-override-each", focal())
            .with_stakes(stakes)
            .build();
        assert_eq!(b.stakes, stakes);
    }
}

// ---------------------------------------------------------------------------
// Scenario 4 — Migration idempotency.
// ---------------------------------------------------------------------------

fn insert_pre_ft056_bundle(store: &Store, iri: &str) {
    let graph: GraphName = bundle_graph().into_owned().into();
    let subj = NamedNode::new(iri).expect("subject iri");
    let cls = NamedNode::new(IRI_DEC_BUNDLE).expect("class iri");
    let rdf_type =
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").expect("rdf:type");
    // Add the focal + rdf:type triples but no dec:stakes literal.
    let focal_iri = NamedNode::new("https://example.com/focal/pre-ft056").expect("focal");
    let q_type = Quad::new(subj.clone(), rdf_type, cls, graph.clone());
    let q_focal = Quad::new(subj, focal_pred().into_owned(), focal_iri, graph);
    store
        .transaction(|mut tx| {
            tx.insert(q_type.as_ref())?;
            tx.insert(q_focal.as_ref())
        })
        .expect("seed pre-FT056 bundle");
}

#[test]
fn migration_backfills_pre_existing_bundles_then_is_idempotent() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/pre-1");
    insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/pre-2");
    insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/pre-3");

    let first = migrate_bundle_stakes(&store, None).expect("migrate");
    assert_eq!(first.backfilled, 3, "first run backfills all 3");
    assert!(first.changed());

    // Every bundle now has dec:stakes "routine".
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?b ?s WHERE {{ \
           {{ ?b a <{cls}> ; <{stakes}> ?s }} \
           UNION \
           {{ GRAPH ?g {{ ?b a <{cls}> ; <{stakes}> ?s }} }} \
         }}",
        cls = IRI_DEC_BUNDLE,
        stakes = IRI_DEC_STAKES,
    );
    let QueryResults::Solutions(sols) = store.query(&q).expect("query") else {
        panic!("expected solutions");
    };
    let mut rows = 0;
    for sol in sols {
        let sol = sol.expect("ok");
        let Some(Term::Literal(lit)) = sol.get("s") else {
            panic!("expected literal");
        };
        assert_eq!(lit.value(), STAKES_ROUTINE);
        rows += 1;
    }
    assert_eq!(rows, 3);

    // Re-run — no-op.
    let second = migrate_bundle_stakes(&store, None).expect("re-run");
    assert_eq!(second.backfilled, 0);
    assert!(!second.changed());
}

#[test]
fn migration_on_empty_store_is_noop() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let outcome = migrate_bundle_stakes(&store, None).expect("migrate");
    assert_eq!(outcome.backfilled, 0);
}

#[test]
fn migration_through_stream_writer_passes_shacl() {
    // Pre-seed via direct store transaction (the writer is for migration's
    // own commit path).
    let store = Arc::new(Store::new().expect("in-memory store"));
    insert_pre_ft056_bundle(&store, "https://decision-cli.dev/ns/bundle/via-writer-1");

    // Seed the active stream so the writer bootstrap succeeds.
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let outcome = migrate_bundle_stakes(&store, Some(&w)).expect("migrate via writer");
    assert_eq!(outcome.backfilled, 1);
    // Re-run via writer.
    let outcome2 = migrate_bundle_stakes(&store, Some(&w)).expect("migrate via writer");
    assert_eq!(outcome2.backfilled, 0);
}

// ---------------------------------------------------------------------------
// Bonus: ontology and shapes exposure.
// ---------------------------------------------------------------------------

#[test]
fn embedded_shapes_declare_bundle_shape() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    let target = NamedNode::new(IRI_DEC_BUNDLE).expect("class iri");
    let mut has_shape = false;
    for q in h.shapes_graph() {
        if q.predicate.as_str() == "http://www.w3.org/ns/shacl#targetClass" {
            if let Term::NamedNode(n) = &q.object {
                if n == &target {
                    has_shape = true;
                    break;
                }
            }
        }
    }
    assert!(
        has_shape,
        "shapes.ttl must declare a sh:NodeShape with sh:targetClass dec:Bundle"
    );
}

#[test]
fn ontology_declares_bundle_class() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    assert!(
        h.declares_class("https://decision-cli.dev/ns#Bundle"),
        "ontology must declare dec:Bundle as rdfs:Class"
    );
}

#[test]
fn bundle_iri_matches_canonical_prefix() {
    let b: Bundle = BundleBuilder::new("hash-canonical", focal())
        .with_stakes(Stakes::Elevated)
        .build();
    assert_eq!(
        b.iri().as_str(),
        "https://decision-cli.dev/ns/bundle/hash-canonical"
    );
}

#[test]
fn stakes_try_from_str_round_trips() {
    for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
        let s = stakes.as_str();
        assert_eq!(Stakes::try_from_str(s).expect("parse"), stakes);
    }
    let err = Stakes::try_from_str("critical").expect_err("must reject");
    assert!(format!("{err}").contains("critical"));
}
