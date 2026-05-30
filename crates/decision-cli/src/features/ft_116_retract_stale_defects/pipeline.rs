//! Orchestrates the find→filter→transition flow for stale defect retraction.

use std::path::Path;
use std::sync::Arc;

use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::ontology::verification_result::{EvidenceProjection, VerificationGraphResult};
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::StreamWriter;

use super::query::find_stale_defects_for_vgr;
use super::transition::close_feedback;

/// Run the auto-close pass for a newly-written VGR.
///
/// This is called inside the same StreamWriter transaction that commits
/// the VGR, so the lifecycle transitions land atomically with the VGR.
///
/// Returns the count of defects closed.
pub fn retract_stale_defects_in_transaction(
    store: &Store,
    writer: &StreamWriter,
    vgr: &VerificationGraphResult,
) -> Result<usize, String> {
    // Extract passing TCs from the VGR
    let passing_tcs: Vec<&EvidenceProjection> = vgr
        .evidence_for
        .iter()
        .filter(|p| p.outcome.as_str() == "pass")
        .collect();

    if passing_tcs.is_empty() {
        // No passing TCs, nothing to retract
        return Ok(0);
    }

    // Find stale defects
    let stale_defects = find_stale_defects_for_vgr(store, &vgr.result_of, &passing_tcs)?;

    if stale_defects.is_empty() {
        return Ok(0);
    }

    // Close each stale defect
    let mut closed_count = 0;
    for defect in &stale_defects {
        // close_feedback silently skips terminal states, making this idempotent
        close_feedback(store, writer, &defect.feedback_iri, &vgr.id)
            .map_err(|e| format!("closing {}: {e}", defect.feedback_iri))?;
        closed_count += 1;
    }

    Ok(closed_count)
}

/// Standalone function for the diagnostic CLI: run the auto-close pass
/// against an existing VGR outside of a transaction.
///
/// This loads the store, opens a writer, finds the VGR, runs the
/// auto-close logic, and persists.
pub fn retract_stale_defects(
    workdir: &Path,
    graph_id: &str,
    dry_run: bool,
) -> Result<RetractResult, String> {
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path)
        .map_err(|e| format!("loading store: {e:#}"))?;
    let store = Arc::new(store);

    // Find the latest VGR for this graph
    let graph_iri = format!("https://decision-cli.dev/ns/verify/graph/{graph_id}");
    let vgr = find_latest_approved_vgr(&store, &graph_iri)?;

    if dry_run {
        // Just report what would be closed
        let passing_tcs: Vec<&EvidenceProjection> = vgr
            .evidence_for
            .iter()
            .filter(|p| p.outcome.as_str() == "pass")
            .collect();
        let stale_defects = find_stale_defects_for_vgr(&store, &graph_iri, &passing_tcs)?;
        return Ok(RetractResult {
            vgr_id: extract_vgr_id(&vgr.id),
            closed_count: stale_defects.len(),
            defect_iris: stale_defects.iter().map(|d| d.feedback_iri.to_string()).collect(),
        });
    }

    // Open writer and run the auto-close
    let scope = crate::core::scope::ActiveScope::load(workdir)
        .map_err(|e| format!("loading scope: {e}"))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| format!("invalid stream IRI: {e}"))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| format!("opening writer: {e:#}"))?;

    let passing_tcs: Vec<&EvidenceProjection> = vgr
        .evidence_for
        .iter()
        .filter(|p| p.outcome.as_str() == "pass")
        .collect();
    let stale_defects = find_stale_defects_for_vgr(&store, &graph_iri, &passing_tcs)?;
    let defect_iris: Vec<String> = stale_defects.iter().map(|d| d.feedback_iri.to_string()).collect();

    let mut closed_count = 0;
    for defect in &stale_defects {
        close_feedback(&store, &writer, &defect.feedback_iri, &vgr.id)
            .map_err(|e| format!("closing {}: {e}", defect.feedback_iri))?;
        closed_count += 1;
    }

    // Persist the store
    persist_store(&store, &dump_path)
        .map_err(|e| format!("persisting store: {e:#}"))?;

    Ok(RetractResult {
        vgr_id: extract_vgr_id(&vgr.id),
        closed_count,
        defect_iris,
    })
}

/// Result of a retraction pass (for CLI reporting).
#[derive(Debug)]
pub struct RetractResult {
    /// The VGR id that was processed.
    pub vgr_id: String,
    /// Number of defects closed.
    pub closed_count: usize,
    /// IRIs of closed defects (for dry-run display).
    pub defect_iris: Vec<String>,
}

fn extract_vgr_id(iri: &str) -> String {
    iri.rsplit('/').next().unwrap_or(iri).to_string()
}

fn find_latest_approved_vgr(
    store: &Store,
    graph_iri: &str,
) -> Result<VerificationGraphResult, String> {
    use oxigraph::sparql::QueryResults;
    use crate::core::ontology::verification_result::StepOutcome;

    // Find the most recent approved VGR for this graph
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX dcterms: <http://purl.org/dc/terms/>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?vgr ?env ?verdict ?started ?ended ?rationale ?gen ?attr ?created WHERE {{
  GRAPH ?g {{
    ?vgr a dec:VerificationGraphResult ;
         dec:resultOf <{graph_iri}> ;
         dec:verdict "approved" ;
         dec:ranInEnvironment ?env ;
         dec:verdict ?verdict ;
         dec:startedAt ?started ;
         dec:endedAt ?ended ;
         dec:rationale ?rationale ;
         prov:wasGeneratedBy ?gen ;
         prov:wasAttributedTo ?attr ;
         dcterms:created ?created .
  }}
}}
ORDER BY DESC(?created)
LIMIT 1"#
    );

    let results = store.query(&q).map_err(|e| format!("query error: {e}"))?;

    if let QueryResults::Solutions(mut solutions) = results {
        if let Some(Ok(solution)) = solutions.next() {
            let vgr_iri = solution.get("vgr")
                .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n) } else { None })
                .ok_or_else(|| "missing vgr")?;
            let env = solution.get("env")
                .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n.as_str().to_string()) } else { None })
                .ok_or_else(|| "missing env")?;
            let verdict_str = solution.get("verdict")
                .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value().to_string()) } else { None })
                .ok_or_else(|| "missing verdict")?;
            let started = solution.get("started")
                .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value().to_string()) } else { None })
                .ok_or_else(|| "missing started")?;
            let ended = solution.get("ended")
                .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value().to_string()) } else { None })
                .ok_or_else(|| "missing ended")?;
            let rationale = solution.get("rationale")
                .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value().to_string()) } else { None })
                .ok_or_else(|| "missing rationale")?;
            let gen = solution.get("gen")
                .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n.as_str().to_string()) } else { None })
                .ok_or_else(|| "missing gen")?;
            let attr = solution.get("attr")
                .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n.as_str().to_string()) } else { None })
                .ok_or_else(|| "missing attr")?;
            let created = solution.get("created")
                .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value().to_string()) } else { None })
                .ok_or_else(|| "missing created")?;

            // Query for evidence projections
            let evidence_q = format!(
                r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?tc ?outcome ?fromStep WHERE {{
  GRAPH ?g {{
    <{vgr_iri}> dec:evidenceFor ?ev .
    ?ev dec:tc ?tc ;
        dec:outcome ?outcome ;
        dec:fromStep ?fromStep .
  }}
}}"#,
                vgr_iri = vgr_iri.as_str()
            );

            let evidence_results = store.query(&evidence_q).map_err(|e| format!("evidence query error: {e}"))?;
            let mut evidence_for = Vec::new();

            if let QueryResults::Solutions(ev_solutions) = evidence_results {
                for ev_sol in ev_solutions {
                    let ev_sol = ev_sol.map_err(|e| format!("evidence solution error: {e}"))?;
                    let tc = ev_sol.get("tc")
                        .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n.as_str().to_string()) } else { None })
                        .ok_or_else(|| "missing tc")?;
                    let outcome_str = ev_sol.get("outcome")
                        .and_then(|t| if let oxigraph::model::Term::Literal(l) = t { Some(l.value()) } else { None })
                        .ok_or_else(|| "missing outcome")?;
                    let from_step = ev_sol.get("fromStep")
                        .and_then(|t| if let oxigraph::model::Term::NamedNode(n) = t { Some(n.as_str().to_string()) } else { None })
                        .ok_or_else(|| "missing fromStep")?;

                    let outcome = match outcome_str {
                        "pass" => StepOutcome::Pass,
                        "fail" => StepOutcome::Fail,
                        _ => StepOutcome::Unrunnable,
                    };

                    evidence_for.push(EvidenceProjection {
                        tc,
                        outcome,
                        from_step,
                    });
                }
            }

            return Ok(VerificationGraphResult {
                id: vgr_iri.as_str().to_string(),
                result_of: graph_iri.to_string(),
                ran_in_environment: env,
                verdict: crate::core::ontology::verdict::Verdict::parse(&verdict_str)
                    .ok_or_else(|| format!("unknown verdict: {verdict_str}"))?,
                started_at: started,
                ended_at: ended.clone(),
                step_traces: Vec::new(), // Not needed for auto-close
                evidence_for,
                rationale,
                was_generated_by: gen,
                was_attributed_to: attr,
                created_at: created,
            });
        }
    }

    Err(format!("no approved VGR found for {graph_iri}"))
}
