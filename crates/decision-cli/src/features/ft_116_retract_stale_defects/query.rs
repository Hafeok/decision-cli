//! SPARQL queries to find candidate stale defects for auto-close.

use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::ontology::verification_result::EvidenceProjection;

/// Result carrying the defects to close for a passing TC.
#[derive(Debug, Clone)]
pub struct StaleDefect {
    /// The feedback IRI to close.
    pub feedback_iri: NamedNode,
    /// The TC the feedback was about.
    pub tc_iri: String,
}

/// Find open defects from prior VGRs of `graph_iri` that should be closed
/// because the new `vgr_iri` shows the TC passing.
///
/// Returns one `StaleDefect` per open feedback found. The caller filters
/// by passing TCs (only close defects for TCs that flipped to pass).
pub fn find_stale_defects_for_vgr(
    store: &Store,
    graph_iri: &str,
    passing_tcs: &[&EvidenceProjection],
) -> Result<Vec<StaleDefect>, String> {
    let mut results = Vec::new();

    for projection in passing_tcs {
        // Only process passing TCs
        if projection.outcome.as_str() != "pass" {
            continue;
        }

        let tc_iri = &projection.tc;

        // Find open defects from this graph for this TC
        let q = format!(
            r#"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?feedback WHERE {{
  # Find feedback for this TC
  GRAPH ?g1 {{
    ?feedback a dec:Feedback ;
              dec:sourceArtifact <{tc_iri}> ;
              dec:lifecycleState ?state ;
              dec:sourceSession ?session .
  }}

  # Ensure the feedback came from a VGR session for this graph
  GRAPH ?g2 {{
    ?vgr a dec:VerificationGraphResult ;
         dec:resultOf <{graph_iri}> ;
         prov:wasGeneratedBy ?session .
  }}

  # Only open states (not terminal)
  FILTER (?state IN ("produced", "routed", "received"))
}}"#
        );

        let query_results = store.query(&q).map_err(|e| format!("SPARQL error: {e}"))?;

        if let QueryResults::Solutions(solutions) = query_results {
            for solution in solutions {
                let solution = solution.map_err(|e| format!("solution error: {e}"))?;
                if let Some(oxigraph::model::Term::NamedNode(fb_iri)) = solution.get("feedback") {
                    results.push(StaleDefect {
                        feedback_iri: fb_iri.clone(),
                        tc_iri: tc_iri.clone(),
                    });
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ontology::verification_result::StepOutcome;

    #[test]
    fn empty_store_returns_empty() {
        let store = Store::new().unwrap();
        let projections = vec![];
        let result = find_stale_defects_for_vgr(&store, "https://test/graph", &projections);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn passing_tc_with_no_defects_returns_empty() {
        let store = Store::new().unwrap();
        let proj = EvidenceProjection {
            tc: "https://test/tc".to_string(),
            outcome: StepOutcome::Pass,
            from_step: "https://test/step".to_string(),
        };
        let result = find_stale_defects_for_vgr(&store, "https://test/graph", &[&proj]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
