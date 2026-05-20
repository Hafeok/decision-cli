//! Programmatic API for the action-interpretation agreement metric (FT-024).
//!
//! Read-only. Counts in Rust off a single SPARQL pass over the
//! orchestration store; rates are derived from the counts so SPARQL's
//! `xsd:double` division surprises never appear in the report.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::dispatch::DispatchStatus;
use crate::core::role_catalog;
use crate::core::sparql::{term_iri_string, term_literal_string};
use crate::core::vocab::{
    VERDICT_AMENDMENT_REQUIRED, VERDICT_APPROVED, VERDICT_REJECTED,
};

use super::queries::build_query;

/// The action-interpretation agreement report defined in ADR-021.
#[derive(Debug, Clone, PartialEq)]
pub struct AgreementReport {
    /// Count of distinct `dec:DispatchGroup` artifacts in any terminal
    /// `dec:dispatchStatus` that match the window + role filter.
    pub total_terminal_groups: u64,
    /// Subset of terminal groups whose action session terminated
    /// successfully (i.e. group status ≠ `action-failed`).
    pub total_action_success: u64,
    /// Verdict count: `approved`.
    pub approved: u64,
    /// Verdict count: `amendment-required`.
    pub amendment_required: u64,
    /// Verdict count: `rejected`.
    pub rejected: u64,
    /// `|A ∩ approved| / |A|` per ADR-021. `0.0` when `|A| = 0`.
    pub agreement_rate: f64,
    /// `|amendment-required| / |A|` per ADR-021. `0.0` when `|A| = 0`.
    pub amendment_rate: f64,
    /// `|rejected| / |A|` per ADR-021. `0.0` when `|A| = 0`.
    pub rejection_rate: f64,
    /// `|A ∩ (rejected ∪ amendment-required)| / |A|` per ADR-021. The
    /// load-bearing one. `0.0` when `|A| = 0`.
    pub false_success_rate: f64,
    /// Window the report was computed over (echoed from the request).
    pub window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Role filter the report was computed over (echoed from the request).
    pub role_filter: Option<String>,
}

/// Errors surfaced by [`agreement`].
#[derive(Debug, Error)]
pub enum MetricsError {
    /// SPARQL execution failure (e.g. malformed dataset, oxigraph IO).
    #[error("SPARQL execution failure: {detail}")]
    Sparql {
        /// Underlying error detail.
        detail: String,
    },

    /// The supplied window has `until < since`.
    #[error("invalid window: start ({since}) > end ({until})")]
    InvalidWindow {
        /// Start of the requested window.
        since: DateTime<Utc>,
        /// End of the requested window.
        until: DateTime<Utc>,
    },

    /// The supplied role filter does not appear in the role catalog
    /// (FT-019 + slice-1 hardcoded implementer surface).
    #[error("unknown role filter: {id}")]
    UnknownRole {
        /// The unknown role id.
        id: String,
    },
}

/// Compute the action-interpretation agreement report for `store`.
///
/// Optional `window` selects only DispatchGroups whose action session
/// `prov:atTime` falls within `[since, until]`. Optional `role_filter`
/// selects only groups whose action session has `dec:role` equal to
/// the supplied id.
///
/// Returns an empty report (`total_terminal_groups = 0`, all rates
/// `0.0`) when no qualifying group exists — the empty-data marker is
/// the zero-count field, so downstream callers can distinguish "no
/// data in window" from "data with zero rate."
///
/// # Errors
/// - [`MetricsError::InvalidWindow`] when `since > until`.
/// - [`MetricsError::UnknownRole`] when `role_filter` is not in the
///   role catalog (the implementer role surface is implicit; the
///   verifier role is graph-resident via FT-019).
/// - [`MetricsError::Sparql`] when the underlying SPARQL execution
///   fails.
pub fn agreement(
    store: &Store,
    window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    role_filter: Option<&str>,
) -> Result<AgreementReport, MetricsError> {
    if let Some((since, until)) = window {
        if since > until {
            return Err(MetricsError::InvalidWindow { since, until });
        }
    }
    if let Some(id) = role_filter {
        validate_role_filter(store, id)?;
    }
    let window_strs = window.map(|(since, until)| (since.to_rfc3339(), until.to_rfc3339()));
    let q = build_query(
        window_strs.as_ref().map(|(s, u)| (s.as_str(), u.as_str())),
        role_filter,
    );
    let counts = execute_and_count(store, &q)?;
    Ok(counts.into_report(window, role_filter.map(str::to_string)))
}

fn validate_role_filter(store: &Store, id: &str) -> Result<(), MetricsError> {
    // The implementer role is the slice-1 hardcoded surface (FT-011);
    // its catalog entry lands in slice 3 (FT-030). Accept it implicitly
    // so slice-2 `dec metrics agreement --role implementer` works
    // before FT-030 lands.
    if id == "implementer" {
        return Ok(());
    }
    let lookup = role_catalog::lookup(store, id)
        .map_err(|e| MetricsError::Sparql { detail: e.to_string() })?;
    if lookup.is_none() {
        return Err(MetricsError::UnknownRole { id: id.to_string() });
    }
    Ok(())
}

#[derive(Default, Debug)]
struct Counts {
    total_terminal_groups: u64,
    total_action_success: u64,
    approved: u64,
    amendment_required: u64,
    rejected: u64,
}

impl Counts {
    fn into_report(
        self,
        window: Option<(DateTime<Utc>, DateTime<Utc>)>,
        role_filter: Option<String>,
    ) -> AgreementReport {
        let action = self.total_action_success;
        let approved_in_a = self.approved;
        let agreement_rate = safe_ratio(approved_in_a, action);
        let amendment_rate = safe_ratio(self.amendment_required, action);
        let rejection_rate = safe_ratio(self.rejected, action);
        let false_success_rate =
            safe_ratio(self.rejected + self.amendment_required, action);
        AgreementReport {
            total_terminal_groups: self.total_terminal_groups,
            total_action_success: action,
            approved: self.approved,
            amendment_required: self.amendment_required,
            rejected: self.rejected,
            agreement_rate,
            amendment_rate,
            rejection_rate,
            false_success_rate,
            window,
            role_filter,
        }
    }
}

fn safe_ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        // Cast is bounded: counts are u64 but in practice well below
        // 2^53 — the f64 cast is exact at slice-2 scale.
        (numerator as f64) / (denominator as f64)
    }
}

fn execute_and_count(store: &Store, q: &str) -> Result<Counts, MetricsError> {
    let results = store
        .query(q)
        .map_err(|e| MetricsError::Sparql { detail: e.to_string() })?;
    let QueryResults::Solutions(sols) = results else {
        return Err(MetricsError::Sparql {
            detail: "agreement query returned unexpected result shape".to_string(),
        });
    };
    let mut counts = Counts::default();
    let mut seen: HashSet<String> = HashSet::new();
    for sol in sols {
        let sol = sol.map_err(|e| MetricsError::Sparql { detail: e.to_string() })?;
        let group = match sol.get("group") {
            Some(t) => term_iri_string(t),
            None => continue,
        };
        // A given DispatchGroup may surface multiple solution rows when
        // it has multiple verdicts (the verifier may emit more than
        // once across amendment retries). We count each group at most
        // once for the terminal totals; the verdict counters use the
        // first verdict observed so the totals don't drift past 100%.
        if !seen.insert(group) {
            continue;
        }
        let status = sol.get("status").map(term_literal_string).unwrap_or_default();
        let parsed = DispatchStatus::parse(&status);
        let Some(parsed) = parsed else { continue };
        if !parsed.is_terminal() {
            continue;
        }
        counts.total_terminal_groups += 1;
        if !matches!(parsed, DispatchStatus::ActionFailed) {
            counts.total_action_success += 1;
        }
        let verdict = sol.get("verdictValue").map(term_literal_string);
        match verdict.as_deref() {
            Some(VERDICT_APPROVED) => counts.approved += 1,
            Some(VERDICT_AMENDMENT_REQUIRED) => counts.amendment_required += 1,
            Some(VERDICT_REJECTED) => counts.rejected += 1,
            _ => {}
        }
    }
    Ok(counts)
}

/// Render an [`AgreementReport`] as a human-readable five-row table.
///
/// The format is the slice-2 CLI surface ([FT-025]); other consumers
/// are welcome to render their own.
///
/// [FT-025]: https://decision-cli.dev/features/FT-025
#[must_use]
pub fn format_report(report: &AgreementReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let window_line = report
        .window
        .map_or_else(
            || "Window: (all time)".to_string(),
            |(s, u)| format!("Window: {s} .. {u}"),
        );
    let role_line = report
        .role_filter
        .as_deref()
        .map_or_else(|| "Role:   (all roles)".to_string(), |r| format!("Role:   {r}"));
    let _ = writeln!(out, "{window_line}");
    let _ = writeln!(out, "{role_line}");
    let _ = writeln!(
        out,
        "Counts: terminal={t}, action_success={a}, approved={ap}, amendment={am}, rejected={r}",
        t = report.total_terminal_groups,
        a = report.total_action_success,
        ap = report.approved,
        am = report.amendment_required,
        r = report.rejected,
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "  Agreement rate     {:>7.2}%", report.agreement_rate * 100.0);
    let _ = writeln!(out, "  Amendment rate     {:>7.2}%", report.amendment_rate * 100.0);
    let _ = writeln!(out, "  Rejection rate     {:>7.2}%", report.rejection_rate * 100.0);
    let _ = writeln!(
        out,
        "  False-success rate {:>7.2}%",
        report.false_success_rate * 100.0
    );
    out
}
