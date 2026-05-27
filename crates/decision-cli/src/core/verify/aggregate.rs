//! Verdict-derivation rules for FT-097 / ADR-028.
//!
//! Two pure functions:
//!
//! 1. [`single_graph_verdict`] — derives the per-graph verdict from
//!    a step-trace pattern. Used by the runner (FT-098) and by SHACL
//!    consistency checks.
//! 2. [`aggregate_verdict`] — composes one or more
//!    `VerificationGraphResult`s for the same target into a single
//!    `AggregateVerdict`, surfacing uncovered TCs in `coverage_gaps`.
//!
//! Both functions are deterministic and pure — no graph access, no I/O.

use std::collections::BTreeSet;

use crate::core::ontology::verdict::Verdict;
use crate::core::ontology::verification_graph::VerificationStep;
use crate::core::ontology::verification_result::{StepOutcome, VerificationGraphResult};

/// Aggregation target: either a single TC or a feature rolling its TCs up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregationTarget {
    /// A single TC IRI.
    Tc(String),
    /// A feature IRI with its declared TC IRIs.
    Feature {
        /// Feature IRI.
        feature: String,
        /// TC IRIs the feature declares (`feature.tests`).
        tests: Vec<String>,
    },
}

impl AggregationTarget {
    /// Label suitable for embedding in rationale strings.
    fn label(&self) -> &str {
        match self {
            Self::Tc(iri) => iri.as_str(),
            Self::Feature { feature, .. } => feature.as_str(),
        }
    }
}

/// Aggregate verdict returned from [`aggregate_verdict`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateVerdict {
    /// Composite verdict per ADR-028.
    pub verdict: Verdict,
    /// One-line operator-facing rationale.
    pub rationale: String,
    /// VGR IRIs that drove the verdict.
    pub contributing_results: Vec<String>,
    /// TC IRIs that have no contributing result. Empty for `Tc` targets
    /// when the single TC is covered; populated for `Feature` targets
    /// when one or more declared TCs are uncovered.
    pub coverage_gaps: Vec<String>,
}

/// Exit codes that signal a graph-design fault rather than a code
/// regression, per FT-110.X. When a failing evidence-bearing step
/// returns one of these, the verdict aggregator demotes the verdict
/// from `Rejected` to `AmendmentRequired` so the FT-108 routing rule
/// sends the feedback to the verify-graph-author (graph re-author),
/// not the implementer (code-writer).
///
/// - `2`   — typical clap / argparse / CLI argument validation failure;
///           the graph called the binary with a flag set the binary
///           doesn't accept. The code under test wasn't even exercised.
/// - `126` — POSIX "found but not executable"; the graph names a
///           binary the env can't run.
/// - `127` — POSIX "command not found"; the graph references a binary
///           the env doesn't have.
///
/// Larger exit codes the verdict aggregator deliberately ignores (101
/// Rust panic, 1 generic-failure, 124 timeout) since those can be
/// legitimate code-side regressions worth routing to the implementer.
pub const GRAPH_FAULT_EXIT_CODES: &[i64] = &[2, 126, 127];

/// Back-compat wrapper around [`single_graph_verdict_with_exit_codes`]
/// — preserves the pre-FT-110.X two-argument shape for callers that
/// haven't been migrated yet. Equivalent to calling the new function
/// with an empty `exit_codes` slice, which means the FT-110.X
/// graph-fault demotion never fires.
#[must_use]
pub fn single_graph_verdict(
    traces: &[StepOutcome],
    provides_evidence_for: &[Vec<String>],
) -> (Verdict, String) {
    single_graph_verdict_with_exit_codes(traces, provides_evidence_for, &[])
}

/// Derive the per-graph verdict (FT-097 §Per-graph verdict derivation +
/// FT-110.X exit-code-aware demotion).
///
/// Inputs are aligned by index — `traces[i]` describes the run of
/// `steps[i]`. `exit_codes[i]` is the matching shell exit when the
/// step kind produces one (`None` for `file-assertion`,
/// `sparql-assertion`, etc.). Misalignment is a SHACL violation
/// enforced separately by the runner.
///
/// Returns `(verdict, rationale)`. The rationale is guaranteed to be
/// ≥ 20 chars per ADR-018 / FT-097 §Behaviour.
///
/// FT-110.X — when a failing evidence-bearing step's exit code is in
/// [`GRAPH_FAULT_EXIT_CODES`], the verdict is demoted from `Rejected`
/// to `AmendmentRequired`: the code wasn't really exercised because
/// the binary rejected the invocation up front, so feedback should
/// route to the graph author (verify-graph-author), not the
/// implementer.
#[must_use]
pub fn single_graph_verdict_with_exit_codes(
    traces: &[StepOutcome],
    provides_evidence_for: &[Vec<String>],
    exit_codes: &[Option<i64>],
) -> (Verdict, String) {
    if traces.is_empty() {
        return (
            Verdict::Approved,
            "vacuous pass: 0 steps in graph; no assertions to evaluate".to_string(),
        );
    }
    // First failing step (left-to-right) — fail dominates unrunnable.
    let mut first_fail: Option<usize> = None;
    let mut first_unrunnable: Option<usize> = None;
    for (i, outcome) in traces.iter().enumerate() {
        match outcome {
            StepOutcome::Fail if first_fail.is_none() => first_fail = Some(i),
            StepOutcome::Unrunnable if first_unrunnable.is_none() => first_unrunnable = Some(i),
            _ => {}
        }
    }
    if let Some(idx) = first_fail {
        // fail dominates: rejected iff that step has evidence linkage;
        // amendment-required iff it doesn't (setup/capture-style step).
        let empty = Vec::new();
        let linked = provides_evidence_for.get(idx).unwrap_or(&empty);
        if !linked.is_empty() {
            // FT-110.X — graph-fault exit codes demote to AmendmentRequired
            // even on evidence-bearing steps. The binary refused the
            // invocation before the code under test ran; the failure
            // is in the graph's command spec, not in the code.
            let exit = exit_codes.get(idx).copied().flatten();
            if let Some(code) = exit {
                if GRAPH_FAULT_EXIT_CODES.contains(&code) {
                    let tcs = linked.join(", ");
                    return (
                        Verdict::AmendmentRequired,
                        format!(
                            "step {idx} failed with exit {code} (binary refused the invocation \
                             before the code ran) on TCs: {tcs}; this is a graph-design fault, \
                             not a code regression — the graph needs editing",
                        ),
                    );
                }
            }
            let tcs = linked.join(", ");
            return (
                Verdict::Rejected,
                format!(
                    "step {idx} failed; evidence regression on TCs: {tcs}; the run is rejected",
                ),
            );
        }
        return (
            Verdict::AmendmentRequired,
            format!(
                "step {idx} failed before producing evidence (setup/capture-style failure); the graph needs editing, not the code",
            ),
        );
    }
    if let Some(idx) = first_unrunnable {
        return (
            Verdict::AmendmentRequired,
            format!(
                "step {idx} was unrunnable (timeout, missing target, or runtime safety violation); the run produced no decisive evidence",
            ),
        );
    }
    (
        Verdict::Approved,
        format!(
            "all {n} steps passed; the run produced approved evidence for every linked TC",
            n = traces.len()
        ),
    )
}

/// Compose one or more `VerificationGraphResult`s for `target` into a
/// single `AggregateVerdict` per ADR-028 §Multi-graph aggregation.
///
/// Pure: no graph access, no I/O. The caller decides what results to
/// feed in (cross-environment de-duplication is not the function's
/// concern per FT-097 §Boundaries).
#[must_use]
pub fn aggregate_verdict(
    target: AggregationTarget,
    results: &[VerificationGraphResult],
) -> AggregateVerdict {
    match &target {
        AggregationTarget::Tc(tc_iri) => aggregate_for_tc(tc_iri, results, &target),
        AggregationTarget::Feature { feature, tests } => {
            aggregate_for_feature(feature, tests, results, &target)
        }
    }
}

fn aggregate_for_tc(
    tc_iri: &str,
    results: &[VerificationGraphResult],
    target: &AggregationTarget,
) -> AggregateVerdict {
    let covering: Vec<&VerificationGraphResult> = results
        .iter()
        .filter(|r| result_covers_tc(r, tc_iri))
        .collect();
    if covering.is_empty() {
        return AggregateVerdict {
            verdict: Verdict::Rejected,
            rationale: format!("no verification graph result covers <{label}>", label = target.label()),
            contributing_results: Vec::new(),
            coverage_gaps: vec![tc_iri.to_string()],
        };
    }
    let any_rejected = covering.iter().any(|r| r.verdict == Verdict::Rejected);
    let any_amendment = covering
        .iter()
        .any(|r| r.verdict == Verdict::AmendmentRequired);
    let all_approved = covering.iter().all(|r| r.verdict == Verdict::Approved);
    if any_rejected {
        let drivers: Vec<String> = covering
            .iter()
            .filter(|r| r.verdict == Verdict::Rejected)
            .map(|r| r.id.clone())
            .collect();
        return AggregateVerdict {
            verdict: Verdict::Rejected,
            rationale: format!(
                "rejection dominates: {n} of {total} contributing results returned rejected for <{label}>",
                n = drivers.len(),
                total = covering.len(),
                label = target.label()
            ),
            contributing_results: drivers,
            coverage_gaps: Vec::new(),
        };
    }
    if all_approved {
        return AggregateVerdict {
            verdict: Verdict::Approved,
            rationale: format!(
                "all {n} contributing results approved <{label}>",
                n = covering.len(),
                label = target.label()
            ),
            contributing_results: covering.iter().map(|r| r.id.clone()).collect(),
            coverage_gaps: Vec::new(),
        };
    }
    // Mix of approved + amendment-required, no rejected.
    if any_amendment {
        return AggregateVerdict {
            verdict: Verdict::AmendmentRequired,
            rationale: format!(
                "mixed approved + amendment-required for <{label}>; no rejection but the run needs amendment",
                label = target.label()
            ),
            contributing_results: covering.iter().map(|r| r.id.clone()).collect(),
            coverage_gaps: Vec::new(),
        };
    }
    AggregateVerdict {
        verdict: Verdict::Approved,
        rationale: format!(
            "all {n} contributing results approved <{label}>",
            n = covering.len(),
            label = target.label()
        ),
        contributing_results: covering.iter().map(|r| r.id.clone()).collect(),
        coverage_gaps: Vec::new(),
    }
}

fn aggregate_for_feature(
    feature: &str,
    tests: &[String],
    results: &[VerificationGraphResult],
    target: &AggregationTarget,
) -> AggregateVerdict {
    if tests.is_empty() {
        return AggregateVerdict {
            verdict: Verdict::Approved,
            rationale: format!(
                "target <{feature}> has no TCs; vacuous pass for the feature roll-up"
            ),
            contributing_results: Vec::new(),
            coverage_gaps: Vec::new(),
        };
    }
    // FT-097 §Multi-graph aggregation: when the input set is empty, the
    // feature aggregate matches the per-target empty-set row — surface
    // "no verification graph result covers <feature>" rather than the
    // looser "rejection dominates" rationale, so the operator sees the
    // distinguishing cause at a glance.
    if results.is_empty() {
        return AggregateVerdict {
            verdict: Verdict::Rejected,
            rationale: format!(
                "no verification graph result covers <{label}>",
                label = target.label()
            ),
            contributing_results: Vec::new(),
            coverage_gaps: tests.to_vec(),
        };
    }
    // For each TC, compute the per-TC aggregate.
    let per_tc: Vec<(String, AggregateVerdict)> = tests
        .iter()
        .map(|tc| {
            (
                tc.clone(),
                aggregate_for_tc(tc, results, &AggregationTarget::Tc(tc.clone())),
            )
        })
        .collect();

    // coverage_gaps = TCs whose aggregate marked them as uncovered.
    let coverage_gaps: Vec<String> = per_tc
        .iter()
        .filter(|(_, agg)| !agg.coverage_gaps.is_empty())
        .map(|(tc, _)| tc.clone())
        .collect();

    // contributing_results: union of all per-TC contributors, but for
    // the rejected case we surface only the rejected drivers (matching
    // the per-TC rule). Preserve input order.
    let any_rejected = per_tc
        .iter()
        .any(|(_, agg)| agg.verdict == Verdict::Rejected);
    let any_amendment = per_tc
        .iter()
        .any(|(_, agg)| agg.verdict == Verdict::AmendmentRequired);

    if any_rejected {
        let mut drivers: Vec<String> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (_, agg) in &per_tc {
            if agg.verdict == Verdict::Rejected {
                for vgr in &agg.contributing_results {
                    if seen.insert(vgr.clone()) {
                        drivers.push(vgr.clone());
                    }
                }
            }
        }
        let rationale = if !coverage_gaps.is_empty() {
            format!(
                "rejection dominates for <{label}>: {n_gaps} uncovered TCs + {n_drivers} rejecting results",
                label = target.label(),
                n_gaps = coverage_gaps.len(),
                n_drivers = drivers.len()
            )
        } else {
            format!(
                "rejection dominates for <{label}>: {n} rejecting results",
                label = target.label(),
                n = drivers.len()
            )
        };
        return AggregateVerdict {
            verdict: Verdict::Rejected,
            rationale,
            contributing_results: drivers,
            coverage_gaps,
        };
    }

    if any_amendment {
        let mut drivers: Vec<String> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (_, agg) in &per_tc {
            for vgr in &agg.contributing_results {
                if seen.insert(vgr.clone()) {
                    drivers.push(vgr.clone());
                }
            }
        }
        return AggregateVerdict {
            verdict: Verdict::AmendmentRequired,
            rationale: format!(
                "amendment-required for <{label}>: at least one TC's verdict is amendment-required, no TC rejected",
                label = target.label()
            ),
            contributing_results: drivers,
            coverage_gaps,
        };
    }

    // All TCs approved (and none have coverage gaps).
    let mut drivers: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (_, agg) in &per_tc {
        for vgr in &agg.contributing_results {
            if seen.insert(vgr.clone()) {
                drivers.push(vgr.clone());
            }
        }
    }
    AggregateVerdict {
        verdict: Verdict::Approved,
        rationale: format!(
            "all {n} TCs of <{label}> approved by their covering results",
            n = tests.len(),
            label = target.label()
        ),
        contributing_results: drivers,
        coverage_gaps,
    }
}

fn result_covers_tc(result: &VerificationGraphResult, tc_iri: &str) -> bool {
    result.evidence_for.iter().any(|ev| ev.tc == tc_iri)
}

/// Convenience wrapper that resolves the single-graph verdict from a
/// `VerificationGraphResult`'s step traces. Useful for tests and the
/// SHACL consistency check.
///
/// FT-110.X: reads `exit_code` off each persisted step trace and feeds
/// it through so a re-verdict from disk matches the live verdict the
/// runner computed at execution time.
#[must_use]
pub fn verdict_from_result(
    result: &VerificationGraphResult,
    steps: &[VerificationStep],
) -> (Verdict, String) {
    let outcomes: Vec<StepOutcome> = result
        .step_traces
        .iter()
        .map(|t| t.outcome)
        .collect();
    let exit_codes: Vec<Option<i64>> = result
        .step_traces
        .iter()
        .map(|t| t.exit_code)
        .collect();
    let evidence: Vec<Vec<String>> = steps
        .iter()
        .map(|s| {
            s.provides_evidence_for
                .iter()
                .map(|n| n.as_str().to_string())
                .collect()
        })
        .collect();
    single_graph_verdict_with_exit_codes(&outcomes, &evidence, &exit_codes)
}

