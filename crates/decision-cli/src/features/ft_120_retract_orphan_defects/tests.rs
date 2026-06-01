//! Tests for FT-120 — orphan-defect retraction.
//!
//! Test criteria: TC-260, TC-261, TC-264.

use std::sync::Arc;

use oxigraph::model::{BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::read_prior_state;
use crate::core::vocab::{lifecycle_state, orchestration_graph};
use crate::core::StreamWriter;

use super::query::{
    find_orphan_defects_all, find_orphan_defects_for_feature, find_orphan_defects_for_graph,
};
use super::transition::retract_orphan;

const DEC_NS: &str = "https://decision-cli.dev/ns#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
const PROV_NS: &str = "http://www.w3.org/ns/prov#";

fn dec(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{DEC_NS}{local}"))
}

fn seed_feedback(
    store: &Store,
    feedback_iri: &str,
    tc_iri: &str,
    session_iri: &str,
    state: LifecycleState,
) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let fb = NamedNode::new_unchecked(feedback_iri);

    store
        .extend(vec![
            Quad::new(
                fb.clone(),
                NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
                dec("Feedback"),
                g.clone(),
            ),
            Quad::new(
                fb.clone(),
                dec("sourceArtifact"),
                NamedNode::new_unchecked(tc_iri),
                g.clone(),
            ),
            Quad::new(
                fb.clone(),
                dec("sourceSession"),
                NamedNode::new_unchecked(session_iri),
                g.clone(),
            ),
            Quad::new(
                fb,
                lifecycle_state().into_owned(),
                Literal::new_simple_literal(state.as_str()),
                g,
            ),
        ])
        .unwrap();
}

fn seed_vgr(store: &Store, vgr_iri: &str, vg_iri: &str, session_iri: &str) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let vgr = NamedNode::new_unchecked(vgr_iri);

    store
        .extend(vec![
            Quad::new(
                vgr.clone(),
                NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
                dec("VerificationGraphResult"),
                g.clone(),
            ),
            Quad::new(
                vgr.clone(),
                dec("resultOf"),
                NamedNode::new_unchecked(vg_iri),
                g.clone(),
            ),
            Quad::new(
                vgr,
                NamedNode::new_unchecked(format!("{PROV_NS}wasGeneratedBy")),
                NamedNode::new_unchecked(session_iri),
                g,
            ),
        ])
        .unwrap();
}

/// Seed a VG with a step list. `step_provides` is (step_iri, tc_iri) pairs.
fn seed_vg_with_steps(store: &Store, vg_iri: &str, step_provides: &[(&str, &str)]) {
    let g: GraphName = orchestration_graph().into_owned().into();
    let vg = NamedNode::new_unchecked(vg_iri);

    let mut quads = vec![Quad::new(
        vg.clone(),
        NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
        dec("VerificationGraph"),
        g.clone(),
    )];

    if step_provides.is_empty() {
        quads.push(Quad::new(
            vg,
            dec("steps"),
            NamedNode::new_unchecked(RDF_NIL),
            g,
        ));
        store.extend(quads).unwrap();
        return;
    }

    // Build the rdf:List head -> rdf:first step -> rdf:rest -> ... -> rdf:nil
    let head = BlankNode::new_unchecked("list-head-0");
    quads.push(Quad::new(vg, dec("steps"), head.clone(), g.clone()));

    for (i, (step_iri, tc_iri)) in step_provides.iter().enumerate() {
        let node = if i == 0 {
            head.clone()
        } else {
            BlankNode::new_unchecked(format!("list-head-{i}"))
        };

        let step = NamedNode::new_unchecked(*step_iri);
        quads.push(Quad::new(
            node.clone(),
            NamedNodeRef::new_unchecked(RDF_FIRST).into_owned(),
            step.clone(),
            g.clone(),
        ));

        // Step type assertion + providesEvidenceFor
        quads.push(Quad::new(
            step.clone(),
            NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
            dec("VerificationStep"),
            g.clone(),
        ));
        quads.push(Quad::new(
            step,
            dec("providesEvidenceFor"),
            NamedNode::new_unchecked(*tc_iri),
            g.clone(),
        ));

        let rest = if i + 1 == step_provides.len() {
            NamedNode::new_unchecked(RDF_NIL).into()
        } else {
            oxigraph::model::Term::BlankNode(BlankNode::new_unchecked(format!("list-head-{}", i + 1)))
        };
        let rest_object: oxigraph::model::Term = rest;
        quads.push(Quad::new(
            node,
            NamedNodeRef::new_unchecked(RDF_REST).into_owned(),
            rest_object,
            g.clone(),
        ));
    }

    store.extend(quads).unwrap();
}

// ----------------------------------------------------------------------
// TC-260 — orphan SPARQL query identifies orphaned defects
// ----------------------------------------------------------------------

#[test]
fn tc_260_orphan_query_identifies_candidates() {
    let store = Store::new().unwrap();

    let vg_iri = "https://decision-cli.dev/ns/verify/graph/VG-001";
    let tc_covered = "https://decision-cli.dev/ns/test/TC-001";
    let tc_orphaned = "https://decision-cli.dev/ns/test/TC-002";
    let step_iri = "https://decision-cli.dev/ns/verify/step/STEP-001";
    let session_iri = "https://test/session-1";

    // VG with one step covering only TC-001
    seed_vg_with_steps(&store, vg_iri, &[(step_iri, tc_covered)]);
    seed_vgr(
        &store,
        "https://decision-cli.dev/ns/result/VGR-001",
        vg_iri,
        session_iri,
    );

    // Open defect against TC-covered (not orphan)
    seed_feedback(
        &store,
        "https://test/fb-covered",
        tc_covered,
        session_iri,
        LifecycleState::Produced,
    );
    // Open defect against TC-orphan (orphan)
    seed_feedback(
        &store,
        "https://test/fb-orphan",
        tc_orphaned,
        session_iri,
        LifecycleState::Produced,
    );
    // Terminal-state defect against TC-orphan (must not be returned)
    seed_feedback(
        &store,
        "https://test/fb-terminal",
        tc_orphaned,
        session_iri,
        LifecycleState::Closed,
    );

    let orphans = find_orphan_defects_all(&store).unwrap();

    assert_eq!(orphans.len(), 1, "exactly one orphan expected");
    assert_eq!(orphans[0].feedback_iri.as_str(), "https://test/fb-orphan");
    assert_eq!(orphans[0].tc_iri, tc_orphaned);
    assert_eq!(orphans[0].vg_iri, vg_iri);

    // Per-graph scope returns the same single result.
    let scoped = find_orphan_defects_for_graph(&store, vg_iri).unwrap();
    assert_eq!(scoped.len(), 1);

    // A different graph IRI returns nothing.
    let other = find_orphan_defects_for_graph(
        &store,
        "https://decision-cli.dev/ns/verify/graph/VG-OTHER",
    )
    .unwrap();
    assert!(other.is_empty());
}

#[test]
fn tc_260_empty_vg_step_list_marks_all_defects_orphan() {
    let store = Store::new().unwrap();

    let vg_iri = "https://decision-cli.dev/ns/verify/graph/VG-002";
    let tc_iri = "https://decision-cli.dev/ns/test/TC-X";
    let session_iri = "https://test/session-2";

    seed_vg_with_steps(&store, vg_iri, &[]);
    seed_vgr(
        &store,
        "https://decision-cli.dev/ns/result/VGR-002",
        vg_iri,
        session_iri,
    );
    seed_feedback(
        &store,
        "https://test/fb-empty",
        tc_iri,
        session_iri,
        LifecycleState::Produced,
    );

    let orphans = find_orphan_defects_all(&store).unwrap();
    assert_eq!(orphans.len(), 1);
}

#[test]
fn tc_260_non_vgr_session_not_returned() {
    // Defects whose sourceSession is not a VGR's wasGeneratedBy
    // session must not be returned as orphans.
    let store = Store::new().unwrap();
    let tc_iri = "https://decision-cli.dev/ns/test/TC-Z";
    let implementer_session = "https://test/implementer-session";

    seed_feedback(
        &store,
        "https://test/fb-impl",
        tc_iri,
        implementer_session,
        LifecycleState::Produced,
    );
    // No VGR seeded — the FILTER NOT EXISTS in the query never finds a VGR
    // wired to this session.

    let orphans = find_orphan_defects_all(&store).unwrap();
    assert!(orphans.is_empty(), "implementer-emitted defect must not be orphan");
}

// ----------------------------------------------------------------------
// TC-261 — transition writes produced → superseded with the new predicate
// ----------------------------------------------------------------------

fn setup_writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().unwrap());
    let stream_iri = NamedNode::new_unchecked("https://test/stream");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream_iri).unwrap();
    (store, writer)
}

#[test]
fn tc_261_produced_to_superseded() {
    let (store, writer) = setup_writer();
    let fb_iri = "https://test/fb-1";
    seed_feedback(
        &store,
        fb_iri,
        "https://test/tc",
        "https://test/session",
        LifecycleState::Produced,
    );

    let fb = NamedNode::new_unchecked(fb_iri);
    let did = retract_orphan(
        &store,
        &writer,
        &fb,
        "https://test/retraction-session",
        "https://test/vg",
        "https://test/tc",
    )
    .unwrap();

    assert!(did, "transition should be applied");
    assert_eq!(
        read_prior_state(&store, &fb).unwrap(),
        LifecycleState::Superseded
    );
}

#[test]
fn tc_261_routed_to_superseded() {
    for from in [LifecycleState::Routed] {
        let (store, writer) = setup_writer();
        let fb_iri = format!("https://test/fb-{}", from.as_str());
        seed_feedback(
            &store,
            &fb_iri,
            "https://test/tc",
            "https://test/session",
            from,
        );

        let fb = NamedNode::new_unchecked(&fb_iri);
        let did = retract_orphan(
            &store,
            &writer,
            &fb,
            "https://test/retract-session",
            "https://test/vg",
            "https://test/tc",
        )
        .unwrap();
        assert!(did, "transition from {} should be applied", from.as_str());
        assert_eq!(
            read_prior_state(&store, &fb).unwrap(),
            LifecycleState::Superseded
        );
    }
}

#[test]
fn tc_261_predicate_set_with_subpropertyof_relationship() {
    let (store, writer) = setup_writer();
    let fb_iri = "https://test/fb-pred";
    seed_feedback(
        &store,
        fb_iri,
        "https://test/tc",
        "https://test/session",
        LifecycleState::Produced,
    );

    let fb = NamedNode::new_unchecked(fb_iri);
    retract_orphan(
        &store,
        &writer,
        &fb,
        "https://test/retract-session",
        "https://test/vg",
        "https://test/tc",
    )
    .unwrap();

    let predicates = ["supersededByTopologyChange", "supersededAt", "supersededReason"];
    for pred_local in predicates {
        let pred = NamedNodeRef::new_unchecked(&format!("{DEC_NS}{pred_local}")).into_owned();
        let count = store
            .quads_for_pattern(Some(fb.as_ref().into()), Some(pred.as_ref()), None, None)
            .filter_map(Result::ok)
            .count();
        assert_eq!(count, 1, "expected exactly one {pred_local} triple");
    }
}

#[test]
fn tc_261_terminal_state_skipped() {
    let (store, writer) = setup_writer();
    for state in [
        LifecycleState::Closed,
        LifecycleState::Rejected,
        LifecycleState::Superseded,
    ] {
        let fb_iri = format!("https://test/fb-terminal-{}", state.as_str());
        seed_feedback(
            &store,
            &fb_iri,
            "https://test/tc",
            "https://test/session",
            state,
        );

        let fb = NamedNode::new_unchecked(&fb_iri);
        let did = retract_orphan(
            &store,
            &writer,
            &fb,
            "https://test/retract-session",
            "https://test/vg",
            "https://test/tc",
        )
        .unwrap();

        assert!(!did, "terminal state {} must be skipped", state.as_str());
        assert_eq!(read_prior_state(&store, &fb).unwrap(), state);
    }
}

// ----------------------------------------------------------------------
// TC-264 — pipeline retracts orphans idempotently
// ----------------------------------------------------------------------

#[test]
fn tc_264_pipeline_retracts_orphans_idempotent() {
    let (store, writer) = setup_writer();

    let vg_iri = "https://decision-cli.dev/ns/verify/graph/VG-264";
    let tc_orphan = "https://decision-cli.dev/ns/test/TC-264-orphan";
    let tc_covered = "https://decision-cli.dev/ns/test/TC-264-covered";
    let step_iri = "https://decision-cli.dev/ns/verify/step/STEP-264";
    let session_iri = "https://test/session-264";

    seed_vg_with_steps(&store, vg_iri, &[(step_iri, tc_covered)]);
    seed_vgr(
        &store,
        "https://decision-cli.dev/ns/result/VGR-264",
        vg_iri,
        session_iri,
    );

    let fb_orphan = "https://test/fb-264-orphan";
    let fb_covered = "https://test/fb-264-covered";
    seed_feedback(
        &store,
        fb_orphan,
        tc_orphan,
        session_iri,
        LifecycleState::Produced,
    );
    seed_feedback(
        &store,
        fb_covered,
        tc_covered,
        session_iri,
        LifecycleState::Produced,
    );

    // First pass: query → retract
    let candidates = find_orphan_defects_for_graph(&store, vg_iri).unwrap();
    assert_eq!(candidates.len(), 1, "exactly one orphan candidate");

    let mut retracted = 0;
    for cand in &candidates {
        if retract_orphan(
            &store,
            &writer,
            &cand.feedback_iri,
            &cand.session_iri,
            &cand.vg_iri,
            &cand.tc_iri,
        )
        .unwrap()
        {
            retracted += 1;
        }
    }
    assert_eq!(retracted, 1);

    // The orphan is now superseded.
    let fb_orphan_node = NamedNode::new_unchecked(fb_orphan);
    assert_eq!(
        read_prior_state(&store, &fb_orphan_node).unwrap(),
        LifecycleState::Superseded
    );
    // The covered one is untouched.
    let fb_covered_node = NamedNode::new_unchecked(fb_covered);
    assert_eq!(
        read_prior_state(&store, &fb_covered_node).unwrap(),
        LifecycleState::Produced
    );

    // Second pass: query finds nothing (idempotent).
    let candidates2 = find_orphan_defects_for_graph(&store, vg_iri).unwrap();
    assert!(candidates2.is_empty(), "second pass should find no orphans");
}

#[test]
fn tc_264_feature_scope_filters_correctly() {
    let (store, _writer) = setup_writer();

    // FT-A links to VG-A, FT-B links to VG-B. Each has one orphan defect.
    let g: GraphName = orchestration_graph().into_owned().into();

    let ft_a_iri = "https://decision-cli.dev/ns/feature/FT-A";
    let vg_a_iri = "https://decision-cli.dev/ns/verify/graph/VG-A";
    let vg_b_iri = "https://decision-cli.dev/ns/verify/graph/VG-B";
    let tc_a_iri = "https://decision-cli.dev/ns/test/TC-A";
    let tc_b_iri = "https://decision-cli.dev/ns/test/TC-B";

    // FT-A verifies on VG-A (no steps → all defects orphan).
    seed_vg_with_steps(&store, vg_a_iri, &[]);
    seed_vg_with_steps(&store, vg_b_iri, &[]);
    store
        .extend(vec![Quad::new(
            NamedNode::new_unchecked(vg_a_iri),
            dec("verifies"),
            NamedNode::new_unchecked(ft_a_iri),
            g.clone(),
        )])
        .unwrap();
    // VG-B is unrelated to FT-A.

    seed_vgr(
        &store,
        "https://decision-cli.dev/ns/result/VGR-A",
        vg_a_iri,
        "https://test/session-a",
    );
    seed_vgr(
        &store,
        "https://decision-cli.dev/ns/result/VGR-B",
        vg_b_iri,
        "https://test/session-b",
    );

    seed_feedback(
        &store,
        "https://test/fb-a",
        tc_a_iri,
        "https://test/session-a",
        LifecycleState::Produced,
    );
    seed_feedback(
        &store,
        "https://test/fb-b",
        tc_b_iri,
        "https://test/session-b",
        LifecycleState::Produced,
    );

    let scoped = find_orphan_defects_for_feature(&store, ft_a_iri).unwrap();
    assert_eq!(scoped.len(), 1, "only FT-A's orphan should match");
    assert_eq!(scoped[0].vg_iri, vg_a_iri);
}
