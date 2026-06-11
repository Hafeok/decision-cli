//! `dec verify graph generate` + `dec_verify_graph_accept` (FT-049 / ADR-030).
//!
//! Two paired handlers that together implement the verify-graph-author
//! role's CLI + MCP surface per ADR-029 §Single-handler discipline:
//!
//!   * [`run_generate`] — assembles a bundle, runs the matcher first,
//!     invokes the worker only when generation is actually needed,
//!     and (optionally, on `--accept`) persists through the slice-2.5
//!     writers ([`crate::verify_graph_new`] + [`crate::verify_step_add`]).
//!   * [`run_accept`] — second half of the MCP two-call protocol;
//!     replays a `New` proposal, re-runs the matcher, refuses stale
//!     proposals, otherwise persists.
//!
//! Level-3 autonomy per ADR-030 §Level-3 autonomy is enforced by the
//! `mode` field on the request: `Interactive` and `PrintOnly` never
//! persist; only `Accept` does.

pub mod bundle;
pub mod defect_feedback;
pub mod enrichment;
pub mod feedback;
pub mod feedback_close;
mod finalize;
mod internal;
pub mod persist;
pub mod proposal;
mod step_vocabulary;
pub mod surface;
pub mod validator;
pub mod worker;

use std::path::{Path, PathBuf};

use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

use crate::core::dispatch::capability_resolver::{
    resolve_default_capability, ResolvedCapability, ResolverError,
};
use crate::core::handler::Error as HandlerError;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::verify::matcher::{MatchKind, MatchReport};

use self::bundle::{assemble_bundle, env_iri_to_short};
use self::finalize::{build_match_response, finalize_generate, persist_if_new};
use self::internal::{coverage_preview_from_report, run_matcher};
use self::proposal::{CoverageReportSummary, GraphProposal, ProposalKind};

pub use self::surface::{
    accept_input_schema, accept_tool_descriptor, generate_input_schema, generate_tool_descriptor,
    parse_accept_request, parse_generate_request, response_for_accept, response_for_generate,
};

/// MCP tool name for the generate verb.
pub const TOOL_NAME_GENERATE: &str = "dec_verify_graph_generate";
/// MCP tool name for the accept verb (companion shim per ADR-030).
pub const TOOL_NAME_ACCEPT: &str = "dec_verify_graph_accept";

/// Mode dispatch per ADR-030 §Level-3 autonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateMode {
    /// Print the proposal and prompt `[y/N]` (CLI default — handled
    /// outside the handler since prompting is a surface concern).
    Interactive,
    /// Persist immediately (non-interactive `--accept`).
    Accept,
    /// Print only; never persist. Used by scripts.
    PrintOnly,
}

impl Default for GenerateMode {
    fn default() -> Self {
        Self::Interactive
    }
}

/// Request for the generate handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Short feature id (e.g. `FT-049`).
    pub feature_id: String,
    /// Short env id (e.g. `BNCH-001-ephemeral-cli`).
    pub environment_id: String,
    /// Persistence mode (defaults to `Interactive`).
    #[serde(default)]
    pub mode: GenerateMode,
    /// Working directory containing `.dec/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
    /// Optional product-root override (defaults to `workdir`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_root: Option<PathBuf>,
}

/// Outcome of the generate handler. Returned verbatim to MCP callers;
/// the CLI renders it as human-readable text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// The structured proposal artifact.
    pub proposal: GraphProposal,
    /// Stable proposal token (the bundle hash). Returned so MCP callers
    /// can pass it back to `dec_verify_graph_accept`.
    pub proposal_token: String,
    /// Pre-persist coverage roll-up (mirrors the matcher's view).
    pub coverage_preview: CoverageReportSummary,
    /// Set iff the proposal was persisted in this call (CLI `--accept`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persisted: Option<PersistedSummary>,
}

/// Persistence summary returned when the handler wrote a new graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSummary {
    /// Minted graph id (e.g. `VG-007`).
    pub graph_id: String,
    /// Absolute path of the on-disk Turtle.
    pub graph_path: PathBuf,
    /// Post-persist coverage roll-up.
    pub coverage_report: CoverageReportSummary,
}

/// Request for the accept handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptRequest {
    /// The proposal payload echoed back from a prior `generate` call.
    pub proposal: GraphProposal,
    /// The bundle-hash token issued at generate time (must match the
    /// proposal's `bundle_hash`).
    pub proposal_token: String,
    /// Short feature id (used to re-run the matcher for stale detection).
    pub feature_id: String,
    /// Short env id.
    pub environment_id: String,
    /// Working directory containing `.dec/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
    /// Optional product-root override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_root: Option<PathBuf>,
}

/// Outcome of the accept handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptResponse {
    /// Persistence summary (graph id + path + final coverage).
    pub persisted: PersistedSummary,
}

/// Single handler for the generate verb. Surface adapters (CLI + MCP)
/// route through this function per ADR-029 §Single-handler discipline.
pub fn run_generate(req: &GenerateRequest) -> Result<GenerateResponse, HandlerError> {
    let workdir = require_workdir(req.workdir.as_deref())?;
    let product_root = req.product_root.as_deref().unwrap_or(workdir);

    // 1. Compute the matcher report (no I/O beyond the orchestration store).
    let report = run_matcher(workdir, product_root, &req.feature_id, &req.environment_id)?;

    // 2. Match-first dispatch — if a complete match exists AND no
    //    actionable defect feedback exists for this (feature, env)
    //    pair, skip the worker entirely (TC-080 AC #2).
    //
    //    FT-107: when defect feedback exists, fall through to worker
    //    dispatch with the feedback in the bundle so the worker can
    //    re-author from runtime evidence.
    if matches!(
        report.kind,
        MatchKind::CompleteSingle | MatchKind::CompleteMultiple
    ) {
        let defects = defect_feedback::load_for(workdir, &req.feature_id, &req.environment_id);
        if defects.is_empty() {
            return Ok(build_match_response(&report));
        }
    }

    // 3. Resolve the verify-graph-author capability (FT-068) before
    //    assembling the bundle — the worker needs endpoint + model_id
    //    on every dispatch (ADR-033 / ADR-037).
    let capability = resolve_verify_graph_author_capability(workdir)?;

    // 4. Assemble the bundle and invoke the worker, with a single
    //    validator-retry pass: when the FT-102 chokepoint or the FT-107
    //    feedback-rejection guard fails on the first call, append the
    //    diagnostic messages to the bundle's metadata.warnings and
    //    re-call the worker once. The model sees its own violations on
    //    the retry and almost always corrects.
    let env_short = env_iri_to_short(&report.environment);
    let bundle = assemble_bundle(
        workdir,
        product_root,
        &req.feature_id,
        &env_short,
        &report,
        &capability,
    )?;
    let proposal = invoke_with_validator_retry(bundle, /* max_retries = */ 1)?;

    let preview = coverage_preview_from_report(&report);
    finalize_generate(req, workdir, &env_short, proposal, preview)
}

/// FT-110 worker-quality follow-up: wrap the worker call + validators
/// in a bounded retry loop. On the first attempt, run the worker and
/// the two validators normally. On a validator failure, build a retry
/// bundle whose `enrichment.bundle_metadata.warnings` carries the
/// violation messages — the worker's prompt renders that block, so the
/// model sees its previous output's diagnostic on the next pass.
///
/// Returns the first proposal that passes both validators, or the last
/// validator error after `max_retries` retries.
fn invoke_with_validator_retry(
    initial_bundle: bundle::VerifyGraphAuthorInputJson,
    max_retries: usize,
) -> Result<proposal::GraphProposal, HandlerError> {
    let mut bundle = initial_bundle;
    let mut attempt: usize = 0;
    loop {
        let proposal = worker::invoke_worker(&bundle)?;
        verify_bundle_hash(&proposal, &bundle)?;
        let validate_result = run_post_worker_validators(&proposal, &bundle);
        match validate_result {
            Ok(()) => return Ok(proposal),
            Err(err) => {
                if attempt >= max_retries {
                    return Err(err);
                }
                attempt += 1;
                bundle = retry_bundle_with_violation(bundle, &err);
            }
        }
    }
}

/// Run both post-worker validators in order. Returns the first error
/// encountered, or `Ok(())` if both pass.
fn run_post_worker_validators(
    proposal: &proposal::GraphProposal,
    bundle: &bundle::VerifyGraphAuthorInputJson,
) -> Result<(), HandlerError> {
    reject_match_when_feedback_present(proposal, bundle)?;
    apply_chokepoint_validator(proposal, &bundle.enrichment)?;
    Ok(())
}

/// Build the retry bundle: same shape as `bundle`, but with the
/// previous attempt's violation message appended to
/// `enrichment.bundle_metadata.warnings`. The worker prompt renders
/// warnings prominently (ADR-066 / FT-102), so the model sees its
/// own error on the next call. Bundle hash recomputes after the
/// mutation so the worker's echo passes.
fn retry_bundle_with_violation(
    mut bundle: bundle::VerifyGraphAuthorInputJson,
    err: &HandlerError,
) -> bundle::VerifyGraphAuthorInputJson {
    let hint = format!(
        "RETRY: your previous proposal was rejected by the dispatch-time validator. \
         {err}. Address every cited violation before responding — out-of-bundle \
         references CANNOT be persisted."
    );
    bundle.enrichment.bundle_metadata.warnings.push(hint);
    bundle.bundle_hash = bundle::compute_bundle_hash_pub(&bundle);
    bundle
}

/// FT-107 — when the bundle carries defect feedback, refuse two
/// degenerate worker responses:
///
/// 1. `kind = Match` against the broken graph: the worker saw the
///    runtime evidence and chose to defer anyway.
/// 2. `kind = New` with an empty `addressed_feedback_iris`: the worker
///    produced a fresh graph but didn't cite which feedback drove its
///    design. Without the citations the accept path has nothing to
///    transition to `addressed`, so the broken-feedback loop never
///    closes. Treat the omission as a contract violation rather than a
///    soft warning — schema-level enforcement isn't expressible
///    cross-payload in JSON Schema (the constraint depends on the
///    *input* bundle), so we enforce it server-side at the same seam
///    as the Match rejection.
fn reject_match_when_feedback_present(
    proposal: &GraphProposal,
    bundle: &bundle::VerifyGraphAuthorInputJson,
) -> Result<(), HandlerError> {
    if bundle.defect_feedback.is_empty() {
        return Ok(());
    }
    let iris: Vec<String> = bundle
        .defect_feedback
        .iter()
        .map(|fb| fb.feedback_iri.clone())
        .collect();
    match proposal.kind {
        ProposalKind::Match => Err(HandlerError::WorkerIgnoredFeedback {
            feedback_iris: iris.clone(),
            detail: format!(
                "verify-graph-author returned kind=Match despite the bundle carrying \
                 {n} defect-feedback entries ({iris:?}); the worker must respond with \
                 kind=New (or Gap) and address the runtime evidence",
                n = iris.len(),
            ),
        }),
        ProposalKind::New => {
            let cited = proposal
                .new
                .as_ref()
                .map(|n| n.addressed_feedback_iris.len())
                .unwrap_or(0);
            if cited == 0 {
                Err(HandlerError::WorkerIgnoredFeedback {
                    feedback_iris: iris.clone(),
                    detail: format!(
                        "verify-graph-author returned kind=New with an empty \
                         addressed_feedback_iris despite the bundle carrying {n} \
                         defect-feedback entries ({iris:?}); the worker must cite \
                         at least one feedback IRI in addressed_feedback_iris so the \
                         accept path can transition it from produced to addressed",
                        n = iris.len(),
                    ),
                })
            } else {
                Ok(())
            }
        }
        // `Gap` is acceptable: the worker is honestly saying it cannot
        // address the feedback with the available vocabulary, which is
        // a useful diagnostic rather than a contract violation.
        ProposalKind::Gap => Ok(()),
    }
}

fn apply_chokepoint_validator(
    proposal: &GraphProposal,
    enrichment: &enrichment::EnrichmentFields,
) -> Result<(), HandlerError> {
    let violations = validator::validate_proposal(proposal, enrichment);
    if violations.is_empty() {
        return Ok(());
    }
    // Emit one feedback per natural upstream target so the operator's
    // inbox has one actionable item per catalog edit (ADR-066 §Rule 3).
    let _ = feedback::emit_gap_feedback(&violations);
    Err(validator::build_rejection_error(&violations))
}

fn require_workdir(workdir: Option<&Path>) -> Result<&Path, HandlerError> {
    workdir.ok_or_else(|| HandlerError::InvalidArgument {
        field: "workdir".to_string(),
        detail: "no working directory available; run from a `dec init`-bootstrapped tree"
            .to_string(),
    })
}

/// FT-068 — open the orchestration store and resolve the
/// verify-graph-author role's default capability. Errors map onto the
/// shared `capability:` prefix the dispatcher (FT-061) uses, so
/// operator-facing messages stay consistent across `dec implement` and
/// `dec verify graph generate`.
fn resolve_verify_graph_author_capability(
    workdir: &Path,
) -> Result<ResolvedCapability, HandlerError> {
    let dump = orchestration_dump_path(workdir);
    let store: Store = load_store_from_dump(&dump).map_err(|e| HandlerError::Internal {
        detail: format!(
            "capability: opening orchestration store at {p}: {e}",
            p = dump.display()
        ),
    })?;
    resolve_default_capability(&store, "verify-graph-author").map_err(resolver_to_handler_error)
}

fn resolver_to_handler_error(err: ResolverError) -> HandlerError {
    let detail = match &err {
        ResolverError::NoActiveBinding { .. } => format!(
            "capability: {err}; run `dec init` (fresh tree) or seed via \
             `python3 scripts/bootstrap_catalog.py`"
        ),
        _ => format!("capability: {err}"),
    };
    HandlerError::Internal { detail }
}

fn verify_bundle_hash(
    proposal: &GraphProposal,
    bundle: &bundle::VerifyGraphAuthorInputJson,
) -> Result<(), HandlerError> {
    if proposal.bundle_hash == bundle.bundle_hash {
        return Ok(());
    }
    Err(HandlerError::Internal {
        detail: format!(
            "worker protocol violation: proposal.bundle_hash ({pp:?}) != \
             input bundle_hash ({bp:?})",
            pp = proposal.bundle_hash,
            bp = bundle.bundle_hash
        ),
    })
}

/// Single handler for the accept verb (companion to `run_generate`).
pub fn run_accept(req: &AcceptRequest) -> Result<AcceptResponse, HandlerError> {
    let workdir = require_workdir(req.workdir.as_deref())?;
    let product_root = req.product_root.as_deref().unwrap_or(workdir);
    verify_proposal_token(req)?;

    // Re-run the matcher and refuse stale proposals.
    let report = run_matcher(workdir, product_root, &req.feature_id, &req.environment_id)?;
    reject_if_complete_match(&report, &req.feature_id, &req.environment_id)?;

    // Persist (only `New` proposals reach this path).
    let env_short = env_iri_to_short(&report.environment);
    let persisted = persist_for_accept(req, workdir, &env_short)?;
    // FT-107: transition any cited defect feedback to `addressed` with
    // the new graph as the addressing artifact. Best-effort — failure
    // logs but does not unwind the persisted proposal.
    transition_addressed_feedback(workdir, &req.proposal, &persisted);
    // FT-100: graph is now whole and ready; fire the auto-dispatch.
    fire_graph_accepted_dispatch(workdir, &persisted.graph_id);
    Ok(AcceptResponse { persisted })
}

/// FT-107 — best-effort hook that walks `proposal.new.addressed_feedback_iris`
/// and transitions each cited feedback to `addressed` with the freshly
/// persisted graph as the addressing artifact. Used by both `run_accept`
/// (MCP path) and the CLI `--accept` branch.
pub(super) fn transition_addressed_feedback(
    workdir: &Path,
    proposal: &GraphProposal,
    persisted: &PersistedSummary,
) {
    let Some(new_payload) = proposal.new.as_ref() else {
        return;
    };
    if new_payload.addressed_feedback_iris.is_empty() {
        return;
    }
    let graph_iri =
        crate::core::ontology::verification_graph::types::graph_iri_for(&persisted.graph_id);
    let session_iri = oxigraph::model::NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/activity/verify-graph-generate/{graph}",
        graph = persisted.graph_id
    ));
    let now = chrono::Utc::now().to_rfc3339();
    // FT-109.C: materialise the dispatch session BEFORE we transition
    // feedback through it. The transition writes `dec:receivingSession`
    // pointing at this IRI; without the typing quads `dec session list`
    // never sees it. The helper is idempotent — a second call against
    // the same VG-NNN is a no-op.
    if let Err(e) = crate::core::dispatch_session::materialize(
        workdir,
        &session_iri,
        "verify-graph-author",
        // The dispatch was scoped to the proposal's feature; the
        // persisted summary doesn't carry the feature_id but the
        // proposal IRI naming carries the VG short id — use that as a
        // proxy when the feature isn't available.
        &persisted.graph_id,
        &now,
        &now,
        crate::core::dispatch_session::DispatchStatus::Completed,
    ) {
        tracing::warn!(
            target: "verify_graph_generate",
            graph = %persisted.graph_id,
            err = %e,
            "dispatch-session materialise failed (best-effort)"
        );
    }
    if let Err(e) = feedback_close::mark_batch_addressed(
        workdir,
        &new_payload.addressed_feedback_iris,
        &graph_iri,
        "verify-graph-author",
        &session_iri,
        &now,
    ) {
        tracing::warn!(
            target: "verify_graph_generate",
            graph = %persisted.graph_id,
            err = %e,
            "feedback-close transition failed (best-effort)"
        );
    }
}

/// Best-effort FT-100 hook: invoke `graph_accepted_dispatch::dispatch_for_graph`
/// after a complete graph has been persisted. Errors are logged, never
/// propagated — the persistence already succeeded.
pub(super) fn fire_graph_accepted_dispatch(workdir: &Path, graph_id: &str) {
    if let Err(e) = crate::core::subscriptions::dispatch_for_graph(workdir, graph_id) {
        tracing::warn!(
            target: "verify_graph_generate",
            graph = %graph_id,
            err = %e,
            "graph_accepted_dispatch failed (best-effort)"
        );
    }
}

fn verify_proposal_token(req: &AcceptRequest) -> Result<(), HandlerError> {
    if req.proposal.bundle_hash == req.proposal_token {
        return Ok(());
    }
    Err(HandlerError::ProposalStale {
        detail: format!(
            "proposal_token ({tok:?}) does not match proposal.bundle_hash ({ph:?}); \
             re-run dec_verify_graph_generate to issue a fresh proposal",
            tok = req.proposal_token,
            ph = req.proposal.bundle_hash
        ),
    })
}

fn reject_if_complete_match(
    report: &MatchReport,
    feature_id: &str,
    environment_id: &str,
) -> Result<(), HandlerError> {
    if !matches!(
        report.kind,
        MatchKind::CompleteSingle | MatchKind::CompleteMultiple
    ) {
        return Ok(());
    }
    Err(HandlerError::ProposalStale {
        detail: format!(
            "the candidate set for ({feature_id}, {environment_id}) has changed since this proposal \
             was issued; re-run dec_verify_graph_generate"
        ),
    })
}

fn persist_for_accept(
    req: &AcceptRequest,
    workdir: &Path,
    env_short: &str,
) -> Result<PersistedSummary, HandlerError> {
    let persisted = match req.proposal.kind {
        ProposalKind::New => persist_if_new(&req.proposal, workdir, &req.feature_id, env_short)?,
        ProposalKind::Match => {
            return Err(refuse_proposal_kind(
                "Match",
                "they have nothing to persist",
            ))
        }
        ProposalKind::Gap => {
            return Err(refuse_proposal_kind("Gap", "there is no graph to persist"))
        }
    };
    persisted.ok_or_else(|| HandlerError::Internal {
        detail: "persist_if_new returned None for a New proposal".to_string(),
    })
}

fn refuse_proposal_kind(kind: &str, reason: &str) -> HandlerError {
    HandlerError::InvalidArgument {
        field: "proposal.kind".to_string(),
        detail: format!("accept refuses {kind} proposals — {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_request_round_trips() {
        let r = GenerateRequest {
            feature_id: "FT-049".to_string(),
            environment_id: "BNCH-001-ephemeral-cli".to_string(),
            mode: GenerateMode::Accept,
            workdir: Some(PathBuf::from("/tmp")),
            product_root: None,
        };
        let v = serde_json::to_value(&r).expect("ser");
        let back: GenerateRequest = serde_json::from_value(v).expect("de");
        assert_eq!(r, back);
    }

    #[test]
    fn descriptors_have_canonical_names() {
        assert_eq!(generate_tool_descriptor().name, TOOL_NAME_GENERATE);
        assert_eq!(accept_tool_descriptor().name, TOOL_NAME_ACCEPT);
    }
}
