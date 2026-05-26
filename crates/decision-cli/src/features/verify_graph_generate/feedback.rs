//! ADR-066 §Rule 3 — gap-feedback emission for validator rejections.
//!
//! When the chokepoint validator ([`super::validator`]) rejects a
//! proposal, this module emits one or more `dec:Feedback` artifacts
//! (`class = "gap"`) targeted at the upstream catalog category that
//! was the natural home for the missing fact:
//!
//! - `Binary` / `FilePath` / `HttpHost` / `CaptureSource` →
//!   `dec:VerificationEnvironment`.
//! - `DecSubcommand` → `dec:CapabilityReference` category.
//! - `SparqlNamespace` → `dec:OntologyDescription` category.
//!
//! Per ADR-066 the gap belongs to the *upstream* artifact (the catalog),
//! not the worker. The emission path writes feedback artifacts via the
//! [`StreamWriter`] chokepoint so SHACL validation runs end-to-end.
//!
//! Tests can record an in-memory log of what would be emitted instead of
//! writing to the orchestration store; the path is gated by
//! [`with_capture`] and is the only way the test-suite exercises this
//! module without persisting feedback.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::validator::{UpstreamTarget, Violation};

/// One feedback record the validator's reject path would emit. The
/// orchestrator persists these via [`StreamWriter`]; tests can intercept
/// them through [`with_capture`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GapFeedbackRecord {
    /// `dec:class` literal value ("gap"). Constant; carried for clarity.
    pub class: String,
    /// Stable target tag ("capability-reference", "ontology-description",
    /// "verification-environment").
    pub target: String,
    /// Default target role for routing per ADR-026.
    pub target_role: String,
    /// Joined violation detail (one line per violation).
    pub evidence: String,
    /// Optional remediation suggestion.
    pub recommendation: String,
    /// Violations included in this aggregate.
    pub violations: Vec<Violation>,
}

thread_local! {
    /// Thread-local capture buffer used by tests. When non-`None`, the
    /// emitter pushes to this vec instead of persisting to a store.
    static CAPTURE: RefCell<Option<Vec<GapFeedbackRecord>>> = const { RefCell::new(None) };
}

/// RAII guard that activates the in-memory capture buffer for the
/// duration of its lifetime. Drop to deactivate.
pub struct CaptureGuard(());

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        CAPTURE.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

/// Activate the test-only capture buffer for the current thread.
#[must_use]
pub fn with_capture() -> CaptureGuard {
    CAPTURE.with(|cell| {
        *cell.borrow_mut() = Some(Vec::new());
    });
    CaptureGuard(())
}

/// Drain the captured records. Returns an empty vec when no capture
/// guard is active.
#[must_use]
pub fn drain_captured() -> Vec<GapFeedbackRecord> {
    CAPTURE.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    })
}

/// Aggregate violations by upstream target category and emit one
/// `dec:Feedback` artifact per category — per ADR-066 §Rule 3 the
/// operator wants one actionable item per catalog edit, not one per
/// violation.
///
/// When the capture buffer is active, records go there. Otherwise they
/// are returned for the caller to persist via [`StreamWriter`].
pub fn emit_gap_feedback(violations: &[Violation]) -> Vec<GapFeedbackRecord> {
    if violations.is_empty() {
        return Vec::new();
    }
    let groups = group_by_target(violations);
    let records: Vec<GapFeedbackRecord> = groups
        .into_iter()
        .map(|(target, vs)| build_record(target, vs))
        .collect();
    CAPTURE.with(|cell| {
        if let Some(buf) = cell.borrow_mut().as_mut() {
            buf.extend(records.clone());
        }
    });
    records
}

fn group_by_target(violations: &[Violation]) -> Vec<(UpstreamTarget, Vec<Violation>)> {
    let mut grouped: BTreeMap<&'static str, (UpstreamTarget, Vec<Violation>)> = BTreeMap::new();
    for v in violations {
        let target = v.kind.upstream_target();
        let key = target.as_str();
        grouped
            .entry(key)
            .or_insert_with(|| (target, Vec::new()))
            .1
            .push(v.clone());
    }
    grouped.into_values().collect()
}

fn build_record(target: UpstreamTarget, vs: Vec<Violation>) -> GapFeedbackRecord {
    let evidence = vs
        .iter()
        .map(|v| {
            format!(
                "step {idx}: {k} {thing} — {why}",
                idx = v.step_index,
                k = format!("{:?}", v.kind).to_lowercase(),
                thing = v.referenced_thing,
                why = v.why_rejected,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    GapFeedbackRecord {
        class: "gap".to_string(),
        target: target.as_str().to_string(),
        target_role: target_role_for(target).to_string(),
        evidence,
        recommendation: recommendation_for(target),
        violations: vs,
    }
}

/// Default routing role per ADR-026 §Routing table. Gap feedback against
/// any catalog category routes to `bundle-assembler`.
fn target_role_for(_target: UpstreamTarget) -> &'static str {
    "bundle-assembler"
}

fn recommendation_for(target: UpstreamTarget) -> String {
    match target {
        UpstreamTarget::CapabilityReference => {
            "Author a CapabilityReference for the missing dec subcommand \
             (`dec catalog capability new CR-NNN --command '<verb>' --version <semver>`), \
             then re-run `dec verify graph generate`."
                .to_string()
        }
        UpstreamTarget::OntologyDescription => {
            "Supersede the active OntologyDescription with one declaring \
             the missing namespace, then re-run `dec verify graph generate`."
                .to_string()
        }
        UpstreamTarget::VerificationEnvironment => {
            "Extend the target env's dec:concreteCapabilities block to \
             include the referenced binary / path / host / variable, then \
             re-run `dec verify graph generate`."
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::verify_graph_generate::validator::ViolationKind;

    fn v(step: usize, kind: ViolationKind, thing: &str) -> Violation {
        Violation {
            step_index: step,
            kind,
            referenced_thing: thing.to_string(),
            why_rejected: "test".to_string(),
        }
    }

    #[test]
    fn one_record_per_upstream_target_category() {
        let violations = vec![
            v(0, ViolationKind::DecSubcommand, "dec verify result inspect"),
            v(1, ViolationKind::SparqlNamespace, "https://fake.example/ns#"),
            v(2, ViolationKind::FilePath, "/etc/passwd"),
            v(3, ViolationKind::DecSubcommand, "dec foo bar"),
        ];
        let records = emit_gap_feedback(&violations);
        assert_eq!(records.len(), 3, "one per category");
        // Class is always gap.
        for r in &records {
            assert_eq!(r.class, "gap");
            assert_eq!(r.target_role, "bundle-assembler");
        }
    }

    #[test]
    fn empty_violations_emit_nothing() {
        let records = emit_gap_feedback(&[]);
        assert!(records.is_empty());
    }

    #[test]
    fn capture_buffer_records_emitted_records() {
        let _g = with_capture();
        let _ = emit_gap_feedback(&[v(0, ViolationKind::DecSubcommand, "dec x")]);
        let drained = drain_captured();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].class, "gap");
    }
}
