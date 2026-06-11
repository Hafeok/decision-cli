//! Sequential graph-run orchestration for `dec verify feature` (FT-099).
//!
//! Given an enumerated list of `(graph, env)` tuples and the feature's
//! TCs, invoke the FT-098 runner once per tuple, collect the persisted
//! VGRs, and compose them through FT-097's `aggregate_verdict`.
//!
//! Pure orchestration — no rendering, no surface concerns.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use oxigraph::model::NamedNode;

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_result::VerificationGraphResult;
use crate::core::verify::aggregate::{aggregate_verdict, AggregationTarget};
use crate::core::verify::coverage::feature_resolver::IRI_TC_PREFIX;
use crate::core::verify::runner::{run_graph, RunGraphRequest, RunnerError, TriggerKind};

use super::enumerate::GraphTuple;
use super::{AggregateBlock, PerGraphRow, PerTcRow};

const RUN_ACTIVITY_PREFIX: &str = "https://decision-cli.dev/ns/activity/verify-feature/";

/// Aggregated outcome of a full sequential run pass.
pub(super) struct RunOutcome {
    /// One `PerGraphRow` per tuple, in enumerated order.
    pub per_graph: Vec<PerGraphRow>,
    /// Per-TC verdict rows in feature-spec declaration order.
    pub per_tc: Vec<PerTcRow>,
    /// Aggregate verdict block per FT-097.
    pub aggregate: AggregateBlock,
    /// Short-id TCs that no contributing result covered.
    pub coverage_gaps: Vec<String>,
}

/// Execute every tuple in order; aggregate; return the structured outcome.
pub(super) fn execute_and_aggregate(
    workdir: &Path,
    feature_id: &str,
    feature_iri: &str,
    tcs: &[String],
    tuples: &[GraphTuple],
) -> Result<RunOutcome, HandlerError> {
    let mut per_graph: Vec<PerGraphRow> = Vec::with_capacity(tuples.len());
    let mut results: Vec<VerificationGraphResult> = Vec::with_capacity(tuples.len());
    for tuple in tuples {
        let row = run_single_tuple(workdir, feature_id, feature_iri, tuple, &mut results);
        per_graph.push(row);
    }
    let agg = aggregate_verdict(
        AggregationTarget::Feature {
            feature: feature_iri.to_string(),
            tests: tcs.to_vec(),
        },
        &results,
    );
    let per_tc = build_per_tc(tcs, &results);
    let aggregate_block = AggregateBlock {
        verdict: agg.verdict.as_str().to_string(),
        rationale: agg.rationale.clone(),
    };
    let coverage_gaps = canonicalize_tcs(&agg.coverage_gaps);
    Ok(RunOutcome {
        per_graph,
        per_tc,
        aggregate: aggregate_block,
        coverage_gaps,
    })
}

fn run_single_tuple(
    workdir: &Path,
    _feature_id: &str,
    feature_iri: &str,
    tuple: &GraphTuple,
    results: &mut Vec<VerificationGraphResult>,
) -> PerGraphRow {
    let graph_node = match NamedNode::new(&tuple.graph_iri) {
        Ok(n) => n,
        Err(e) => {
            return PerGraphRow {
                vg: tuple.graph_short.clone(),
                env: tuple.env_short.clone(),
                verdict: Some("rejected".to_string()),
                result_id: None,
                status: "error".to_string(),
                note: Some(format!("graph IRI: {e}")),
            };
        }
    };
    let runner_req = RunGraphRequest {
        graph: graph_node,
        triggered_by: TriggerKind::Aggregate {
            feature: NamedNode::new_unchecked(feature_iri.to_string()),
        },
        capture_bindings: std::collections::HashMap::new(),
        run_activity: mint_run_activity(_feature_id, &tuple.graph_short),
        workdir: workdir.to_path_buf(),
    };
    match run_graph(&runner_req) {
        Ok(resp) => {
            let row = PerGraphRow {
                vg: tuple.graph_short.clone(),
                env: tuple.env_short.clone(),
                verdict: Some(resp.verdict.as_str().to_string()),
                result_id: Some(resp.result.as_str().to_string()),
                status: "ran".to_string(),
                note: None,
            };
            results.push(resp.result_artifact);
            row
        }
        Err(err) => PerGraphRow {
            vg: tuple.graph_short.clone(),
            env: tuple.env_short.clone(),
            verdict: Some("rejected".to_string()),
            result_id: None,
            status: "error".to_string(),
            note: Some(format_runner_error(&err)),
        },
    }
}

fn build_per_tc(tcs: &[String], results: &[VerificationGraphResult]) -> Vec<PerTcRow> {
    tcs.iter()
        .map(|tc_iri| {
            let agg = aggregate_verdict(AggregationTarget::Tc(tc_iri.clone()), results);
            PerTcRow {
                tc: canonical_tc_short(tc_iri),
                verdict: agg.verdict.as_str().to_string(),
                rationale: agg.rationale.clone(),
                from_results: agg.contributing_results.clone(),
            }
        })
        .collect()
}

fn canonical_tc_short(iri: &str) -> String {
    iri.strip_prefix(IRI_TC_PREFIX).unwrap_or(iri).to_string()
}

fn canonicalize_tcs(iris: &[String]) -> Vec<String> {
    iris.iter().map(|i| canonical_tc_short(i)).collect()
}

fn mint_run_activity(feature_id: &str, graph_id: &str) -> NamedNode {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let iri = format!("{RUN_ACTIVITY_PREFIX}{feature_id}/{graph_id}/ts-{nanos}");
    NamedNode::new_unchecked(iri)
}

fn format_runner_error(err: &RunnerError) -> String {
    match err {
        RunnerError::ArtifactNotFound { kind, id } => format!("not found: {kind} <{id}>"),
        RunnerError::SafetyViolation { step, op } => {
            format!("safety: step <{step}> requires op <{op}>")
        }
        RunnerError::EnvSetupFailed { exit_code, .. } => {
            format!("env setup failed (exit {exit_code})")
        }
        RunnerError::ResultWriteFailed { source } => {
            format!("result persistence failed: {source:#}")
        }
        RunnerError::Internal { detail } => format!("internal: {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tc_short_strips_prefix() {
        assert_eq!(
            canonical_tc_short("https://decision-cli.dev/ns/tc/TC-001"),
            "TC-001"
        );
        assert_eq!(canonical_tc_short("TC-007"), "TC-007");
    }
}
