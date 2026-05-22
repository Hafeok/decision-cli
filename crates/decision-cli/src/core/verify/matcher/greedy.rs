//! Deterministic greedy minimum-cover selection (FT-046).
//!
//! Algorithm per FT-046 §Behaviour step 6:
//!   - At each step, pick the candidate covering the most
//!     still-uncovered TCs.
//!   - Ties broken by ascending `VG-NNN` numeric suffix.
//!   - Stop when residual is empty (CompleteMultiple) or when no
//!     candidate can extend the cover (Partial).
//!
//! Two runs over the same store produce byte-equal output (TC-071).

use std::collections::BTreeSet;

use super::candidates::Candidate;
use crate::core::verify::coverage::TcId;

/// Compute a numeric sort key for `VG-NNN[-suffix]` ids: numeric tail
/// first so `VG-3` orders before `VG-10`; the original string sorts
/// ids with identical numeric tails. Mirrors `query::graph_sort_key`
/// from FT-042 but kept here so the matcher does not reach across
/// feature boundaries (ADR-016).
#[must_use]
pub(super) fn numeric_suffix_key(id: &str) -> (u64, String) {
    let tail = id.strip_prefix("VG-").unwrap_or(id);
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    let n = digits.parse::<u64>().unwrap_or(u64::MAX);
    (n, id.to_string())
}

/// Pick a minimum cover from `non_empty` against `all_tcs`. Inputs
/// are pre-sorted by ascending `VG-NNN` numeric suffix in
/// [`super::query::graphs_in_env`]; this preserves that order, which
/// is the source of the tiebreak determinism.
pub(super) fn minimum_cover<'a>(
    non_empty: &'a [Candidate],
    all_tcs: &[TcId],
) -> Vec<&'a Candidate> {
    let mut residual: BTreeSet<&str> = all_tcs.iter().map(String::as_str).collect();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    let mut chosen: Vec<&Candidate> = Vec::new();

    loop {
        let Some(idx) = pick_best_candidate(non_empty, &used, &residual) else {
            // No candidate can extend the cover.
            break;
        };
        let pick = &non_empty[idx];
        for tc in &pick.2 {
            residual.remove(tc.as_str());
        }
        used.insert(pick.1.as_str());
        chosen.push(pick);
        if residual.is_empty() {
            break;
        }
    }
    chosen
}

/// Score every still-unused candidate by how many *new* TCs it would
/// add. Best score wins; ties broken by the candidate's position in the
/// input list (suffix-sorted upstream). Returns the chosen index, or
/// `None` when no candidate can extend the cover.
fn pick_best_candidate(
    non_empty: &[Candidate],
    used: &BTreeSet<&str>,
    residual: &BTreeSet<&str>,
) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None; // (score, index)
    for (i, cand) in non_empty.iter().enumerate() {
        if used.contains(cand.1.as_str()) {
            continue;
        }
        let score = cand
            .2
            .iter()
            .filter(|tc| residual.contains(tc.as_str()))
            .count();
        if score == 0 {
            continue;
        }
        match best {
            None => best = Some((score, i)),
            Some((s, _)) if score > s => best = Some((score, i)),
            _ => {} // lower or equal score with earlier index: keep earlier
        }
    }
    best.map(|(_, idx)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::verify::coverage::CoverageHit;

    #[test]
    fn numeric_suffix_key_orders_by_number_not_lexically() {
        let mut ids = vec!["VG-10", "VG-3", "VG-007", "VG-2"];
        ids.sort_by_key(|s| numeric_suffix_key(s));
        // VG-002 < VG-003 < VG-007 < VG-010 by numeric tail. The
        // sort key (n, original) keeps "VG-007" < "VG-10" because
        // 7 < 10 regardless of the suffix string length.
        assert_eq!(ids, vec!["VG-2", "VG-3", "VG-007", "VG-10"]);
    }

    #[test]
    fn minimum_cover_picks_two_graph_solution_over_three() {
        // TC-071 scenario: VG-3=[T1,T2], VG-5=[T2,T3], VG-7=[T3,T4],
        // VG-9=[T4]. Greedy with deterministic suffix tiebreak picks
        // VG-3 then VG-7 (cover size 2), not VG-5+VG-3+VG-9 (size 3).
        let tcs: Vec<String> = vec!["T1", "T2", "T3", "T4"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let mk = |id: &str, covers: &[&str]| {
            (
                "feat".to_string(),
                format!("https://decision-cli.dev/ns/graph/{id}"),
                covers.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
                Vec::<CoverageHit>::new(),
            )
        };
        let cands = vec![
            mk("VG-3", &["T1", "T2"]),
            mk("VG-5", &["T2", "T3"]),
            mk("VG-7", &["T3", "T4"]),
            mk("VG-9", &["T4"]),
        ];
        let pick = minimum_cover(&cands, &tcs);
        let ids: Vec<&str> = pick.iter().map(|c| c.1.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "https://decision-cli.dev/ns/graph/VG-3",
                "https://decision-cli.dev/ns/graph/VG-7",
            ]
        );
    }

    #[test]
    fn minimum_cover_returns_partial_when_no_candidate_extends() {
        let tcs: Vec<String> = vec!["T1", "T2"].into_iter().map(str::to_string).collect();
        let cands = vec![(
            "feat".to_string(),
            "https://decision-cli.dev/ns/graph/VG-1".to_string(),
            vec!["T1".to_string()],
            Vec::<CoverageHit>::new(),
        )];
        let pick = minimum_cover(&cands, &tcs);
        assert_eq!(pick.len(), 1);
        assert_eq!(pick[0].1, "https://decision-cli.dev/ns/graph/VG-1");
    }
}
