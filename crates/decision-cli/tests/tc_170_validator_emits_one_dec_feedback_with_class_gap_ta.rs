//! TC-170 — Validator emits one `dec:Feedback` with `class = "gap"`
//! targeted at the upstream catalog artifact, not the worker.
//!
//! Validates: FT-102 · ADR-066.
//! Spec: `.product/tests/TC-170-validator-emits-one-dec-feedback-with-class-gap-ta.md`
//!
//! The test exercises the gap-feedback emitter directly via the in-memory
//! capture buffer (`feedback::with_capture`). The contract proved:
//!
//! - One emitted record per upstream-target category (not one per violation).
//! - `class = "gap"` on every record.
//! - `target_role = "bundle-assembler"` — the gap is the upstream's
//!   problem, not the worker's (ADR-066 §Rule 3).
//! - Each record's `evidence` body names the violations it aggregates.
//! - Records' targets resolve to the natural catalog artifact category:
//!   `dec_subcommand` → CapabilityReference, `sparql_namespace` →
//!   OntologyDescription, `file_path` / `http_host` / `binary` →
//!   VerificationBench.

use decision_cli::verify_graph_generate::feedback::{
    drain_captured, emit_gap_feedback, with_capture,
};
use decision_cli::verify_graph_generate::validator::{
    UpstreamTarget, Violation, ViolationKind,
};

fn v(step: usize, kind: ViolationKind, thing: &str) -> Violation {
    Violation {
        step_index: step,
        kind,
        referenced_thing: thing.to_string(),
        why_rejected: "not in bundle".to_string(),
    }
}

#[test]
fn tc_170_validator_emits_one_dec_feedback_with_class_gap_ta() {
    scenario_a_single_dec_subcommand_violation();
    scenario_b_single_sparql_namespace_violation();
    scenario_c_multi_category_violations_one_per_target();
    scenario_d_each_record_carries_violation_detail();
    scenario_e_feedback_routes_to_bundle_assembler();
}

#[test]
fn scenario_a_single_dec_subcommand_violation() {
    let _g = with_capture();
    let records = emit_gap_feedback(&[v(
        0,
        ViolationKind::DecSubcommand,
        "dec verify result inspect",
    )]);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].class, "gap");
    assert_eq!(
        records[0].target,
        UpstreamTarget::CapabilityReference.as_str()
    );
    assert!(records[0]
        .evidence
        .contains("dec verify result inspect"));
    let drained = drain_captured();
    assert_eq!(drained.len(), 1, "capture buffer mirrors the emission");
}

#[test]
fn scenario_b_single_sparql_namespace_violation() {
    let _g = with_capture();
    let records = emit_gap_feedback(&[v(
        0,
        ViolationKind::SparqlNamespace,
        "https://fake.example/ns#",
    )]);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].class, "gap");
    assert_eq!(
        records[0].target,
        UpstreamTarget::OntologyDescription.as_str()
    );
    assert!(records[0]
        .evidence
        .contains("https://fake.example/ns#"));
}

#[test]
fn scenario_c_multi_category_violations_one_per_target() {
    let _g = with_capture();
    // Three violations in three different categories ⇒ three records.
    let records = emit_gap_feedback(&[
        v(0, ViolationKind::DecSubcommand, "dec verify result inspect"),
        v(1, ViolationKind::SparqlNamespace, "https://fake.example/ns#"),
        v(2, ViolationKind::FilePath, "/etc/passwd"),
    ]);
    assert_eq!(records.len(), 3, "one record per upstream-target category");
    let targets: Vec<&str> = records.iter().map(|r| r.target.as_str()).collect();
    assert!(targets.contains(&UpstreamTarget::CapabilityReference.as_str()));
    assert!(targets.contains(&UpstreamTarget::OntologyDescription.as_str()));
    assert!(targets.contains(&UpstreamTarget::VerificationBench.as_str()));
    for r in &records {
        assert_eq!(r.class, "gap");
        assert_eq!(r.target_role, "bundle-assembler");
    }
}

#[test]
fn scenario_d_each_record_carries_violation_detail() {
    let _g = with_capture();
    let records = emit_gap_feedback(&[
        v(0, ViolationKind::DecSubcommand, "dec foo bar"),
        v(3, ViolationKind::DecSubcommand, "dec baz qux"),
    ]);
    assert_eq!(records.len(), 1, "both DecSubcommand violations land in one record");
    assert_eq!(records[0].violations.len(), 2);
    // Evidence mentions both.
    assert!(records[0].evidence.contains("dec foo bar"));
    assert!(records[0].evidence.contains("dec baz qux"));
    assert!(
        !records[0].recommendation.is_empty(),
        "every record carries a remediation suggestion"
    );
}

#[test]
fn scenario_e_feedback_routes_to_bundle_assembler() {
    // Per ADR-066 §Rule 3, the gap is the upstream's responsibility —
    // the routing tag on every emitted record points at
    // `bundle-assembler`, NOT at any worker role.
    let _g = with_capture();
    let records = emit_gap_feedback(&[
        v(0, ViolationKind::Binary, "curl"),
        v(1, ViolationKind::DecSubcommand, "dec foo"),
        v(2, ViolationKind::SparqlNamespace, "https://x.example/"),
    ]);
    assert!(!records.is_empty());
    for r in &records {
        assert_eq!(
            r.target_role, "bundle-assembler",
            "every gap record routes to bundle-assembler, not the worker; \
             record: target={}, class={}, role={}",
            r.target, r.class, r.target_role,
        );
    }
}

#[test]
fn empty_violations_emit_no_records() {
    let _g = with_capture();
    let records = emit_gap_feedback(&[]);
    assert!(records.is_empty());
    let drained = drain_captured();
    assert!(drained.is_empty());
}
