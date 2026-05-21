//! `MatchReport` shape builders (FT-046).
//!
//! Three constructors mirroring the matcher's three terminal paths:
//! `none_report` (no candidate touches any TC), `single_report` (one
//! candidate covers everything — CompleteSingle), and `cover_report`
//! (greedy minimum cover that either completes or leaves residual).

use std::collections::BTreeSet;

use super::candidates::{graph_iri_to_short, order_tcs, Candidate};
use super::greedy::numeric_suffix_key;
use super::report::{EnvId, GraphSummary, MatchKind, MatchReport};
use crate::core::verify::coverage::TcId;

/// `MatchKind::None` outcome — every TC stays in `residual_uncovered`.
pub(super) fn none_report(
    feature_iri: String,
    env_iri: EnvId,
    all_tcs: Vec<TcId>,
) -> MatchReport {
    MatchReport {
        feature: feature_iri,
        environment: env_iri,
        kind: MatchKind::None,
        graphs: Vec::new(),
        covered_by_match: Vec::new(),
        residual_uncovered: all_tcs,
    }
}

/// Pick a single candidate that covers every TC, if one exists.
/// Ties broken by ascending `VG-NNN` numeric suffix.
pub(super) fn pick_complete_single<'a>(
    non_empty: &'a [Candidate],
    all_tcs: &[TcId],
) -> Option<&'a Candidate> {
    let all_set: BTreeSet<&str> = all_tcs.iter().map(String::as_str).collect();
    let mut singles: Vec<&Candidate> = non_empty
        .iter()
        .filter(|c| {
            let cov_set: BTreeSet<&str> = c.2.iter().map(String::as_str).collect();
            cov_set == all_set
        })
        .collect();
    if singles.is_empty() {
        return None;
    }
    singles.sort_by(|a, b| {
        numeric_suffix_key(graph_iri_to_short(&a.1))
            .cmp(&numeric_suffix_key(graph_iri_to_short(&b.1)))
    });
    Some(singles[0])
}

/// CompleteSingle outcome.
pub(super) fn single_report(
    feature_iri: String,
    env_iri: EnvId,
    all_tcs: &[TcId],
    chosen: &Candidate,
) -> MatchReport {
    let covered_in_order = order_tcs(all_tcs, &chosen.2);
    MatchReport {
        feature: feature_iri,
        environment: env_iri,
        kind: MatchKind::CompleteSingle,
        graphs: vec![GraphSummary::from_iris(
            &chosen.1,
            &chosen.0,
            covered_in_order.clone(),
        )],
        covered_by_match: covered_in_order,
        residual_uncovered: Vec::new(),
    }
}

/// CompleteMultiple / Partial outcome — built from a greedy cover.
pub(super) fn cover_report(
    feature_iri: String,
    env_iri: EnvId,
    all_tcs: &[TcId],
    cover: &[&Candidate],
) -> MatchReport {
    let mut covered_set: BTreeSet<String> = BTreeSet::new();
    let mut graphs_out: Vec<GraphSummary> = Vec::with_capacity(cover.len());
    for chosen in cover {
        for tc in &chosen.2 {
            covered_set.insert(tc.clone());
        }
        graphs_out.push(GraphSummary::from_iris(
            &chosen.1,
            &chosen.0,
            order_tcs(all_tcs, &chosen.2),
        ));
    }
    let covered_by_match: Vec<TcId> = all_tcs
        .iter()
        .filter(|t| covered_set.contains(*t))
        .cloned()
        .collect();
    let residual_uncovered: Vec<TcId> = all_tcs
        .iter()
        .filter(|t| !covered_set.contains(*t))
        .cloned()
        .collect();
    let kind = if residual_uncovered.is_empty() {
        MatchKind::CompleteMultiple
    } else {
        MatchKind::Partial
    };
    MatchReport {
        feature: feature_iri,
        environment: env_iri,
        kind,
        graphs: graphs_out,
        covered_by_match,
        residual_uncovered,
    }
}
