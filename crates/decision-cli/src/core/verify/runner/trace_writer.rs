//! Phase-5 persistence — build `VerificationGraphResult` + commit
//! through `StreamWriter` (FT-098 §Behaviour).

use std::path::Path;
use std::sync::Arc;

use crate::core::ontology::verdict::Verdict;
use crate::core::ontology::verification_bench::VerificationBench;
use crate::core::ontology::verification_graph::VerificationGraph;
use crate::core::ontology::verification_result::{
    to_canonical_turtle, EvidenceProjection, StepOutcome, VerificationGraphResult,
    VerificationStepTrace,
};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::{verify_result_graph, IRI_DEC_VERIFY_RESULT_PREFIX};
use crate::core::StreamWriter;
use anyhow::Context as AnyhowContext;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use super::kinds::StepRunTrace;
use super::request::RunnerError;

/// Build the result artifact and persist it through `StreamWriter`.
/// Returns the minted IRI.
pub(crate) fn write_result(
    workdir: &Path,
    graph: &VerificationGraph,
    env: &VerificationBench,
    traces: &[StepRunTrace],
    verdict: Verdict,
    rationale: String,
    run_activity: &NamedNode,
    runner_agent: &NamedNode,
    started_at: String,
    ended_at: String,
) -> Result<(NamedNode, VerificationGraphResult), RunnerError> {
    let id = mint_result_id(workdir);
    let result_iri = format!("{IRI_DEC_VERIFY_RESULT_PREFIX}{id}");
    let step_traces: Vec<VerificationStepTrace> = traces
        .iter()
        .enumerate()
        .map(|(i, t)| VerificationStepTrace {
            id: format!("{result_iri}/step/{i}"),
            traces_step: graph.steps[i].id.as_str().to_string(),
            outcome: t.outcome,
            started_at: t.started_at.clone(),
            ended_at: t.ended_at.clone(),
            exit_code: t.exit_code,
            stdout_excerpt: t.stdout_excerpt.clone(),
            stderr_excerpt: t.stderr_excerpt.clone(),
            error_message: t.error_message.clone(),
            was_generated_by: run_activity.as_str().to_string(),
        })
        .collect();
    let evidence_for: Vec<EvidenceProjection> =
        build_evidence_projections(graph, traces, &result_iri);

    let result = VerificationGraphResult {
        id: result_iri.clone(),
        result_of: graph.id.as_str().to_string(),
        ran_in_environment: env.iri().as_str().to_string(),
        verdict,
        started_at,
        ended_at: ended_at.clone(),
        step_traces,
        evidence_for,
        rationale,
        was_generated_by: run_activity.as_str().to_string(),
        was_attributed_to: runner_agent.as_str().to_string(),
        created_at: ended_at,
    };

    let store = load_store_from_dump(&orchestration_dump_path(workdir)).map_err(|e| {
        RunnerError::Internal {
            detail: format!("loading orchestration store: {e:#}"),
        }
    })?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).map_err(|e| RunnerError::Internal {
        detail: format!("loading active scope: {e}"),
    })?;
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| RunnerError::Internal {
        detail: format!("active stream iri: {e}"),
    })?;
    let writer =
        StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| RunnerError::Internal {
            detail: format!("opening stream writer: {e:#}"),
        })?;
    let quads = result.to_quads(verify_result_graph());
    let mutation = Mutation::insert(quads);
    writer
        .commit(mutation)
        .map_err(|source| RunnerError::ResultWriteFailed { source })?;

    // FT-116: Auto-close stale defects after VGR commit (within same transaction)
    use crate::features::ft_116_retract_stale_defects::retract_stale_defects_in_transaction;
    let _closed_count =
        retract_stale_defects_in_transaction(&store, &writer, &result).map_err(|e| {
            RunnerError::Internal {
                detail: format!("auto-closing stale defects: {e}"),
            }
        })?;

    persist_store(&store, &orchestration_dump_path(workdir)).map_err(|e| {
        RunnerError::Internal {
            detail: format!("persisting orchestration store: {e:#}"),
        }
    })?;

    // Write the canonical Turtle file too, mirroring the env/graph
    // on-disk convention.
    write_canonical_turtle(workdir, &id, &result).map_err(|e| RunnerError::Internal {
        detail: format!("persisting result turtle: {e:#}"),
    })?;

    let iri = NamedNode::new_unchecked(result_iri);
    Ok((iri, result))
}

fn build_evidence_projections(
    graph: &VerificationGraph,
    traces: &[StepRunTrace],
    result_iri: &str,
) -> Vec<EvidenceProjection> {
    let mut out: Vec<EvidenceProjection> = Vec::new();
    for (i, step) in graph.steps.iter().enumerate() {
        if step.provides_evidence_for.is_empty() {
            continue;
        }
        let Some(trace) = traces.get(i) else {
            continue;
        };
        let from_step = format!("{result_iri}/step/{i}");
        for tc in &step.provides_evidence_for {
            out.push(EvidenceProjection {
                tc: tc.as_str().to_string(),
                outcome: trace.outcome,
                from_step: from_step.clone(),
            });
        }
    }
    out
}

fn mint_result_id(workdir: &Path) -> String {
    let dir = workdir.join(".dec").join("verify").join("result");
    let _ = std::fs::create_dir_all(&dir);
    let mut max: usize = 0;
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(stem) = name.strip_suffix(".ttl") else {
                continue;
            };
            let Some(num) = stem.strip_prefix("VGR-") else {
                continue;
            };
            if let Ok(n) = num.parse::<usize>() {
                if n > max {
                    max = n;
                }
            }
        }
    }
    format!("VGR-{:03}", max + 1)
}

fn write_canonical_turtle(
    workdir: &Path,
    id: &str,
    result: &VerificationGraphResult,
) -> anyhow::Result<()> {
    let dir = workdir.join(".dec").join("verify").join("result");
    std::fs::create_dir_all(&dir).context("create result dir")?;
    let path = dir.join(format!("{id}.ttl"));
    let body = to_canonical_turtle(result);
    std::fs::write(&path, body).context("write result ttl")?;
    Ok(())
}

/// Determine whether `traces` indicate at least one fail.
#[allow(dead_code)]
pub(crate) fn any_fail(traces: &[StepRunTrace]) -> bool {
    traces
        .iter()
        .any(|t| matches!(t.outcome, StepOutcome::Fail))
}
