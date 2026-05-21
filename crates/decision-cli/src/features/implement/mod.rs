//! `dec implement <FT-XXX>` — implementer-role end-to-end (FT-011 + FT-013).
//!
//! This module is the harness side of the slice-1 implementer loop. It:
//!
//! 1. Validates that the active stream authorises the `ship` goal
//!    (ADR-005 / [`crate::scope::ActiveScope`]).
//! 2. Assembles a context bundle for the target feature by invoking
//!    `product context FT-XXX --depth 1` as a subprocess (ADR-009),
//!    falling back to a minimal synthetic bundle when product-cli is
//!    not on `$PATH`.
//! 3. Computes a SHA-256 over the bundle bytes.
//! 4. Mints `Session` + `Dispatch` artifacts in the orchestration store
//!    with full PROV-O lineage (ADR-004) and the `dec:inStream` tag
//!    (ADR-005) — both via [`crate::StreamWriter`] so the tagging is
//!    structural rather than incidental.
//! 5. Spawns the code-writer worker as a subprocess in one-shot mode
//!    (`code-writer run-once`), feeding it a `DispatchPayload` on stdin
//!    and parsing the `WorkerResponse` from stdout (ADR-008).
//! 6. Persists the resulting `CodeChange` into the product-cli graph
//!    slice at `<product-root>/.product/graph/code-changes.nq` so the
//!    cross-store PROV-O invariant (TC-013) is machine-verifiable.
//! 7. Reports the outcome on stdout.

mod bundle;
mod codechange;
mod feedback_handling;
mod lifecycle;
mod prepare;
mod quads;
mod session_show;
mod vocab;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::dispatch::{DispatchEvent, DispatchGroup, DispatchStatus};
use crate::core::subscriptions::verifier_dispatch::{
    emit_verifier_dispatch_event, VerifierDispatchSeed,
};
use crate::core::StreamWriter;

pub use bundle::resolve_product_root;
pub use session_show::session_show;

use bundle::persist_store;
use lifecycle::{
    assemble_implement_outcome, build_dispatch_payload, commit_session_completion,
    extract_code_change, finalize_implement_run, persist_code_change,
};
use prepare::prepare_dispatch;
use quads::build_failure_quad;
use worker::{format_worker_failure, run_worker};

/// Goal verb the implementer role always pursues (ADR-005 / §3.4).
pub const IMPLEMENT_GOAL: &str = "ship";

/// Hard-coded slice-1 model id (model catalog deferred per §6.2).
pub const SLICE1_MODEL_ID: &str = "claude-sonnet-4-5";

/// Hard-coded role id (single role in slice 1).
pub const IMPLEMENTER_ROLE: &str = "code-writer";

/// CLI arguments accepted by [`run`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct ImplementArgs {
    pub feature_id: String,
    pub workspace: Option<PathBuf>,
    pub worker_command: Option<String>,
    pub product_root: Option<PathBuf>,
    pub bundle_depth: usize,
    /// FT-047 / ADR-031 — optional waiver supplied via `--waive-coverage`
    /// (CLI) or `accept_waiver: { reason }` (MCP). When present, the
    /// chain-integrity gate mints a `dec:CoverageWaiver` rather than
    /// refusing the dispatch.
    pub waiver: Option<crate::core::verify::WaiverIntent>,
}

impl ImplementArgs {
    /// Build with sensible defaults.
    #[must_use]
    pub fn new(feature_id: impl Into<String>) -> Self {
        Self {
            feature_id: feature_id.into(),
            workspace: None,
            worker_command: None,
            product_root: None,
            bundle_depth: 1,
            waiver: None,
        }
    }
}

/// Final result of a successful [`run`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct ImplementOutcome {
    pub session_iri: String,
    pub dispatch_iri: String,
    pub code_change_iri: String,
    pub bundle_hash: String,
    pub workspace_dir: PathBuf,
    pub product_codechange_path: PathBuf,
    pub files_written: Vec<PathBuf>,
    pub worker_status: String,
    pub turn_count: u64,
    pub latency_seconds: f64,
    /// FT-017 finalisation: commit + status transition. `None` when the
    /// run errored before finalisation could run.
    pub finalize: Option<crate::finalize::FinalizeOutcome>,
    /// FT-047 / ADR-031: IRI of the persisted `dec:CoverageWaiver`, when
    /// the chain-integrity gate accepted a `--waive-coverage` reason.
    /// `None` when the gate passed cleanly.
    pub waiver_iri: Option<String>,
}

struct DispatchContext {
    session_iri: NamedNode,
    dispatch_iri: NamedNode,
    bundle_hash: String,
    bundle_markdown: String,
    workspace_dir: PathBuf,
    product_root: PathBuf,
    dump_path: PathBuf,
    store: Arc<Store>,
    writer: StreamWriter,
    worker_argv: Vec<String>,
    /// FT-021 / ADR-017: the paired DispatchGroup minted at command entry.
    group: DispatchGroup,
    /// FT-047 / ADR-031: persisted waiver IRI when the chain-integrity
    /// gate accepted a `--waive-coverage` reason. `None` when coverage
    /// was already complete.
    waiver_iri: Option<NamedNode>,
}

fn record_worker_failure(ctx: &mut DispatchContext, detail: &str) {
    let mut fail = Mutation::default();
    fail.inserts
        .push(build_failure_quad(&ctx.session_iri, detail));
    ctx.writer
        .commit(fail.with_cause("dec implement: worker failure"))
        .ok();
    // FT-021 / ADR-017: action failure transitions the DispatchGroup
    // into `action-failed`. The verifier is NOT dispatched.
    let _ = ctx
        .group
        .transition(&ctx.writer, DispatchEvent::ActionFailed);
    let _ = persist_store(&ctx.store, &ctx.dump_path);
}

/// Run the implementer dispatch end-to-end. See module docs.
pub fn run(workdir: &Path, args: &ImplementArgs) -> Result<ImplementOutcome> {
    let mut ctx = prepare_dispatch(workdir, args)?;
    let dispatch_payload = build_dispatch_payload(&ctx, args);
    let worker_run =
        run_worker(&ctx.worker_argv, &dispatch_payload).context("running code-writer worker")?;
    let response = worker_run.response;

    // FT-031 + FT-032: scan the worker's stdout for emit_feedback
    // records BEFORE we look at the action artifact. Blocking
    // emissions short-circuit the action path: the orchestrator drops
    // the produced artifact (if any), commits the feedback, and pauses
    // the DispatchGroup. Non-blocking emissions are committed in
    // parallel; the dispatch proceeds normally.
    let blocking = feedback_handling::apply_worker_feedback(&mut ctx, &worker_run.raw_stdout)?;
    if !blocking.is_empty() {
        // FT-032 §Behaviour step 3 — drop the action artifact and park
        // the group. The worker's WorkerResponse is preserved in the
        // returned outcome for telemetry, but the action's CodeChange
        // is intentionally NOT persisted: the addressing artifact
        // becomes the system's new authoritative input.
        return feedback_handling::finalize_blocked_run(ctx, response, blocking);
    }

    if response.status != "ok" {
        let detail = format_worker_failure(response.error.as_ref());
        record_worker_failure(&mut ctx, &detail);
        return Err(anyhow!("code-writer worker reported failure: {detail}"));
    }
    let code_change = extract_code_change(&response)?;
    let codechange_path = persist_code_change(&ctx, args, code_change)?;
    commit_session_completion(&ctx, code_change)?;
    // FT-021 / ADR-017: action terminated with a produced artifact.
    // Transition the DispatchGroup to `awaiting-interpretation`. The
    // verifier dispatch itself is FT-022 / FT-023; from FT-021's point
    // of view the group is now ready for that subscription to fire.
    let _final_status = transition_after_action(&mut ctx)?;
    // FT-022 / ADR-017: now that the group is `awaiting-interpretation`,
    // run the verifier-dispatch subscription's delivery handler so the
    // outbox carries the `dec:VerifierDispatchEvent` for FT-023 to pick
    // up. The handler is idempotent — a stale event from a previous run
    // is not duplicated.
    emit_verifier_dispatch_for_group(&mut ctx)?;
    let finalize_outcome = finalize_implement_run(workdir, &ctx, args, code_change)?;
    Ok(assemble_implement_outcome(
        ctx,
        &response,
        code_change,
        codechange_path,
        finalize_outcome,
    ))
}

/// Advance the DispatchGroup after the action session has terminated
/// with a produced artifact. Returns the resulting status (typically
/// [`DispatchStatus::AwaitingInterpretation`]) so callers can decide
/// whether to block on the verifier.
fn transition_after_action(ctx: &mut DispatchContext) -> Result<DispatchStatus> {
    let status = ctx
        .group
        .transition(&ctx.writer, DispatchEvent::ActionCompleted)
        .with_context(|| {
            format!(
                "transitioning DispatchGroup {} to awaiting-interpretation",
                ctx.group.iri_str()
            )
        })?;
    persist_store(&ctx.store, &ctx.dump_path)?;
    Ok(status)
}

/// FT-022 / ADR-017 — fire the verifier-dispatch handler for the
/// just-transitioned `DispatchGroup`. Idempotent: a previous emission
/// for the same group suppresses the second one. Persists the store
/// snapshot so a fresh `dec` process picks the event up after a crash.
fn emit_verifier_dispatch_for_group(ctx: &mut DispatchContext) -> Result<()> {
    if ctx.group.status != DispatchStatus::AwaitingInterpretation {
        return Ok(());
    }
    let seed = VerifierDispatchSeed {
        group: ctx.group.iri.clone(),
        action_session: ctx.group.action_session.clone(),
    };
    let emitted_at = Utc::now().to_rfc3339();
    let emitted = emit_verifier_dispatch_event(&ctx.writer, &ctx.store, &seed, &emitted_at)
        .map_err(|e| anyhow!("verifier dispatch handler: {e}"))?;
    if emitted.is_some() {
        persist_store(&ctx.store, &ctx.dump_path)?;
    }
    Ok(())
}
