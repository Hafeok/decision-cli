//! Front-matter-to-motivational mapping table (FT-074 §Behaviour step 2).
//!
//! Encodes the slice-1 informal-field-to-motivational-predicate mapping
//! the migration tool uses to decide whether a non-conformant artifact
//! can be backfilled (a clean mapping exists) or must be flagged as an
//! orphan (no mapping survives the new vocabulary).
//!
//! The mapping is intentionally narrow: ADR-042's "no false motivational
//! edges" invariant means we only synthesise edges that have a clear
//! informal-field source. When in doubt, classify as orphan rather than
//! backfill (FT-074 §Invariants).

#![allow(missing_docs)]

use crate::core::vocab::IRI_DEC_FEEDBACK;

/// One row of the slice-1 mapping table — describes one informal
/// front-matter field that maps to one motivational predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingRow {
    /// Source artifact type the field lives on (e.g. `dec:ADR`).
    pub source_type: &'static str,
    /// Front-matter field name (e.g. `features`, `validates.features`).
    pub source_field: &'static str,
    /// Motivational predicate IRI emitted on backfill.
    pub motivational_predicate: &'static str,
    /// Target artifact type the predicate points at.
    pub target_type: &'static str,
}

pub const IRI_DEC_FEATURE: &str = "https://decision-cli.dev/ns#Feature";
pub const IRI_DEC_ADR: &str = "https://decision-cli.dev/ns#ADR";
pub const IRI_DEC_TC: &str = "https://decision-cli.dev/ns#TC";
pub const IRI_DEC_DEPENDENCY: &str = "https://decision-cli.dev/ns#Dependency";

pub const IRI_DEC_DECIDES_FOR: &str = "https://decision-cli.dev/ns#decidesFor";
pub const IRI_DEC_VALIDATES: &str = "https://decision-cli.dev/ns#validates";
pub const IRI_DEC_REQUIRED_BY: &str = "https://decision-cli.dev/ns#requiredBy";
pub const IRI_DEC_SUPERSEDES: &str = "https://decision-cli.dev/ns#supersedes";

/// The slice-1 mapping table. Adding a row is an ADR amendment to FT-074
/// — the mapping is the load-bearing contract that determines which
/// artifacts can be backfilled.
pub const SLICE_1_MAPPING: &[MappingRow] = &[
    // ADR.features → :decidesFor → Feature
    MappingRow {
        source_type: IRI_DEC_ADR,
        source_field: "features",
        motivational_predicate: IRI_DEC_DECIDES_FOR,
        target_type: IRI_DEC_FEATURE,
    },
    // TC.validates.features → :validates → Feature
    MappingRow {
        source_type: IRI_DEC_TC,
        source_field: "validates.features",
        motivational_predicate: IRI_DEC_VALIDATES,
        target_type: IRI_DEC_FEATURE,
    },
    // TC.validates.adrs → :validates → ADR
    MappingRow {
        source_type: IRI_DEC_TC,
        source_field: "validates.adrs",
        motivational_predicate: IRI_DEC_VALIDATES,
        target_type: IRI_DEC_ADR,
    },
    // Dependency.features → :requiredBy → Feature
    MappingRow {
        source_type: IRI_DEC_DEPENDENCY,
        source_field: "features",
        motivational_predicate: IRI_DEC_REQUIRED_BY,
        target_type: IRI_DEC_FEATURE,
    },
    // Dependency.adrs → :requiredBy → ADR
    MappingRow {
        source_type: IRI_DEC_DEPENDENCY,
        source_field: "adrs",
        motivational_predicate: IRI_DEC_REQUIRED_BY,
        target_type: IRI_DEC_ADR,
    },
    // ADR.supersedes → :supersedes → ADR
    MappingRow {
        source_type: IRI_DEC_ADR,
        source_field: "supersedes",
        motivational_predicate: IRI_DEC_SUPERSEDES,
        target_type: IRI_DEC_ADR,
    },
];

/// Resolve the motivational predicates a given source type is allowed
/// to emit during backfill — the set the audit consults when checking
/// whether an artifact has any mappable informal edges.
#[must_use]
pub fn predicates_for_source_type(source_type: &str) -> Vec<&'static str> {
    SLICE_1_MAPPING
        .iter()
        .filter(|row| row.source_type == source_type)
        .map(|row| row.motivational_predicate)
        .collect()
}

/// Type-specific orphan-repair guidance used in `migration-orphan-needs-repair`
/// feedback emissions (FT-074 §Behaviour step 4). Mirrors the per-type
/// motivational vocabulary catalog from FT-070.
#[must_use]
pub fn orphan_repair_guidance(rdf_type: &str) -> String {
    match rdf_type {
        IRI_DEC_FEATURE => format!(
            "Feature requires motivational provenance. Add one of: `addresses` (→ {}), `decomposesFrom` (→ Brief), `originatedFrom` (→ DiscoveryFinding), `respondsTo` (→ Question), or declare BoundaryArtifact membership with `external_origin`.",
            IRI_DEC_FEEDBACK
        ),
        IRI_DEC_ADR => "ADR requires motivational provenance. Add one of: `addresses` (→ Question), `decidesFor` (→ Feature), `supersedes` (→ ADR), or declare BoundaryArtifact membership.".to_string(),
        IRI_DEC_TC => "TC requires motivational provenance. Add a `validates` edge (→ Feature or ADR), or declare BoundaryArtifact membership.".to_string(),
        IRI_DEC_DEPENDENCY => "Dependency requires motivational provenance. Add a `requiredBy` edge (→ Feature or ADR), or declare BoundaryArtifact membership.".to_string(),
        other => format!("Artifact type <{other}> requires motivational provenance. See FT-070's per-type vocabulary or declare BoundaryArtifact membership."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_one_mapping_covers_all_four_source_types() {
        let mut types: Vec<&str> = SLICE_1_MAPPING.iter().map(|r| r.source_type).collect();
        types.sort();
        types.dedup();
        assert!(types.contains(&IRI_DEC_ADR), "ADR mapping missing");
        assert!(types.contains(&IRI_DEC_TC), "TC mapping missing");
        assert!(
            types.contains(&IRI_DEC_DEPENDENCY),
            "Dependency mapping missing"
        );
    }

    #[test]
    fn predicates_for_source_type_returns_only_matching_predicates() {
        let adr_preds = predicates_for_source_type(IRI_DEC_ADR);
        assert!(adr_preds.contains(&IRI_DEC_DECIDES_FOR));
        assert!(adr_preds.contains(&IRI_DEC_SUPERSEDES));
        // ADR mapping does not allow `:validates` (that is TC).
        assert!(!adr_preds.contains(&IRI_DEC_VALIDATES));
    }

    #[test]
    fn orphan_guidance_is_type_specific() {
        let feature_guidance = orphan_repair_guidance(IRI_DEC_FEATURE);
        assert!(feature_guidance.contains("decomposesFrom"));
        let adr_guidance = orphan_repair_guidance(IRI_DEC_ADR);
        assert!(adr_guidance.contains("decidesFor"));
    }
}
