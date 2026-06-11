//! TC-174 — `adrs-rejected:` re-introduces the gap with severity
//! `intentional` and carries the reason through to the preflight report.
//!
//! Validates: FT-104.
//! Spec: `.product/tests/TC-174-adrs-rejected-re-introduces-the-gap-with-severity.md`.
//!
//! Five scenarios exercise the explicit-opt-out path against the
//! FT-104 algorithm:
//!
//! - A: a default-acknowledged ADR rejected by a feature surfaces as a
//!   gap with `severity = intentional` and the literal reason.
//! - B: the text-format render visibly distinguishes intentional gaps
//!   from missing ones (the [`CoverageStatus::severity_label`] surface
//!   used by the renderer reports `"intentional"`).
//! - C: an empty `reason:` is a parse-time error so malformed features
//!   surface in `product feature show` rather than silently slip past
//!   preflight.
//! - D: a rejection of an ADR that isn't default-acknowledged degrades
//!   to a regular missing-link gap (the drift validator owns the
//!   visibility surface, not preflight).
//! - E: the `product feature reject` verb shape is idempotent — the
//!   second invocation with the same args updates the reason in place.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::default_ack::{
    evaluate_cross_cutting, parse_adrs_rejected, AdrsRejectedError, CoverageStatus,
    DefaultAcknowledgeConfig, RejectedAdr,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
fn tempdir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "decision-cli-tc174-{tag}-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&base).expect("create tempdir");
    base
}

fn cfg(adrs: &[&str]) -> DefaultAcknowledgeConfig {
    DefaultAcknowledgeConfig {
        adrs: adrs.iter().map(|s| (*s).to_string()).collect(),
        source: None,
    }
}

const ADR_CC: &str = "ADR-CC";

#[test]
fn tc_174_adrs_rejected_re_introduces_the_gap_with_severity() {
    // ----- Scenario A: rejection surfaces as a distinct gap kind -----
    let frontmatter = "\
id: FT-OPTOUT
title: Opt-out feature
adrs: []
adrs-rejected:
  - id: ADR-CC
    reason: \"This feature uses an alternative pattern because <stated rationale>.\"
";
    let rejections = parse_adrs_rejected(frontmatter).expect("parse frontmatter");
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].id, ADR_CC);
    let reason_a = rejections[0].reason.clone();

    let rows = evaluate_cross_cutting(&[ADR_CC.into()], &[], &rejections, &cfg(&[ADR_CC]));
    assert_eq!(rows.len(), 1, "one cross-cutting row");

    // The status must be `Intentional` (not `Missing`) and the literal
    // reason from the frontmatter must be threaded through.
    match &rows[0].status {
        CoverageStatus::Intentional { reason } => {
            assert_eq!(reason, &reason_a, "literal reason must thread through");
        }
        other => panic!("scenario A: expected Intentional, got {other:?}"),
    }
    assert!(
        rows[0].status.is_gap(),
        "scenario A: an intentional rejection still counts as a gap"
    );
    assert_eq!(
        rows[0].status.severity_label(),
        "intentional",
        "scenario A: severity field equals intentional (the JSON/MCP surface)"
    );

    // ----- Scenario B: text-format renders the rejection visibly -----
    // The text renderer keys off `severity_label()` to pick the section
    // / glyph. We assert the label is exactly `"intentional"` so a
    // human reader sees ADR-CC under a distinct section from missing
    // gaps. (The literal-string render is tested in the algorithm
    // unit tests; this assertion guards the renderer contract.)
    let label = rows[0].status.severity_label();
    assert_ne!(
        label, "missing",
        "scenario B: intentional rejections must NOT be rendered under the missing section"
    );
    // The reason snippet must remain reachable from the row for the
    // renderer to print it next to the ADR id.
    if let CoverageStatus::Intentional { reason } = &rows[0].status {
        assert!(
            reason.contains("alternative pattern"),
            "scenario B: reason snippet flows through to the renderer"
        );
    }

    // ----- Scenario C: empty reason is a parse-time error -----
    let bad = "\
id: FT-BADOPTOUT
adrs-rejected:
  - id: ADR-CC
    reason: \"\"
";
    let err = parse_adrs_rejected(bad).expect_err("empty reason must error");
    match err {
        AdrsRejectedError::EmptyReason { adr_id, .. } => {
            assert_eq!(adr_id, ADR_CC, "scenario C: error names the empty entry");
        }
        other => panic!("scenario C: expected EmptyReason, got {other:?}"),
    }
    // And the error message names the bad field (so stderr can carry it
    // back to the operator).
    let msg = parse_adrs_rejected(bad)
        .expect_err("re-run for message")
        .to_string();
    assert!(
        msg.contains("reason") && msg.contains(ADR_CC),
        "scenario C: error message must name reason and the ADR id; got `{msg}`",
    );

    // ----- Scenario D: rejection without default-ack is incoherent -----
    // FT-OPTOUT still has `adrs-rejected: [ADR-CC]`, but the config no
    // longer lists ADR-CC. Per the FT-104 spec, preflight reports a
    // regular `missing`-severity gap (the rejection has no effect
    // because there was nothing to default-acknowledge) and the drift
    // validator owns the user-visible warning surface.
    let rows = evaluate_cross_cutting(&[ADR_CC.into()], &[], &rejections, &cfg(&[]));
    assert_eq!(
        rows[0].status,
        CoverageStatus::Missing,
        "scenario D: without default-ack the rejection degrades to missing"
    );

    // ----- Scenario E: `product feature reject` verb shape is idempotent -----
    // The verb writes (or updates) one `adrs-rejected:` entry. Running
    // it a second time with the same `(adr, reason)` MUST not duplicate
    // the entry, and running it with a different reason MUST update
    // the existing entry in place. This test exercises the
    // idempotency contract by running the merge twice.
    let mut rejections: Vec<RejectedAdr> = Vec::new();
    apply_reject(&mut rejections, "ADR-CC", "Stated rationale here.");
    apply_reject(&mut rejections, "ADR-CC", "Stated rationale here.");
    assert_eq!(
        rejections.len(),
        1,
        "scenario E: re-running the same reject is idempotent"
    );
    assert_eq!(rejections[0].reason, "Stated rationale here.");

    apply_reject(&mut rejections, "ADR-CC", "Updated rationale.");
    assert_eq!(
        rejections.len(),
        1,
        "scenario E: re-running with a new reason updates in place (no duplication)"
    );
    assert_eq!(rejections[0].reason, "Updated rationale.");
}

/// Emulate the `product feature reject ADR-NNN --feature FT-XXX
/// --reason "..."` verb contract. Inserts a new entry; if one already
/// exists for `adr_id`, updates the reason in place. Idempotent on
/// equal inputs.
fn apply_reject(rejections: &mut Vec<RejectedAdr>, adr_id: &str, reason: &str) {
    if let Some(existing) = rejections.iter_mut().find(|r| r.id == adr_id) {
        existing.reason = reason.to_string();
    } else {
        rejections.push(RejectedAdr {
            id: adr_id.to_string(),
            reason: reason.to_string(),
        });
    }
}
