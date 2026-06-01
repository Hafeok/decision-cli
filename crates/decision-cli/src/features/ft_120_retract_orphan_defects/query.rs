//! SPARQL queries to find candidate orphaned defects.

use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

/// One orphan candidate: open feedback whose source VG no longer covers
/// its source TC.
#[derive(Debug, Clone)]
pub struct OrphanDefect {
    /// The feedback IRI to retract.
    pub feedback_iri: NamedNode,
    /// The TC the feedback was about.
    pub tc_iri: String,
    /// The VG that no longer covers the TC.
    pub vg_iri: String,
    /// The session that produced the original defect (the retraction
    /// authority cites this so the audit trail traces back).
    pub session_iri: String,
}

/// Find every orphan defect across the whole store.
///
/// A defect is orphaned when its source VG has no step that provides
/// evidence for the defect's source TC.
pub fn find_orphan_defects_all(store: &Store) -> Result<Vec<OrphanDefect>, String> {
    let q = r#"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
SELECT DISTINCT ?feedback ?tc ?vg ?session WHERE {
  GRAPH ?g1 {
    ?feedback a dec:Feedback ;
              dec:sourceArtifact ?tc ;
              dec:sourceSession ?session ;
              dec:lifecycleState ?state .
  }
  GRAPH ?g2 {
    ?vgr a dec:VerificationGraphResult ;
         dec:resultOf ?vg ;
         prov:wasGeneratedBy ?session .
  }
  FILTER (?state IN ("produced", "routed"))
  FILTER NOT EXISTS {
    GRAPH ?g3 {
      ?vg dec:steps/rdf:rest*/rdf:first ?step .
      ?step dec:providesEvidenceFor ?tc .
    }
  }
}"#;

    collect_orphans(store, q)
}

/// Find orphan defects scoped to a single VG.
pub fn find_orphan_defects_for_graph(
    store: &Store,
    vg_iri: &str,
) -> Result<Vec<OrphanDefect>, String> {
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
SELECT DISTINCT ?feedback ?tc ?vg ?session WHERE {{
  BIND (<{vg_iri}> AS ?vg)
  GRAPH ?g1 {{
    ?feedback a dec:Feedback ;
              dec:sourceArtifact ?tc ;
              dec:sourceSession ?session ;
              dec:lifecycleState ?state .
  }}
  GRAPH ?g2 {{
    ?vgr a dec:VerificationGraphResult ;
         dec:resultOf ?vg ;
         prov:wasGeneratedBy ?session .
  }}
  FILTER (?state IN ("produced", "routed"))
  FILTER NOT EXISTS {{
    GRAPH ?g3 {{
      ?vg dec:steps/rdf:rest*/rdf:first ?step .
      ?step dec:providesEvidenceFor ?tc .
    }}
  }}
}}"#
    );

    collect_orphans(store, &q)
}

/// Find orphan defects scoped to all VGs that verify (directly or via TC)
/// the given feature.
pub fn find_orphan_defects_for_feature(
    store: &Store,
    feature_iri: &str,
) -> Result<Vec<OrphanDefect>, String> {
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
SELECT DISTINCT ?feedback ?tc ?vg ?session WHERE {{
  GRAPH ?gf {{
    ?vg a dec:VerificationGraph .
    {{ ?vg dec:verifies <{feature_iri}> }}
    UNION
    {{
      <{feature_iri}> dec:tests ?tc_link .
      ?vg dec:verifies ?tc_link .
    }}
  }}
  GRAPH ?g1 {{
    ?feedback a dec:Feedback ;
              dec:sourceArtifact ?tc ;
              dec:sourceSession ?session ;
              dec:lifecycleState ?state .
  }}
  GRAPH ?g2 {{
    ?vgr a dec:VerificationGraphResult ;
         dec:resultOf ?vg ;
         prov:wasGeneratedBy ?session .
  }}
  FILTER (?state IN ("produced", "routed"))
  FILTER NOT EXISTS {{
    GRAPH ?g3 {{
      ?vg dec:steps/rdf:rest*/rdf:first ?step .
      ?step dec:providesEvidenceFor ?tc .
    }}
  }}
}}"#
    );

    collect_orphans(store, &q)
}

fn collect_orphans(store: &Store, q: &str) -> Result<Vec<OrphanDefect>, String> {
    let mut out = Vec::new();
    let results = store.query(q).map_err(|e| format!("SPARQL error: {e}"))?;

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|e| format!("solution error: {e}"))?;

            let Some(oxigraph::model::Term::NamedNode(fb)) = solution.get("feedback") else {
                continue;
            };
            let Some(oxigraph::model::Term::NamedNode(tc)) = solution.get("tc") else {
                continue;
            };
            let Some(oxigraph::model::Term::NamedNode(vg)) = solution.get("vg") else {
                continue;
            };
            let Some(oxigraph::model::Term::NamedNode(session)) = solution.get("session") else {
                continue;
            };

            out.push(OrphanDefect {
                feedback_iri: fb.clone(),
                tc_iri: tc.as_str().to_string(),
                vg_iri: vg.as_str().to_string(),
                session_iri: session.as_str().to_string(),
            });
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_returns_empty_for_all_scopes() {
        let store = Store::new().unwrap();
        assert!(find_orphan_defects_all(&store).unwrap().is_empty());
        assert!(find_orphan_defects_for_graph(&store, "https://test/vg")
            .unwrap()
            .is_empty());
        assert!(find_orphan_defects_for_feature(&store, "https://test/ft")
            .unwrap()
            .is_empty());
    }
}
