//! Embedded base ontology bundle, including SHACL shapes (FT-006 / ADR-007).
//!
//! The Turtle assets under `assets/` are compiled into consumers via
//! `include_str!` on the constants below. This crate exposes the raw
//! asset bytes, the IRI constants, and the typed artifact submodules;
//! *parsing* the assets into a queryable store (and validating the
//! FT-006 structural invariants) is the job of decision-cli's
//! `core::ontology::OntologyHandle`, which sits above the store
//! boundary this crate must not cross (ADR-086).

pub mod application_contract;
pub mod archetype;
pub mod boundary_artifact;
pub mod capability;
pub mod catalog;
pub mod conformance_audit;
pub mod coverage_waiver;
pub mod feedback;
pub mod mechanical_provenance;
pub mod per_type_shapes;
pub mod provenance;
pub mod role_binding;
pub mod session_record;
pub mod verdict;
pub mod verification_bench;
pub mod verification_graph;
pub mod verification_result;
pub mod worker_image;
pub mod worker_image_submission;

pub use per_type_shapes::{
    PER_TYPE_BOUNDARY_EXEMPT_FILES, PER_TYPE_SHAPE_FILES, PER_TYPE_SHAPE_IRIS, SHAPES_MANIFEST_TTL,
};

use thiserror::Error;

/// The version baked into the embedded ontology header. Bumping this
/// constant requires regenerating `ontology.ttl`'s `owl:versionInfo`
/// and is intentionally tied to a binary release.
///
/// Recorded on every bootstrap session (ADR-007).
pub const ONTOLOGY_VERSION: &str = "0.7.0";

/// Named graph the embedded ontology is loaded into. Exposed so FT-008
/// can run shapes / queries against the same graph the handle parses.
pub const ONTOLOGY_GRAPH_IRI: &str = "https://decision-cli.dev/ns/ontology";

/// Named graph the embedded SHACL shapes are loaded into.
pub const SHAPES_GRAPH_IRI: &str = "https://decision-cli.dev/ns/shapes";

/// Raw Turtle bytes for the base ontology. Read-only.
pub const ONTOLOGY_TTL: &str = include_str!("assets/ontology.ttl");

/// Raw Turtle bytes for the SHACL shapes graph. Read-only.
pub const SHAPES_TTL: &str = include_str!("assets/shapes.ttl");

/// Raw Turtle bytes for the universal mechanical-provenance fragment
/// (FT-069 / ADR-038). Loaded into the shapes named graph alongside
/// `SHAPES_TTL`. Kept as a separate file so per-type shapes shipped
/// later (FT-072) can reference the universal fragment by its IRI
/// without textual inlining.
pub const MECHANICAL_PROVENANCE_SHAPES_TTL: &str =
    include_str!("assets/shapes/mechanical-provenance.ttl");

/// Raw Turtle bytes for the slice-1 motivational-predicate vocabulary
/// (FT-070 / ADR-038 / ADR-039). Loaded into the shapes named graph;
/// each motivational predicate is declared as
/// `rdfs:subPropertyOf prov:wasDerivedFrom` so the full-chain traversal
/// (FT-075 / ADR-043) walks them uniformly.
pub const MOTIVATIONAL_PREDICATES_TTL: &str =
    include_str!("assets/shapes/motivational-predicates.ttl");

/// Raw Turtle bytes for the BoundaryArtifact class + four slice-1
/// subclasses + their SHACL shapes (FT-071 / ADR-040). Loaded into the
/// shapes named graph alongside the universal mechanical fragment so
/// per-type shapes (FT-072) reference `:BoundaryArtifact` class membership.
pub const BOUNDARY_ARTIFACT_SHAPES_TTL: &str = include_str!("assets/shapes/boundary-artifact.ttl");

/// IRI of the universal mechanical-provenance `sh:NodeShape` (FT-069 /
/// ADR-038). Composed via `sh:and` from every artifact-type shape.
pub const MECHANICAL_PROVENANCE_SHAPE: &str =
    "https://decision-cli.dev/ns#MechanicalProvenanceShape";

/// IRI of the `dec:Session` provenance shape (FT-069 / ADR-038). Targets
/// `dec:Session` and composes the universal mechanical fragment.
pub const SESSION_PROVENANCE_SHAPE: &str = "https://decision-cli.dev/ns#SessionProvenanceShape";

/// IRI of the PROV-O `wasDerivedFrom` property. Parent property of every
/// motivational predicate shipped in [`MOTIVATIONAL_PREDICATES_TTL`]
/// (FT-070 / ADR-039).
pub const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";

/// IRI of the `dec:BoundaryArtifact` top class (FT-071 / ADR-040).
pub const BOUNDARY_ARTIFACT_CLASS: &str = "https://decision-cli.dev/ns#BoundaryArtifact";

/// IRI of the `dec:SensingActionOutput` BoundaryArtifact subclass.
pub const SENSING_ACTION_OUTPUT: &str = "https://decision-cli.dev/ns#SensingActionOutput";

/// IRI of the `dec:InitialRequest` BoundaryArtifact subclass.
pub const INITIAL_REQUEST: &str = "https://decision-cli.dev/ns#InitialRequest";

/// IRI of the `dec:BootstrapArtifact` BoundaryArtifact subclass.
pub const BOOTSTRAP_ARTIFACT: &str = "https://decision-cli.dev/ns#BootstrapArtifact";

/// IRI of the `dec:MigrationBackfill` BoundaryArtifact subclass.
pub const MIGRATION_BACKFILL: &str = "https://decision-cli.dev/ns#MigrationBackfill";

/// IRI of the `dec:external_origin` property required by `:BoundaryArtifactShape`.
pub const EXTERNAL_ORIGIN_PROP: &str = "https://decision-cli.dev/ns#external_origin";

/// IRI of the `dec:isMigrationBackfill` property required by `:MigrationBackfillShape`.
pub const IS_MIGRATION_BACKFILL_PROP: &str = "https://decision-cli.dev/ns#isMigrationBackfill";

/// IRI of the `:BoundaryArtifactShape` SHACL NodeShape (FT-071 / ADR-040).
pub const BOUNDARY_ARTIFACT_SHAPE: &str = "https://decision-cli.dev/ns#BoundaryArtifactShape";

/// IRI of the `:MigrationBackfillShape` SHACL NodeShape extension (FT-071 / ADR-042).
pub const MIGRATION_BACKFILL_SHAPE: &str = "https://decision-cli.dev/ns#MigrationBackfillShape";

/// The four slice-1 BoundaryArtifact subclasses, cross-checked against
/// the shipped TTL by FT-071's exit-criterion test (TC-121).
pub const BOUNDARY_ARTIFACT_SUBCLASSES: &[&str] = &[
    SENSING_ACTION_OUTPUT,
    INITIAL_REQUEST,
    BOOTSTRAP_ARTIFACT,
    MIGRATION_BACKFILL,
];

/// The slice-1 motivational-predicate IRIs declared by
/// [`MOTIVATIONAL_PREDICATES_TTL`] (FT-070). The list mirrors the
/// slice-1 vocabulary table in FT-070's feature_spec; the parsed shape
/// file is cross-checked against this constant by FT-070's exit-criterion
/// test (TC-120) to keep the Turtle file and the Rust source in lockstep.
pub const MOTIVATIONAL_PREDICATES: &[&str] = &[
    "https://decision-cli.dev/ns#addresses",
    "https://decision-cli.dev/ns#decomposesFrom",
    "https://decision-cli.dev/ns#originatedFrom",
    "https://decision-cli.dev/ns#respondsTo",
    "https://decision-cli.dev/ns#decidesFor",
    "https://decision-cli.dev/ns#supersedes",
    "https://decision-cli.dev/ns#validates",
    "https://decision-cli.dev/ns#requiredBy",
    "https://decision-cli.dev/ns#motivatedBy",
    "https://decision-cli.dev/ns#observedIn",
    "https://decision-cli.dev/ns#observedVia",
    "https://decision-cli.dev/ns#producedBy",
    "https://decision-cli.dev/ns#derivedFrom",
    "https://decision-cli.dev/ns#raisedIn",
    "https://decision-cli.dev/ns#raisedBy",
    "https://decision-cli.dev/ns#audits",
];

/// Errors produced when loading or validating the embedded ontology.
#[derive(Debug, Error)]
pub enum OntologyError {
    /// The embedded Turtle did not parse — almost certainly a build-time bug.
    #[error("compiled ontology asset is malformed: {0}")]
    CompiledAssetMalformed(String),

    /// The parsed ontology omits a structural invariant required by FT-006.
    #[error("compiled ontology is missing an FT-006 invariant: {0}")]
    InvariantViolation(String),
}
