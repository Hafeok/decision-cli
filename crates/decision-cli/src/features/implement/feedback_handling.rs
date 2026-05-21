//! Worker-feedback ingest path for the implementer dispatch (FT-031 / FT-032).
//!
//! Split out of `mod.rs` so the run pipeline stays under ADR-013 Rule 1's
//! 400-line hard cap. This file owns three responsibilities:
//!
//!   1. Scanning the worker's raw stdout for emit_feedback records
//!      ([`apply_worker_feedback`]).
//!   2. Deciding whether an emission is blocking per ADR-023 / ADR-025
//!      ([`is_blocking_emission`]).
//!   3. Closing out a paused run with a `paused-by-feedback`
//!      [`ImplementOutcome`] when at least one blocking emission landed
//!      ([`finalize_blocked_run`]).

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use oxigraph::model::NamedNode;

use super::bundle::persist_store;
use super::worker::WorkerResponseJson;
use super::{DispatchContext, ImplementOutcome};
use crate::core::dispatch::{pause_on_feedback, DispatchStatus};
use crate::core::feedback::class::{Disposition, FeedbackClass};
use crate::core::worker::{apply_feedback_emission, parse_feedback_records, FeedbackEmission};

/// FT-031 + FT-032: ingest every `__DEC_FEEDBACK__` record on the
/// worker's stdout. Returns the IRIs of any blocking emissions; the
/// caller uses them to pause the dispatch.
///
/// Non-blocking emissions are committed but do not gate the dispatch.
/// Parse / commit errors are logged via `tracing` and otherwise
/// non-fatal: a malformed record cannot be allowed to silently swallow
/// the worker's output.
pub(super) fn apply_worker_feedback(
    ctx: &mut DispatchContext,
    raw_stdout: &str,
) -> Result<Vec<NamedNode>> {
    let (records, parse_errs) = parse_feedback_records(raw_stdout);
    for err in &parse_errs {
        tracing::warn!(
            target: "dec::implement::feedback",
            error = %err,
            "skipped malformed feedback record",
        );
    }
    let mut blocking: Vec<NamedNode> = Vec::new();
    for record in &records {
        let iri = match apply_feedback_emission(&ctx.writer, record, Some(&ctx.session_iri)) {
            Ok(iri) => iri,
            Err(err) => {
                tracing::warn!(
                    target: "dec::implement::feedback",
                    error = %err,
                    "failed to commit feedback emission; skipping",
                );
                continue;
            }
        };
        if is_blocking_emission(record) {
            blocking.push(iri);
        }
    }
    if !records.is_empty() {
        let _ = persist_store(&ctx.store, &ctx.dump_path);
    }
    Ok(blocking)
}

/// Decide whether `emission` should pause the dispatch. Honours an
/// explicit `blocking` override, otherwise falls back to the class
/// default (ADR-023 / ADR-025).
fn is_blocking_emission(emission: &FeedbackEmission) -> bool {
    if let Some(b) = emission.blocking {
        return b;
    }
    FeedbackClass::from_iri_value(&emission.feedback_class)
        .map(|c| matches!(c.default_disposition(), Disposition::Blocking))
        .unwrap_or(false)
}

/// FT-032 §Behaviour — close out a run whose worker emitted blocking
/// feedback. Pauses the DispatchGroup, persists the store snapshot, and
/// returns an [`ImplementOutcome`] whose `worker_status` carries the
/// `paused-by-feedback` terminal status from ADR-025.
pub(super) fn finalize_blocked_run(
    mut ctx: DispatchContext,
    response: WorkerResponseJson,
    blocking: Vec<NamedNode>,
) -> Result<ImplementOutcome> {
    pause_on_feedback(&ctx.writer, &ctx.store, &ctx.group.iri, &blocking)
        .map_err(|e| anyhow!("pausing DispatchGroup on blocking feedback: {e}"))?;
    // Refresh the in-memory status so downstream telemetry matches the
    // persisted literal.
    ctx.group.status = DispatchStatus::PausedForFeedback;
    persist_store(&ctx.store, &ctx.dump_path)?;
    let workspace_dir = ctx.workspace_dir.clone();
    let waiver_iri = ctx.waiver_iri.as_ref().map(|n| n.as_str().to_string());
    Ok(ImplementOutcome {
        session_iri: ctx.session_iri.as_str().to_string(),
        dispatch_iri: ctx.dispatch_iri.as_str().to_string(),
        // No CodeChange artifact was persisted; surface an explicit
        // empty IRI so callers do not mistake the paused run for an
        // approved one.
        code_change_iri: String::new(),
        bundle_hash: ctx.bundle_hash,
        workspace_dir,
        product_codechange_path: PathBuf::new(),
        files_written: Vec::new(),
        worker_status: "paused-by-feedback".to_string(),
        turn_count: response.telemetry.turn_count,
        latency_seconds: response.telemetry.latency_seconds,
        finalize: None,
        waiver_iri,
    })
}
