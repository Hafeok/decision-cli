//! Preflight algorithm extension (the substantive edit of FT-104).
//!
//! Before this slice, the preflight loop classified each cross-cutting
//! ADR as either *linked* (the feature lists it under `adrs:` or shares
//! a domain with it) or a *gap*. FT-104 introduces two new outcomes:
//!
//! - **default-acknowledged** — the ADR is not linked but appears in
//!   `[features] default-acknowledged-cross-cutting` and the feature
//!   does not reject it. Not a gap; rendered with a clarifying tag.
//! - **intentional** — the feature explicitly opts out via
//!   `adrs-rejected:` with a rationale string. Treated as a gap, but
//!   carries `severity = intentional` and the reason so dashboards can
//!   distinguish "forgot to acknowledge" from "deliberately rejects".
//!
//! Precedence (high to low):
//!   1. explicit link in `adrs:` → linked
//!   2. explicit rejection in `adrs-rejected:` AND ADR is default-acked
//!      → intentional (with reason)
//!   3. ADR in default-acked list → default-acknowledged
//!   4. otherwise → missing

use super::config::DefaultAcknowledgeConfig;
use super::frontmatter::RejectedAdr;

/// Coverage status for one cross-cutting ADR against one feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageStatus {
    /// The feature explicitly lists the ADR in its `adrs:` frontmatter
    /// (or shares a domain with it). No gap.
    Linked,
    /// The ADR is config-level default-acknowledged and the feature has
    /// not rejected it. No gap; the renderer surfaces a tag so the
    /// operator can see *why* a previously-flagged ADR is now clean.
    DefaultAcknowledged,
    /// The feature explicitly rejected a default-acknowledged ADR via
    /// `adrs-rejected:`. Counts as a gap with audit metadata.
    Intentional {
        /// Operator-supplied rationale, carried through to the report.
        reason: String,
    },
    /// Pre-FT-104 gap shape. Neither linked, nor default-ack, nor
    /// rejected — the feature simply didn't acknowledge the concern.
    Missing,
}

impl CoverageStatus {
    /// `true` when the status is rendered as a gap in preflight. Both
    /// `Missing` and `Intentional` count; `Linked` and
    /// `DefaultAcknowledged` do not.
    #[must_use]
    pub fn is_gap(&self) -> bool {
        matches!(self, Self::Missing | Self::Intentional { .. })
    }

    /// Short severity label used by JSON/MCP renderers and by snapshot
    /// tests. Matches the controlled vocabulary in TC-174 / FT-104.
    #[must_use]
    pub fn severity_label(&self) -> &'static str {
        match self {
            Self::Linked => "linked",
            Self::DefaultAcknowledged => "default-acknowledged",
            Self::Intentional { .. } => "intentional",
            Self::Missing => "missing",
        }
    }
}

/// One row of the cross-cutting coverage table: the ADR, its computed
/// status, and any rejection reason carried through to the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossCuttingRow {
    /// ADR id, e.g. `"ADR-013"`.
    pub adr_id: String,
    /// Computed status (see [`CoverageStatus`]).
    pub status: CoverageStatus,
}

impl CrossCuttingRow {
    /// Convenience constructor for the linked row.
    #[must_use]
    pub fn linked(adr_id: impl Into<String>) -> Self {
        Self {
            adr_id: adr_id.into(),
            status: CoverageStatus::Linked,
        }
    }

    /// Convenience constructor for the missing row.
    #[must_use]
    pub fn missing(adr_id: impl Into<String>) -> Self {
        Self {
            adr_id: adr_id.into(),
            status: CoverageStatus::Missing,
        }
    }
}

/// Run the FT-104 preflight algorithm for a single feature.
///
/// Inputs:
/// - `cross_cutting_adrs`: every ADR whose `scope: cross-cutting`
///   applies to the repo, in deterministic order.
/// - `feature_linked_adrs`: the IDs the feature's `adrs:` frontmatter
///   lists (covers the per-feature explicit-link case).
/// - `feature_rejections`: the feature's `adrs-rejected:` entries.
/// - `config`: parsed `[features] default-acknowledged-cross-cutting`.
///
/// Returns one row per input ADR, ordered the same way the input was
/// supplied. The caller may filter / sort / render as needed.
#[must_use]
pub fn evaluate_cross_cutting(
    cross_cutting_adrs: &[String],
    feature_linked_adrs: &[String],
    feature_rejections: &[RejectedAdr],
    config: &DefaultAcknowledgeConfig,
) -> Vec<CrossCuttingRow> {
    let linked: std::collections::HashSet<&str> =
        feature_linked_adrs.iter().map(String::as_str).collect();
    cross_cutting_adrs
        .iter()
        .map(|adr_id| {
            if linked.contains(adr_id.as_str()) {
                return CrossCuttingRow::linked(adr_id.clone());
            }
            let rejection = feature_rejections.iter().find(|r| &r.id == adr_id);
            let default_acked = config.acknowledges(adr_id);
            let status = match (rejection, default_acked) {
                (Some(r), true) => CoverageStatus::Intentional {
                    reason: r.reason.clone(),
                },
                (_, true) => CoverageStatus::DefaultAcknowledged,
                // Rejection without default-ack → ignored (drift warning
                // is the visibility surface; preflight treats the row as
                // a regular missing-link gap per the FT-104 spec).
                (Some(_), false) | (None, false) => CoverageStatus::Missing,
            };
            CrossCuttingRow {
                adr_id: adr_id.clone(),
                status,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(adrs: &[&str]) -> DefaultAcknowledgeConfig {
        DefaultAcknowledgeConfig {
            adrs: adrs.iter().map(|s| (*s).to_string()).collect(),
            source: None,
        }
    }

    #[test]
    fn linked_takes_precedence_over_default_ack() {
        let rows = evaluate_cross_cutting(
            &["ADR-001".into()],
            &["ADR-001".into()],
            &[],
            &cfg(&["ADR-001"]),
        );
        assert_eq!(rows[0].status, CoverageStatus::Linked);
    }

    #[test]
    fn default_ack_clears_the_gap_when_not_rejected() {
        let rows = evaluate_cross_cutting(&["ADR-001".into()], &[], &[], &cfg(&["ADR-001"]));
        assert_eq!(rows[0].status, CoverageStatus::DefaultAcknowledged);
        assert!(!rows[0].status.is_gap());
    }

    #[test]
    fn rejection_of_default_ack_yields_intentional_with_reason() {
        let rejections = vec![RejectedAdr {
            id: "ADR-001".into(),
            reason: "feature uses alt pattern".into(),
        }];
        let rows =
            evaluate_cross_cutting(&["ADR-001".into()], &[], &rejections, &cfg(&["ADR-001"]));
        assert!(matches!(
            &rows[0].status,
            CoverageStatus::Intentional { reason } if reason == "feature uses alt pattern"
        ));
        assert!(rows[0].status.is_gap());
        assert_eq!(rows[0].status.severity_label(), "intentional");
    }

    #[test]
    fn rejection_of_non_default_ack_falls_back_to_missing() {
        let rejections = vec![RejectedAdr {
            id: "ADR-STRAY".into(),
            reason: "has no effect because not default-acked".into(),
        }];
        let rows = evaluate_cross_cutting(&["ADR-STRAY".into()], &[], &rejections, &cfg(&[]));
        assert_eq!(rows[0].status, CoverageStatus::Missing);
    }

    #[test]
    fn neither_linked_nor_acked_is_a_missing_gap() {
        let rows = evaluate_cross_cutting(&["ADR-001".into()], &[], &[], &cfg(&[]));
        assert_eq!(rows[0].status, CoverageStatus::Missing);
        assert!(rows[0].status.is_gap());
    }

    #[test]
    fn empty_config_preserves_pre_ft_104_behaviour() {
        // The Default config has no ADRs; identical to the old shape
        // where every cross-cutting ADR is a gap unless linked.
        let rows = evaluate_cross_cutting(
            &["ADR-001".into(), "ADR-002".into()],
            &["ADR-002".into()],
            &[],
            &DefaultAcknowledgeConfig::default(),
        );
        assert_eq!(rows[0].status, CoverageStatus::Missing);
        assert_eq!(rows[1].status, CoverageStatus::Linked);
    }
}
