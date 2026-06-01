//! Apply the `superseded` lifecycle transition for orphaned defects.

use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::{read_prior_state, ApplyError};
use crate::core::vocab::{lifecycle_state, orchestration_graph};
use crate::core::StreamWriter;

/// Evidence quads attached to the orphan retraction.
pub fn build_orphan_evidence(
    feedback_iri: &NamedNode,
    session_iri: &str,
    vg_iri: &str,
    tc_iri: &str,
    graph: NamedNodeRef<'_>,
) -> Vec<Quad> {
    let g: GraphName = graph.into_owned().into();
    let now = Utc::now().to_rfc3339();
    let reason = format!(
        "source VG {} no longer covers TC {} (topology change)",
        extract_short_id(vg_iri),
        extract_short_id(tc_iri),
    );

    vec![
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked(
                "https://decision-cli.dev/ns#supersededByTopologyChange",
            )
            .into_owned(),
            NamedNode::new_unchecked(session_iri),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#supersededAt").into_owned(),
            Literal::new_typed_literal(
                now,
                NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            ),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#supersededReason")
                .into_owned(),
            Literal::new_simple_literal(reason),
            g,
        ),
    ]
}

fn extract_short_id(iri: &str) -> &str {
    iri.rsplit('/').next().unwrap_or(iri)
}

/// Apply the orphan-retraction transition to a single feedback artifact.
///
/// Reads the prior state, refuses terminal states (idempotent skip),
/// builds the mutation, and commits through the writer.
pub fn retract_orphan(
    store: &Store,
    writer: &StreamWriter,
    feedback_iri: &NamedNode,
    session_iri: &str,
    vg_iri: &str,
    tc_iri: &str,
) -> Result<bool, ApplyError> {
    let prior = read_prior_state(store, feedback_iri)?;

    if prior.is_terminal() {
        return Ok(false);
    }

    let graph = orchestration_graph();
    let g: GraphName = graph.into_owned().into();
    let lifecycle_pred = lifecycle_state();

    let removes: Vec<Quad> = store
        .quads_for_pattern(
            Some(feedback_iri.as_ref().into()),
            Some(lifecycle_pred),
            None,
            None,
        )
        .filter_map(Result::ok)
        .collect();

    let mut inserts = vec![Quad::new(
        feedback_iri.clone(),
        lifecycle_pred.into_owned(),
        Literal::new_simple_literal(LifecycleState::Superseded.as_str()),
        g.clone(),
    )];
    inserts.extend(build_orphan_evidence(
        feedback_iri,
        session_iri,
        vg_iri,
        tc_iri,
        graph,
    ));

    let mutation = Mutation {
        inserts,
        removes,
        ..Mutation::default()
    };

    writer
        .commit(mutation)
        .map_err(|e| ApplyError::Store(format!("{e:#}")))?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_short_id_from_iri() {
        assert_eq!(
            extract_short_id("https://decision-cli.dev/ns/verify/graph/VG-088"),
            "VG-088"
        );
        assert_eq!(extract_short_id("VG-088"), "VG-088");
    }

    #[test]
    fn build_orphan_evidence_creates_three_quads_with_topology_predicate() {
        let fb = NamedNode::new_unchecked("https://test/fb");
        let session = "https://test/session";
        let vg = "https://test/VG-001";
        let tc = "https://test/TC-001";
        let graph = NamedNodeRef::new_unchecked("https://test/graph");

        let evidence = build_orphan_evidence(&fb, session, vg, tc, graph);

        assert_eq!(evidence.len(), 3);
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("supersededByTopologyChange")));
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("supersededAt")));
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("supersededReason")));
    }
}
