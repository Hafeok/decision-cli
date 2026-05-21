//! Internal helpers for the verify_graph_generate handler (FT-049).
//!
//! Split out of `mod.rs` to keep file sizes within ADR-013 §Rule 1.

use std::path::Path;

use crate::core::handler::Error as HandlerError;
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::verify::coverage::feature_resolver::IRI_TC_PREFIX;
use crate::core::verify::matcher::{best_matching_graphs, MatchReport};
use crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX;

use super::proposal::CoverageReportSummary;

/// Load the orchestration store and run the matcher for `(feature, env)`.
pub(super) fn run_matcher(
    workdir: &Path,
    product_root: &Path,
    feature_id: &str,
    env_short: &str,
) -> Result<MatchReport, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("matcher: loading orchestration store: {e}"),
    })?;
    let _scope = ActiveScope::load(workdir).map_err(|e| HandlerError::Internal {
        detail: format!("matcher: loading active scope: {e}"),
    })?;
    best_matching_graphs(feature_id, env_short, &store, product_root).map_err(|e| match e {
        crate::core::verify::matcher::MatchError::ArtifactNotFound { kind, id } => {
            HandlerError::ArtifactNotFound { kind, id }
        }
        crate::core::verify::matcher::MatchError::StoreUnreachable { detail } => {
            HandlerError::Internal {
                detail: format!("matcher: {detail}"),
            }
        }
    })
}

/// Translate a `MatchReport` into the wire-shape coverage summary.
pub(super) fn coverage_preview_from_report(report: &MatchReport) -> CoverageReportSummary {
    CoverageReportSummary {
        covered: report
            .covered_by_match
            .iter()
            .map(|t| short_tc_id(t))
            .collect(),
        uncovered: report
            .residual_uncovered
            .iter()
            .map(|t| short_tc_id(t))
            .collect(),
        considered: report.graphs.iter().map(|g| short_graph_id(&g.id)).collect(),
    }
}

/// Strip the dec TC IRI prefix; pass through values that are already short.
pub(super) fn short_tc_id(iri: &str) -> String {
    iri.strip_prefix(IRI_TC_PREFIX).unwrap_or(iri).to_string()
}

/// Strip the dec graph IRI prefix; pass through values that are already short.
pub(super) fn short_graph_id(iri: &str) -> String {
    iri.strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
        .unwrap_or(iri)
        .to_string()
}

/// Bundle-hash placeholder for `Match` proposals so MCP's accept path
/// can detect a stale match by comparing against the current matcher
/// state (hash binds the matched graph id; changing the candidate set
/// changes the id and hence the hash).
pub(super) fn preview_bundle_hash_for_match(matched_graph_short: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"match:");
    h.update(matched_graph_short.as_bytes());
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}
