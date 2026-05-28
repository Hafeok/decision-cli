//! Per-graph coverage scoring for enumerated matcher candidates (FT-046).
//!
//! Reads every `dec:VerificationGraph` in the requested env via
//! [`super::query::graphs_in_env`], then asks FT-045's
//! [`feature_covered_by`] one candidate at a time to derive its
//! per-graph coverage subset. Empty coverage sets are dropped per
//! FT-046 §Behaviour step 4.

use std::collections::BTreeSet;
use std::path::Path;

use oxigraph::store::Store;

use super::query;
use super::MatchError;
use crate::core::verify::coverage::{feature_covered_by, CoverageHit, GraphId, TcId};
use crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX;

/// One candidate row: `(verifies_iri, graph_iri, deduped_covers, raw_hits)`.
pub(super) type Candidate = (String, GraphId, Vec<TcId>, Vec<CoverageHit>);

/// Strip the verify-graph IRI prefix to expose `VG-NNN[-suffix]`.
pub(super) fn graph_iri_to_short(iri: &str) -> &str {
    iri.strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX).unwrap_or(iri)
}

/// Project a covered-TC set onto the feature's declaration order.
pub(super) fn order_tcs(all_tcs: &[TcId], covers: &[TcId]) -> Vec<TcId> {
    let cov_set: BTreeSet<&str> = covers.iter().map(String::as_str).collect();
    all_tcs
        .iter()
        .filter(|t| cov_set.contains(t.as_str()))
        .cloned()
        .collect()
}

/// Enumerate graphs in `env_iri` and score each by its per-graph
/// coverage against `feature`'s TCs. Returns only candidates whose
/// coverage set is non-empty (FT-046 §Behaviour step 4).
pub(super) fn collect_non_empty(
    store: &Store,
    env_iri: &str,
    feature: &str,
    product_root: &Path,
) -> Result<Vec<Candidate>, MatchError> {
    let candidates = query::graphs_in_env(store, env_iri)?;
    let mut out: Vec<Candidate> = Vec::with_capacity(candidates.len());
    let workdir = product_root
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| product_root.to_path_buf());
    let graph_dir = workdir.join(".dec").join("verify").join("graph");
    for (graph_iri, verifies_iri) in &candidates {
        let report =
            feature_covered_by(feature, graph_iri_to_short(graph_iri), store, product_root)?;
        let covers: Vec<TcId> = report
            .covered
            .iter()
            .map(|h| h.tc.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if covers.is_empty() {
            continue;
        }
        // Disk-authoritative cross-check: only keep this candidate
        // if the graph's on-disk .ttl file actually has at least one
        // step whose dec:providesEvidenceFor matches one of the TCs
        // the store-side coverage query claims. Prevents stale
        // store-side providesEvidenceFor cruft from making the
        // matcher believe a graph covers a feature it cannot
        // actually emit evidence for at runtime.
        if !graph_has_disk_evidence_for(&graph_dir, graph_iri_to_short(graph_iri), &covers) {
            continue;
        }
        out.push((
            verifies_iri.clone(),
            graph_iri.clone(),
            covers,
            report.covered,
        ));
    }
    Ok(out)
}

/// Returns true iff `<graph_dir>/<graph_short>.ttl` parses cleanly
/// and at least one of its steps has `dec:providesEvidenceFor`
/// pointing at one of the listed TCs.
fn graph_has_disk_evidence_for(
    graph_dir: &Path,
    graph_short: &str,
    expected_tcs: &[TcId],
) -> bool {
    use crate::core::ontology::verification_graph::io::from_turtle;
    let path = graph_dir.join(format!("{graph_short}.ttl"));
    let Ok(graph) = from_turtle(&path) else {
        return false;
    };
    let expected: BTreeSet<&str> = expected_tcs.iter().map(String::as_str).collect();
    graph.steps.iter().any(|step| {
        step.provides_evidence_for
            .iter()
            .any(|tc| expected.contains(tc.as_str()))
    })
}
