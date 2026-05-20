//! Post-dispatch lifecycle helpers extracted from [`super::run`].
//!
//! Splits the implementer-run pipeline into discrete steps:
//! payload assembly, code-change persistence, completion commit,
//! finalisation, outcome assembly. Each helper is small enough to read
//! at a glance per ADR-013 Rule 2.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use super::bundle::{persist_store, product_codechange_path};
use super::codechange::write_codechange_to_product_graph;
use super::quads::build_completion_quads;
use super::worker::{CodeChangeJson, DispatchPayloadJson, WorkerResponseJson};
use super::{DispatchContext, ImplementArgs, ImplementOutcome, SLICE1_MODEL_ID};

/// Build the JSON payload streamed to the code-writer worker's stdin.
pub(super) fn build_dispatch_payload(
    ctx: &DispatchContext,
    args: &ImplementArgs,
) -> DispatchPayloadJson {
    let workspace_path = ctx
        .workspace_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.workspace_dir.clone())
        .to_string_lossy()
        .into_owned();
    DispatchPayloadJson {
        dispatch_id: ctx.dispatch_iri.as_str().to_string(),
        session_id: ctx.session_iri.as_str().to_string(),
        feature_id: args.feature_id.clone(),
        bundle_markdown: ctx.bundle_markdown.clone(),
        bundle_hash: ctx.bundle_hash.clone(),
        workspace_path,
        model_id: SLICE1_MODEL_ID.to_string(),
        timeout_seconds: 1800,
    }
}

/// Persist the worker's CodeChange into the product-cli graph slice.
pub(super) fn persist_code_change(
    ctx: &DispatchContext,
    args: &ImplementArgs,
    code_change: &CodeChangeJson,
) -> Result<PathBuf> {
    let codechange_path = product_codechange_path(&ctx.product_root);
    write_codechange_to_product_graph(
        &codechange_path,
        code_change,
        &ctx.session_iri,
        &ctx.dispatch_iri,
        &args.feature_id,
    )
    .with_context(|| {
        format!(
            "writing CodeChange artifact to product graph at {}",
            codechange_path.display()
        )
    })?;
    Ok(codechange_path)
}

/// Commit the session-completion quads (PROV-O `endedAtTime` + generated).
pub(super) fn commit_session_completion(
    ctx: &DispatchContext,
    code_change: &CodeChangeJson,
) -> Result<()> {
    let completed_at = Utc::now().to_rfc3339();
    let code_change_iri = NamedNode::new(&code_change.iri)
        .with_context(|| format!("code change IRI {}", code_change.iri))?;
    let complete_quads = build_completion_quads(&ctx.session_iri, &code_change_iri, &completed_at);
    let mut complete = Mutation::default();
    for q in complete_quads {
        complete.inserts.push(q);
    }
    ctx.writer
        .commit(complete.with_cause("dec implement: worker complete"))
        .context("committing session completion")?;
    persist_store(&ctx.store, &ctx.dump_path)?;
    Ok(())
}

/// Drive FT-017 finalisation (commit + status transition).
pub(super) fn finalize_implement_run(
    workdir: &std::path::Path,
    ctx: &DispatchContext,
    args: &ImplementArgs,
    code_change: &CodeChangeJson,
) -> Result<crate::finalize::FinalizeOutcome> {
    let finalize_input = crate::finalize::FinalizeInput {
        repo_root: workdir,
        product_root: &ctx.product_root,
        feature_id: &args.feature_id,
        session_iri: ctx.session_iri.as_str(),
        dispatch_iri: ctx.dispatch_iri.as_str(),
        code_change_iri: code_change.iri.as_str(),
        bundle_hash: &ctx.bundle_hash,
        worker_summary: &code_change.summary,
    };
    crate::finalize::finalize_run(&finalize_input).context("finalising dec implement run (FT-017)")
}

/// Assemble the final outcome returned by [`super::run`].
pub(super) fn assemble_implement_outcome(
    ctx: DispatchContext,
    response: &WorkerResponseJson,
    code_change: &CodeChangeJson,
    codechange_path: PathBuf,
    finalize_outcome: crate::finalize::FinalizeOutcome,
) -> ImplementOutcome {
    let files_written: Vec<PathBuf> = code_change
        .files
        .iter()
        .map(|f| ctx.workspace_dir.join(&f.path))
        .collect();
    ImplementOutcome {
        session_iri: ctx.session_iri.as_str().to_string(),
        dispatch_iri: ctx.dispatch_iri.as_str().to_string(),
        code_change_iri: code_change.iri.clone(),
        bundle_hash: ctx.bundle_hash,
        workspace_dir: ctx.workspace_dir,
        product_codechange_path: codechange_path,
        files_written,
        worker_status: response.status.clone(),
        turn_count: response.telemetry.turn_count,
        latency_seconds: response.telemetry.latency_seconds,
        finalize: Some(finalize_outcome),
    }
}

/// Resolve the worker response into a `&CodeChangeJson`, erroring when
/// the worker reported `status=ok` without producing a CodeChange.
pub(super) fn extract_code_change(response: &WorkerResponseJson) -> Result<&CodeChangeJson> {
    response
        .code_change
        .as_ref()
        .ok_or_else(|| anyhow!("worker reported status=ok with no code_change"))
}
