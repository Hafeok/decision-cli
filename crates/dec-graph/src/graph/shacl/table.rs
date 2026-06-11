//! Per-type shape table for the FT-073 slice-1 validator.
//!
//! Mirrors the per-type TTL files in `core::ontology::assets::shapes`.
//! Cross-checked at test time by `tests::validator_table_matches_shape_files`
//! so the in-Rust table and the shipped TTL stay in lockstep.

use std::collections::{BTreeMap, BTreeSet};

/// One per-type provenance shape — slice-1 mirror of the TTL files in
/// `core::ontology::assets::shapes` enumerated in FT-072.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeShape {
    /// `sh:targetClass` IRI.
    pub class_iri: String,
    /// Motivational predicates the type accepts (one suffices per the
    /// per-shape `sh:or` block).
    pub motivational: BTreeSet<String>,
    /// True iff the per-type shape includes the BoundaryArtifact escape
    /// hatch as the first `sh:or` branch (FT-072 §Invariants).
    pub accepts_boundary: bool,
    /// True iff the type has no `sh:or` motivational block at all
    /// (Session, Dispatch — per FT-072's `PER_TYPE_BOUNDARY_EXEMPT_FILES`
    /// and the mechanical-provenance.ttl `SessionProvenanceShape`).
    pub motivational_exempt: bool,
}

/// Build the slice-1 type-shape table.
#[must_use]
pub fn slice1_type_shapes() -> BTreeMap<String, TypeShape> {
    let dec = "https://decision-cli.dev/ns#";
    let mut t: BTreeMap<String, TypeShape> = BTreeMap::new();
    for (class, preds, accepts_boundary, motivational_exempt) in SLICE1_TYPE_ROWS {
        let class_iri = format!("{dec}{class}");
        let motivational: BTreeSet<String> = preds.iter().map(|p| format!("{dec}{p}")).collect();
        t.insert(
            class_iri.clone(),
            TypeShape {
                class_iri,
                motivational,
                accepts_boundary: *accepts_boundary,
                motivational_exempt: *motivational_exempt,
            },
        );
    }
    t
}

/// (class IRI shortname, motivational predicate shortnames,
/// accepts_boundary, motivational_exempt) — the slice-1 mirror of the
/// per-type TTL files in `core::ontology::assets::shapes/`.
const SLICE1_TYPE_ROWS: &[(&str, &[&str], bool, bool)] = &[
    ("Acknowledgement", &["motivatedBy"], true, false),
    (
        "ADR",
        &["addresses", "decidesFor", "supersedes"],
        true,
        false,
    ),
    ("Brief", &["respondsTo"], true, false),
    ("ConformanceAudit", &["audits"], true, false),
    ("Dependency", &["requiredBy"], true, false),
    ("DiscoveryFinding", &["derivedFrom"], true, false),
    ("Dispatch", &[], false, true),
    (
        "Feature",
        &[
            "addresses",
            "decomposesFrom",
            "originatedFrom",
            "respondsTo",
        ],
        true,
        false,
    ),
    (
        "Feedback",
        &["observedIn", "observedVia", "producedBy"],
        true,
        false,
    ),
    ("Model", &["addresses", "decomposesFrom"], true, false),
    ("Policy", &["addresses", "decomposesFrom"], true, false),
    (
        "QueryTemplate",
        &["decomposesFrom", "addresses"],
        true,
        false,
    ),
    ("Question", &["raisedIn", "raisedBy"], true, false),
    ("Session", &[], false, true),
    ("Subscription", &["motivatedBy"], true, false),
    ("TC", &["validates"], true, false),
    ("WorkerImage", &["addresses", "decomposesFrom"], true, false),
    ("WorkerImageSubmission", &[], true, false),
];

/// Boundary class set — `dec:BoundaryArtifact` plus its slice-1
/// subclasses. Passed in rather than imported so this module stays
/// free of the ontology side-effects on parse.
#[must_use]
pub fn slice1_boundary_class_set(top: &str, subclasses: &[&str]) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = BTreeSet::new();
    s.insert(top.to_string());
    for cls in subclasses {
        s.insert((*cls).to_string());
    }
    s
}
