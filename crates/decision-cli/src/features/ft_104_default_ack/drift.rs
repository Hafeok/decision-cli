//! `product graph check` drift validators for the FT-104 surface.
//!
//! Three drift conditions surface as warnings (never errors — drift is
//! informational per the FT-104 invariants):
//!
//! - **W035** — an entry in `default-acknowledged-cross-cutting`
//!   references an ADR that no longer exists in the catalog.
//! - **W036** — an entry in `default-acknowledged-cross-cutting`
//!   references an ADR whose `scope:` has changed away from
//!   `cross-cutting` (e.g. demoted to `feature-specific` or `platform`).
//! - **W037** — a feature's `adrs-rejected:` entry references an ADR
//!   that is not in `default-acknowledged-cross-cutting`. Rejecting an
//!   ADR that isn't auto-acknowledged is incoherent.
//!
//! Output is sorted by `(code, target_id)` so snapshot tests have a
//! stable shape.

use std::collections::BTreeSet;

use super::config::DefaultAcknowledgeConfig;
use super::frontmatter::RejectedAdr;

/// One drift warning. The shape mirrors product-cli's `Diagnostic`
/// type but stays local to this slice to avoid pulling product-cli's
/// internals into decision-cli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftWarning {
    /// Warning code (e.g. `"W035"`).
    pub code: String,
    /// Short, single-line message that names the offending entry.
    pub message: String,
    /// Operator hint suggesting the local fix. Always non-empty.
    pub hint: String,
}

/// A feature's rejection set as the drift checker sees it: feature id
/// plus the parsed `adrs-rejected:` rows.
#[derive(Debug, Clone)]
pub struct FeatureRejectionRecord {
    /// Feature id, e.g. `"FT-OPTOUT"`.
    pub feature_id: String,
    /// The feature's parsed `adrs-rejected:` entries.
    pub rejections: Vec<RejectedAdr>,
}

/// Catalog view: every accepted ADR plus whether its scope is
/// `cross-cutting`. The drift checker takes this rather than a raw
/// graph handle so it stays composable with both decision-cli's
/// projection reader and product-cli's `KnowledgeGraph`.
#[derive(Debug, Clone)]
pub struct AdrSnapshot {
    /// ADR id, e.g. `"ADR-005"`.
    pub adr_id: String,
    /// `true` iff the ADR's `scope:` is currently `cross-cutting`.
    pub is_cross_cutting: bool,
}

/// Evaluate every FT-104 drift condition and return the warnings in
/// stable, snapshot-friendly order.
#[must_use]
pub fn check_drift(
    config: &DefaultAcknowledgeConfig,
    adr_catalog: &[AdrSnapshot],
    feature_rejections: &[FeatureRejectionRecord],
) -> Vec<DriftWarning> {
    let mut warnings = Vec::new();

    let existing_ids: BTreeSet<&str> = adr_catalog.iter().map(|a| a.adr_id.as_str()).collect();
    let cross_cutting_ids: BTreeSet<&str> = adr_catalog
        .iter()
        .filter(|a| a.is_cross_cutting)
        .map(|a| a.adr_id.as_str())
        .collect();

    // W035 / W036: walk the config entries, fire one or the other (but
    // not both) per entry.
    for adr_id in &config.adrs {
        if !existing_ids.contains(adr_id.as_str()) {
            warnings.push(DriftWarning {
                code: "W035".into(),
                message: format!(
                    "default-acknowledged-cross-cutting references {adr_id}, but no such ADR \
                     exists in the catalog"
                ),
                hint: format!(
                    "Either remove {adr_id} from `[features] default-acknowledged-cross-cutting` \
                     in product.toml, or restore the ADR file."
                ),
            });
            continue;
        }
        if !cross_cutting_ids.contains(adr_id.as_str()) {
            warnings.push(DriftWarning {
                code: "W036".into(),
                message: format!(
                    "default-acknowledged-cross-cutting references {adr_id}, whose scope is no \
                     longer cross-cutting"
                ),
                hint: format!(
                    "Remove {adr_id} from `[features] default-acknowledged-cross-cutting` — \
                     default-acknowledging a non-cross-cutting ADR has no effect."
                ),
            });
        }
    }

    // W037: feature rejection points at an ADR not in default-ack list.
    for feat in feature_rejections {
        for rej in &feat.rejections {
            if !config.acknowledges(&rej.id) {
                warnings.push(DriftWarning {
                    code: "W037".into(),
                    message: format!(
                        "{}: rejecting an ADR ({}) that is not default-acknowledged has no \
                         effect",
                        feat.feature_id, rej.id
                    ),
                    hint: format!(
                        "Either add {} to `[features] default-acknowledged-cross-cutting` or \
                         remove the rejection from {}.",
                        rej.id, feat.feature_id
                    ),
                });
            }
        }
    }

    warnings.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.message.cmp(&b.message)));
    warnings
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

    fn snap(id: &str, xcut: bool) -> AdrSnapshot {
        AdrSnapshot {
            adr_id: id.into(),
            is_cross_cutting: xcut,
        }
    }

    #[test]
    fn no_drift_no_warnings() {
        let warnings = check_drift(&cfg(&["ADR-001"]), &[snap("ADR-001", true)], &[]);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn missing_adr_fires_w035() {
        let warnings = check_drift(
            &cfg(&["ADR-001", "ADR-GONE"]),
            &[snap("ADR-001", true)],
            &[],
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "W035");
        assert!(warnings[0].message.contains("ADR-GONE"));
        assert!(warnings[0].hint.contains("ADR-GONE"));
    }

    #[test]
    fn rescoped_adr_fires_w036() {
        let warnings = check_drift(&cfg(&["ADR-RESCOPED"]), &[snap("ADR-RESCOPED", false)], &[]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "W036");
        assert!(warnings[0].message.contains("ADR-RESCOPED"));
        assert!(warnings[0].message.contains("no longer cross-cutting"));
    }

    #[test]
    fn stray_rejection_fires_w037() {
        let rec = FeatureRejectionRecord {
            feature_id: "FT-OPTOUT".into(),
            rejections: vec![RejectedAdr {
                id: "ADR-STRAY".into(),
                reason: "has no effect because not default-acked".into(),
            }],
        };
        let warnings = check_drift(
            &cfg(&["ADR-001"]),
            &[snap("ADR-001", true), snap("ADR-STRAY", true)],
            &[rec],
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].code, "W037");
        assert!(warnings[0].message.contains("FT-OPTOUT"));
        assert!(warnings[0].message.contains("ADR-STRAY"));
    }

    #[test]
    fn valid_rejection_does_not_fire_w037() {
        let rec = FeatureRejectionRecord {
            feature_id: "FT-OPTOUT".into(),
            rejections: vec![RejectedAdr {
                id: "ADR-ALIVE".into(),
                reason: "valid rejection".into(),
            }],
        };
        let warnings = check_drift(&cfg(&["ADR-ALIVE"]), &[snap("ADR-ALIVE", true)], &[rec]);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn three_drift_conditions_coexist_in_stable_order() {
        let rec = FeatureRejectionRecord {
            feature_id: "FT-OPTOUT".into(),
            rejections: vec![
                RejectedAdr {
                    id: "ADR-ALIVE".into(),
                    reason: "valid".into(),
                },
                RejectedAdr {
                    id: "ADR-STRAY".into(),
                    reason: "incoherent".into(),
                },
            ],
        };
        let warnings = check_drift(
            &cfg(&["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]),
            &[
                snap("ADR-ALIVE", true),
                snap("ADR-RESCOPED", false),
                snap("ADR-STRAY", true),
            ],
            &[rec],
        );
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        // sorted by (code, message)
        assert_eq!(warnings[0].code, "W035");
        assert_eq!(warnings[1].code, "W036");
        assert_eq!(warnings[2].code, "W037");
    }

    #[test]
    fn fixing_each_clears_it_independently() {
        // Start with all three.
        let mut config = cfg(&["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]);
        let mut catalog = vec![
            snap("ADR-ALIVE", true),
            snap("ADR-RESCOPED", false),
            snap("ADR-STRAY", true),
        ];
        let mut rec = FeatureRejectionRecord {
            feature_id: "FT-OPTOUT".into(),
            rejections: vec![
                RejectedAdr {
                    id: "ADR-ALIVE".into(),
                    reason: "valid".into(),
                },
                RejectedAdr {
                    id: "ADR-STRAY".into(),
                    reason: "incoherent".into(),
                },
            ],
        };
        assert_eq!(check_drift(&config, &catalog, &[rec.clone()]).len(), 3);

        // 1) Remove ADR-GONE → two remain.
        config.adrs.remove("ADR-GONE");
        assert_eq!(check_drift(&config, &catalog, &[rec.clone()]).len(), 2);

        // 2) Re-scope ADR-RESCOPED to cross-cutting → one remains.
        if let Some(entry) = catalog.iter_mut().find(|s| s.adr_id == "ADR-RESCOPED") {
            entry.is_cross_cutting = true;
        }
        assert_eq!(check_drift(&config, &catalog, &[rec.clone()]).len(), 1);

        // 3) Remove the stray rejection → zero.
        rec.rejections.retain(|r| r.id != "ADR-STRAY");
        assert!(check_drift(&config, &catalog, &[rec]).is_empty());
    }
}
