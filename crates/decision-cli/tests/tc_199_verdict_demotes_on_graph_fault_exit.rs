//! TC-199 — verdict aggregator demotes Rejected → AmendmentRequired
//! when an evidence-bearing step's exit code is in
//! `GRAPH_FAULT_EXIT_CODES`. Code-regression exit codes still produce
//! Rejected.
//!
//! Validates: FT-110.

use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::core::ontology::verification_result::StepOutcome;
use decision_cli::core::verify::{
    single_graph_verdict, single_graph_verdict_with_exit_codes, GRAPH_FAULT_EXIT_CODES,
};

#[test]
fn tc_199_verdict_demotes_on_graph_fault_exit() {
    let evidence = vec![vec!["TC-T199a".to_string()]];
    let outcomes = vec![StepOutcome::Fail];

    // Graph-fault exit codes demote to AmendmentRequired.
    for &code in GRAPH_FAULT_EXIT_CODES {
        let (verdict, rationale) =
            single_graph_verdict_with_exit_codes(&outcomes, &evidence, &[Some(code)]);
        assert_eq!(
            verdict,
            Verdict::AmendmentRequired,
            "exit {code} should demote; got rationale: {rationale}"
        );
        assert!(
            rationale.contains(&format!("exit {code}")),
            "rationale should name the exit code: {rationale}"
        );
        assert!(
            rationale.contains("graph-design fault"),
            "rationale should diagnose graph-design fault: {rationale}"
        );
    }

    // Code-regression exit codes keep Rejected.
    for code in [1, 101, 124] {
        let (verdict, _) =
            single_graph_verdict_with_exit_codes(&outcomes, &evidence, &[Some(code)]);
        assert_eq!(
            verdict,
            Verdict::Rejected,
            "exit {code} should keep Rejected (real code regression)"
        );
    }

    // Missing exit code (None) — non-shell step kinds — keeps Rejected.
    let (verdict, _) = single_graph_verdict_with_exit_codes(&outcomes, &evidence, &[None]);
    assert_eq!(
        verdict,
        Verdict::Rejected,
        "None exit_code should not trigger demotion"
    );

    // Non-evidence step: failing setup is always AmendmentRequired,
    // exit code is irrelevant. (FT-097 behaviour preserved.)
    let no_evidence: Vec<Vec<String>> = vec![Vec::new()];
    let (verdict, _) = single_graph_verdict_with_exit_codes(&outcomes, &no_evidence, &[Some(1)]);
    assert_eq!(
        verdict,
        Verdict::AmendmentRequired,
        "non-evidence fail is always AmendmentRequired regardless of exit code"
    );

    // Back-compat wrapper — old two-arg shape keeps legacy verdict.
    let (verdict, _) = single_graph_verdict(&outcomes, &evidence);
    assert_eq!(
        verdict,
        Verdict::Rejected,
        "back-compat single_graph_verdict (no exit codes) keeps legacy Rejected"
    );
}
