//! TC-153 — `aggregate_verdict()` composes multi-graph results per ADR-028
//!          with rejection dominance and gap reporting.
//!
//! Spec: `.product/tests/TC-153-aggregate-verdict-composes-multi-graph-results-per.md`
//! Validates: FT-097 · ADR-028.
//!
//! Pure-function tests over in-memory `VerificationGraphResult`s.

use std::collections::BTreeSet;

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::{
    EvidenceProjection, StepOutcome, VerificationGraphResult,
};
use decision_cli::core::verify::{aggregate_verdict, AggregationTarget};

const FEATURE_IRI: &str = "https://decision-cli.dev/ns/feature/FT-FIXTURE";
const TC_A: &str = "https://decision-cli.dev/ns/tc/TC-A";
const TC_B: &str = "https://decision-cli.dev/ns/tc/TC-B";

fn vgr(id: &str, verdict: Verdict, covers: &[&str]) -> VerificationGraphResult {
    let evidence_for: Vec<EvidenceProjection> = covers
        .iter()
        .map(|tc| EvidenceProjection {
            tc: (*tc).into(),
            outcome: match verdict {
                Verdict::Approved => StepOutcome::Pass,
                Verdict::Rejected => StepOutcome::Fail,
                Verdict::AmendmentRequired => StepOutcome::Unrunnable,
            },
            from_step: format!("https://decision-cli.dev/ns/result/{id}/step/0"),
        })
        .collect();
    VerificationGraphResult {
        id: format!("https://decision-cli.dev/ns/result/{id}"),
        result_of: "https://decision-cli.dev/ns/graph/VG-FIXTURE".into(),
        ran_in_environment: "https://decision-cli.dev/ns/bench/BNCH-001".into(),
        verdict,
        started_at: "2026-05-26T00:00:00Z".into(),
        ended_at: "2026-05-26T00:00:01Z".into(),
        step_traces: Vec::new(),
        evidence_for,
        rationale: "synthetic fixture verdict used in TC-153".into(),
        was_generated_by: "https://decision-cli.dev/ns/activity/run/synth".into(),
        was_attributed_to: "https://decision-cli.dev/ns/agent/runner".into(),
        created_at: "2026-05-26T00:00:01Z".into(),
    }
}

fn feature_target() -> AggregationTarget {
    AggregationTarget::Feature {
        feature: FEATURE_IRI.into(),
        tests: vec![TC_A.into(), TC_B.into()],
    }
}

fn as_set(v: &[String]) -> BTreeSet<String> {
    v.iter().cloned().collect()
}

#[test]
fn tc_153_aggregate_verdict_composes_multi_graph_results_per() {
    feature_row_1_empty_results();
    feature_row_2_two_results_each_covers_one_tc_all_approved();
    feature_row_3_one_result_one_tc_uncovered();
    feature_row_4_rejection_dominates();
    feature_row_5_mix_approved_and_amendment();
    feature_row_6_all_amendment_required();
    feature_row_7_redundant_approved_cover();
    tc_row_8_single_approved();
    tc_row_9_rejection_dominates();
    tc_row_10_uncovered_tc();
}

fn feature_row_1_empty_results() {
    let agg = aggregate_verdict(feature_target(), &[]);
    assert_eq!(agg.verdict, Verdict::Rejected, "row 1: verdict");
    assert!(
        agg.rationale.contains("no verification graph result covers"),
        "row 1: rationale = {:?}",
        agg.rationale
    );
    assert!(
        agg.rationale.contains(FEATURE_IRI),
        "row 1: rationale must name feature"
    );
    assert_eq!(
        as_set(&agg.coverage_gaps),
        as_set(&[TC_A.into(), TC_B.into()]),
        "row 1: coverage_gaps must include both TCs"
    );
    assert!(
        agg.contributing_results.is_empty(),
        "row 1: contributing_results empty"
    );
}

fn feature_row_2_two_results_each_covers_one_tc_all_approved() {
    let r1 = vgr("VGR-A", Verdict::Approved, &[TC_A]);
    let r2 = vgr("VGR-B", Verdict::Approved, &[TC_B]);
    let agg = aggregate_verdict(feature_target(), &[r1.clone(), r2.clone()]);
    assert_eq!(agg.verdict, Verdict::Approved, "row 2: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 2: no gaps");
    let contributors = as_set(&agg.contributing_results);
    assert!(contributors.contains(&r1.id), "row 2: contributors include r1");
    assert!(contributors.contains(&r2.id), "row 2: contributors include r2");
}

fn feature_row_3_one_result_one_tc_uncovered() {
    let r1 = vgr("VGR-A", Verdict::Approved, &[TC_A]);
    let agg = aggregate_verdict(feature_target(), &[r1]);
    assert_eq!(agg.verdict, Verdict::Rejected, "row 3: verdict");
    assert_eq!(
        as_set(&agg.coverage_gaps),
        as_set(&[TC_B.into()]),
        "row 3: TC-B uncovered"
    );
}

fn feature_row_4_rejection_dominates() {
    let approved = vgr("VGR-APP", Verdict::Approved, &[TC_A, TC_B]);
    let rejected = vgr("VGR-REJ", Verdict::Rejected, &[TC_A]);
    let agg = aggregate_verdict(feature_target(), &[approved, rejected.clone()]);
    assert_eq!(agg.verdict, Verdict::Rejected, "row 4: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 4: no gaps");
    let contributors = as_set(&agg.contributing_results);
    assert!(
        contributors.contains(&rejected.id),
        "row 4: rejecting VGR must contribute"
    );
}

fn feature_row_5_mix_approved_and_amendment() {
    let approved = vgr("VGR-APP", Verdict::Approved, &[TC_A, TC_B]);
    let amend = vgr("VGR-AMEND", Verdict::AmendmentRequired, &[TC_A]);
    let agg = aggregate_verdict(feature_target(), &[approved, amend]);
    assert_eq!(agg.verdict, Verdict::AmendmentRequired, "row 5: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 5: no gaps");
}

fn feature_row_6_all_amendment_required() {
    let a = vgr("VGR-A", Verdict::AmendmentRequired, &[TC_A]);
    let b = vgr("VGR-B", Verdict::AmendmentRequired, &[TC_B]);
    let agg = aggregate_verdict(feature_target(), &[a, b]);
    assert_eq!(agg.verdict, Verdict::AmendmentRequired, "row 6: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 6: no gaps");
}

fn feature_row_7_redundant_approved_cover() {
    let a = vgr("VGR-A", Verdict::Approved, &[TC_A, TC_B]);
    let b = vgr("VGR-B", Verdict::Approved, &[TC_A, TC_B]);
    let agg = aggregate_verdict(feature_target(), &[a, b]);
    assert_eq!(agg.verdict, Verdict::Approved, "row 7: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 7: no gaps");
    assert_eq!(
        agg.contributing_results.len(),
        2,
        "row 7: both VGRs contribute"
    );
}

fn tc_row_8_single_approved() {
    let r = vgr("VGR-A", Verdict::Approved, &[TC_A]);
    let agg = aggregate_verdict(AggregationTarget::Tc(TC_A.into()), &[r.clone()]);
    assert_eq!(agg.verdict, Verdict::Approved, "row 8: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 8: no gaps");
    assert_eq!(agg.contributing_results, vec![r.id], "row 8: contributors");
}

fn tc_row_9_rejection_dominates() {
    let approved = vgr("VGR-APP", Verdict::Approved, &[TC_A]);
    let rejected = vgr("VGR-REJ", Verdict::Rejected, &[TC_A]);
    let agg = aggregate_verdict(
        AggregationTarget::Tc(TC_A.into()),
        &[approved, rejected.clone()],
    );
    assert_eq!(agg.verdict, Verdict::Rejected, "row 9: verdict");
    assert!(agg.coverage_gaps.is_empty(), "row 9: no gaps");
    assert_eq!(
        agg.contributing_results,
        vec![rejected.id],
        "row 9: only rejecting VGR contributes"
    );
}

fn tc_row_10_uncovered_tc() {
    let r = vgr("VGR-B", Verdict::Approved, &[TC_B]);
    let agg = aggregate_verdict(AggregationTarget::Tc(TC_A.into()), &[r]);
    assert_eq!(agg.verdict, Verdict::Rejected, "row 10: verdict");
    assert!(
        agg.rationale.contains("no verification graph result covers"),
        "row 10: rationale = {:?}",
        agg.rationale
    );
    assert!(
        agg.rationale.contains(TC_A),
        "row 10: rationale must name TC-A"
    );
    assert_eq!(
        as_set(&agg.coverage_gaps),
        as_set(&[TC_A.into()]),
        "row 10: TC-A in coverage_gaps"
    );
    assert!(
        agg.contributing_results.is_empty(),
        "row 10: contributing_results empty for uncovered target"
    );
}
