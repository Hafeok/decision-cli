//! Post-matcher / post-worker glue for `dec verify graph generate`.
//!
//! Extracted from `mod.rs` to honour ADR-013 file-length limits. These
//! helpers convert a worker proposal (or a matcher early-return) into a
//! `GenerateResponse`, including the optional persistence pass for
//! `--accept` mode. Each function is a single concern; the handler
//! orchestrates them.

use std::path::Path;

use crate::core::handler::Error as HandlerError;
use crate::core::verify::matcher::MatchReport;

use super::internal::{coverage_preview_from_report, preview_bundle_hash_for_match, run_matcher, short_graph_id};
use super::persist::{persist_new_proposal, Persisted};
use super::proposal::{CoverageReportSummary, GraphProposal, MatchProposal, ProposalKind};
use super::{GenerateMode, GenerateRequest, GenerateResponse, PersistedSummary};

/// Build a `GenerateResponse` that short-circuits the worker because the
/// matcher already reported complete coverage (TC-080 AC #2).
pub(super) fn build_match_response(report: &MatchReport) -> GenerateResponse {
    let match_graph_short = report
        .graphs
        .first()
        .map(|g| short_graph_id(&g.id))
        .unwrap_or_else(|| "unknown".to_string());
    let preview = coverage_preview_from_report(report);
    let proposal = GraphProposal::new_match(
        preview_bundle_hash_for_match(&match_graph_short),
        MatchProposal {
            graph_id: match_graph_short,
            rationale: "An existing graph in this environment already covers \
                every TC the feature lists; no new graph needed."
                .to_string(),
        },
    );
    let token = proposal.bundle_hash.clone();
    GenerateResponse {
        proposal,
        proposal_token: token,
        coverage_preview: preview,
        persisted: None,
    }
}

/// Apply the request's mode to a fresh proposal, optionally persisting.
pub(super) fn finalize_generate(
    req: &GenerateRequest,
    workdir: &Path,
    env_short: &str,
    proposal: GraphProposal,
    preview: CoverageReportSummary,
) -> Result<GenerateResponse, HandlerError> {
    let token = proposal.bundle_hash.clone();
    match req.mode {
        GenerateMode::Accept => {
            let persisted = persist_if_new(&proposal, workdir, &req.feature_id, env_short)?;
            // FT-100: a whole new graph just landed; fire auto-dispatch.
            // Match-/Gap-kind proposals return None here and skip dispatch
            // (no fresh graph to run).
            if let Some(summary) = &persisted {
                super::fire_graph_accepted_dispatch(workdir, &summary.graph_id);
            }
            Ok(GenerateResponse {
                proposal,
                proposal_token: token,
                coverage_preview: preview,
                persisted,
            })
        }
        GenerateMode::Interactive | GenerateMode::PrintOnly => Ok(GenerateResponse {
            proposal,
            proposal_token: token,
            coverage_preview: preview,
            persisted: None,
        }),
    }
}

/// Persist a `New` proposal; return `None` for non-`New` kinds so the
/// caller can branch cleanly.
pub(super) fn persist_if_new(
    proposal: &GraphProposal,
    workdir: &Path,
    feature_id: &str,
    env_short: &str,
) -> Result<Option<PersistedSummary>, HandlerError> {
    let new_payload = match proposal.kind {
        ProposalKind::New => proposal
            .new
            .as_ref()
            .ok_or_else(|| HandlerError::Internal {
                detail: "proposal.kind = New but proposal.new payload missing".to_string(),
            })?,
        _ => return Ok(None),
    };
    let persisted = persist_new_proposal(workdir, feature_id, env_short, &new_payload.steps)?;
    Ok(Some(post_persist_summary(
        persisted, workdir, feature_id, env_short,
    )))
}

fn post_persist_summary(
    persisted: Persisted,
    workdir: &Path,
    feature_id: &str,
    env_short: &str,
) -> PersistedSummary {
    let coverage_report = match run_matcher(workdir, workdir, feature_id, env_short) {
        Ok(report) => coverage_preview_from_report(&report),
        Err(_) => CoverageReportSummary::default(),
    };
    PersistedSummary {
        graph_id: persisted.graph_id,
        graph_path: persisted.graph_path,
        coverage_report,
    }
}
