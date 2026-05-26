//! TC-152 — Single-graph verdict derivation matches FT-097 rule for every
//!          step-outcome combination.
//!
//! Spec: `.product/tests/TC-152-single-graph-verdict-derivation-matches-ft-097-rul.md`
//! Validates: FT-097 · ADR-028.
//!
//! Table-driven assertion over every row in the TC's truth table. Each
//! row's failure message names the row index.

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::StepOutcome;
use decision_cli::core::verify::single_graph_verdict;

const TC_001: &str = "https://decision-cli.dev/ns/tc/TC-001";
const TC_002: &str = "https://decision-cli.dev/ns/tc/TC-002";
const TC_003: &str = "https://decision-cli.dev/ns/tc/TC-003";

struct Row {
    idx: u32,
    outcomes: Vec<StepOutcome>,
    evidence: Vec<Vec<String>>,
    expected: Verdict,
    rationale_must_contain: Vec<&'static str>,
}

fn rows() -> Vec<Row> {
    vec![
        // 1: [pass] / [[TC-001]] → approved, "all 1 steps passed"
        Row {
            idx: 1,
            outcomes: vec![StepOutcome::Pass],
            evidence: vec![vec![TC_001.into()]],
            expected: Verdict::Approved,
            rationale_must_contain: vec!["all 1 steps passed"],
        },
        // 2: [pass, pass, pass] / [[TC-001], [], [TC-002]] → approved
        Row {
            idx: 2,
            outcomes: vec![StepOutcome::Pass, StepOutcome::Pass, StepOutcome::Pass],
            evidence: vec![vec![TC_001.into()], vec![], vec![TC_002.into()]],
            expected: Verdict::Approved,
            rationale_must_contain: vec!["all 3 steps passed"],
        },
        // 3: [pass, fail] / [[TC-001], [TC-002]] → rejected, "step 1" + "TC-002"
        Row {
            idx: 3,
            outcomes: vec![StepOutcome::Pass, StepOutcome::Fail],
            evidence: vec![vec![TC_001.into()], vec![TC_002.into()]],
            expected: Verdict::Rejected,
            rationale_must_contain: vec!["step 1", "TC-002"],
        },
        // 4: [fail, pass] / [[], [TC-001]] → amendment-required, "step 0" + setup-cue
        Row {
            idx: 4,
            outcomes: vec![StepOutcome::Fail, StepOutcome::Pass],
            evidence: vec![vec![], vec![TC_001.into()]],
            expected: Verdict::AmendmentRequired,
            rationale_must_contain: vec!["step 0", "setup"],
        },
        // 5: [unrunnable] / [[TC-001]] → amendment-required, "unrunnable"
        Row {
            idx: 5,
            outcomes: vec![StepOutcome::Unrunnable],
            evidence: vec![vec![TC_001.into()]],
            expected: Verdict::AmendmentRequired,
            rationale_must_contain: vec!["unrunnable"],
        },
        // 6: [pass, unrunnable, pass] → amendment-required, "step 1" + "unrunnable"
        Row {
            idx: 6,
            outcomes: vec![
                StepOutcome::Pass,
                StepOutcome::Unrunnable,
                StepOutcome::Pass,
            ],
            evidence: vec![
                vec![TC_001.into()],
                vec![TC_002.into()],
                vec![TC_003.into()],
            ],
            expected: Verdict::AmendmentRequired,
            rationale_must_contain: vec!["step 1", "unrunnable"],
        },
        // 7: [fail, unrunnable] → rejected, "step 0" (fail dominates)
        Row {
            idx: 7,
            outcomes: vec![StepOutcome::Fail, StepOutcome::Unrunnable],
            evidence: vec![vec![TC_001.into()], vec![TC_002.into()]],
            expected: Verdict::Rejected,
            rationale_must_contain: vec!["step 0"],
        },
        // 8: [] → approved, "0 steps" (vacuous)
        Row {
            idx: 8,
            outcomes: vec![],
            evidence: vec![],
            expected: Verdict::Approved,
            rationale_must_contain: vec!["0 steps"],
        },
    ]
}

#[test]
fn tc_152_single_graph_verdict_derivation_matches_ft_097_rul() {
    for row in rows() {
        let (verdict, rationale) = single_graph_verdict(&row.outcomes, &row.evidence);
        assert_eq!(
            verdict, row.expected,
            "row {}: verdict mismatch (rationale={rationale:?})",
            row.idx
        );
        assert!(
            rationale.chars().count() >= 20,
            "row {}: rationale must be ≥ 20 chars (got {} chars: {rationale:?})",
            row.idx,
            rationale.chars().count()
        );
        for needle in &row.rationale_must_contain {
            assert!(
                rationale.contains(needle),
                "row {}: rationale {:?} must contain {:?}",
                row.idx,
                rationale,
                needle
            );
        }
    }
}
