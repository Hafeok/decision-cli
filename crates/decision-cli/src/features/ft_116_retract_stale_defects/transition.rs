//! Apply the `closed` lifecycle transition for retracted defects.

use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::{read_prior_state, ApplyError};
use crate::core::vocab::{lifecycle_state, orchestration_graph};
use crate::core::StreamWriter;

/// Evidence quads for the retraction-based close.
pub fn build_retraction_evidence(
    feedback_iri: &NamedNode,
    vgr_iri: &str,
    graph: NamedNodeRef<'_>,
) -> Vec<Quad> {
    let g: GraphName = graph.into_owned().into();
    let now = Utc::now().to_rfc3339();
    let reason = format!("evidence retracted by approved {}", extract_vgr_id(vgr_iri));

    vec![
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#closedByEvidenceRetraction")
                .into_owned(),
            NamedNode::new_unchecked(vgr_iri),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#closedAt").into_owned(),
            Literal::new_typed_literal(
                now,
                NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            ),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#closedReason").into_owned(),
            Literal::new_simple_literal(reason),
            g,
        ),
    ]
}

/// Extract the short VGR id from an IRI (e.g., "VGR-123" from "https://.../VGR-123").
fn extract_vgr_id(iri: &str) -> &str {
    iri.rsplit('/').next().unwrap_or(iri)
}

/// Apply the close transition to a single feedback artifact.
///
/// This validates the prior state, builds the mutation with lifecycle state
/// change and evidence quads, and commits through the writer.
pub fn close_feedback(
    store: &Store,
    writer: &StreamWriter,
    feedback_iri: &NamedNode,
    vgr_iri: &str,
) -> Result<(), ApplyError> {
    // Read and validate the prior state
    let prior = read_prior_state(store, feedback_iri)?;

    // Only transition from non-terminal states
    if prior.is_terminal() {
        // Silently skip terminal states - this is idempotent behavior
        return Ok(());
    }

    // Build the mutation
    let graph = orchestration_graph();
    let g: GraphName = graph.into_owned().into();

    // Remove prior lifecycle state
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

    // Insert new state + evidence
    // Use Superseded (not Closed) because ADR-024 allows produced → superseded
    // but not produced → closed. New passing evidence supersedes old failing evidence.
    let mut inserts = vec![Quad::new(
        feedback_iri.clone(),
        lifecycle_pred.into_owned(),
        Literal::new_simple_literal(LifecycleState::Superseded.as_str()),
        g.clone(),
    )];
    inserts.extend(build_retraction_evidence(feedback_iri, vgr_iri, graph));

    let mutation = Mutation {
        inserts,
        removes,
        ..Mutation::default()
    };

    writer
        .commit(mutation)
        .map_err(|e| ApplyError::Store(format!("{e:#}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_vgr_id_from_full_iri() {
        assert_eq!(
            extract_vgr_id("https://decision-cli.dev/ns/result/VGR-123"),
            "VGR-123"
        );
    }

    #[test]
    fn extract_vgr_id_from_short_form() {
        assert_eq!(extract_vgr_id("VGR-456"), "VGR-456");
    }

    #[test]
    fn build_retraction_evidence_creates_three_quads() {
        let fb = NamedNode::new_unchecked("https://test/fb");
        let vgr = "https://test/VGR-1";
        let graph = NamedNodeRef::new_unchecked("https://test/graph");

        let evidence = build_retraction_evidence(&fb, vgr, graph);

        assert_eq!(evidence.len(), 3);
        // Check predicates
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("closedByEvidenceRetraction")));
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("closedAt")));
        assert!(evidence
            .iter()
            .any(|q| q.predicate.as_str().ends_with("closedReason")));
    }
}
