//! Embedded base ontology bundle, including SHACL shapes (FT-006 / ADR-007).
//!
//! The Turtle assets in [`assets`](self) are compiled into the binary
//! via `include_str!`. They are parsed lazily on the first
//! [`OntologyHandle::load`] call and cached in a process-wide
//! [`OnceLock`] so subsequent callers share a single parse.
//!
//! Parse failures are a build-time bug — they propagate as
//! [`OntologyError::CompiledAssetMalformed`] and bubble up to whoever
//! requested the handle. In a correctly-built binary this code path is
//! unreachable.
//!
//! Scope (mirrors FT-006 §Boundaries):
//!
//! - This module exposes the ontology + shapes graph for **other**
//!   modules to validate against. It does **not** itself validate
//!   user-supplied definition documents (FT-008) and it does **not**
//!   persist any triples into the orchestration store (FT-009).

pub mod boundary_artifact;
pub mod capability;
pub mod coverage_waiver;
mod helpers;
pub mod mechanical_provenance;
mod per_type_shapes;
pub mod role_binding;
pub mod session_record;
pub mod verdict;
pub mod verification_env;
pub mod verification_graph;
pub mod worker_image;
pub mod worker_image_submission;

pub use per_type_shapes::{
    PER_TYPE_BOUNDARY_EXEMPT_FILES, PER_TYPE_SHAPE_FILES, PER_TYPE_SHAPE_IRIS,
    SHAPES_MANIFEST_TTL,
};

use std::sync::OnceLock;

use oxigraph::model::{GraphNameRef, NamedNode, NamedNodeRef, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use helpers::{
    extract_version_info, invariant_boundary_artifact_shapes_present,
    invariant_mechanical_provenance_shapes_present, invariant_motivational_predicates_present,
    invariant_ontology_classes_present, invariant_shapes_present, load_turtle_into_graph,
    sha256_hex_of_assets,
};
use per_type_shapes::invariant_per_type_shape_files_present;

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
/// per-type shapes (FT-072) can reference `:BoundaryArtifact` class
/// membership in the first branch of their `sh:or` block.
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

/// Single shared parsed ontology — populated on first request.
static SHARED: OnceLock<OntologyHandle> = OnceLock::new();

/// A parsed view over the embedded ontology + SHACL shapes.
///
/// Cheap to clone (internal state is reference-counted via a `Store`),
/// but callers should usually hit [`OntologyHandle::load`] which
/// returns the process-wide cached instance.
#[derive(Clone)]
pub struct OntologyHandle {
    store: Store,
    hash: String,
    version: String,
}

/// Errors produced by [`OntologyHandle::load`].
#[derive(Debug, Error)]
pub enum OntologyError {
    /// The embedded Turtle did not parse — almost certainly a build-time bug.
    #[error("compiled ontology asset is malformed: {0}")]
    CompiledAssetMalformed(String),

    /// The parsed ontology omits a structural invariant required by FT-006.
    #[error("compiled ontology is missing an FT-006 invariant: {0}")]
    InvariantViolation(String),
}

impl OntologyHandle {
    /// Return the process-wide handle, parsing the embedded assets on first use.
    pub fn load() -> Result<&'static Self, OntologyError> {
        if let Some(handle) = SHARED.get() {
            return Ok(handle);
        }
        let parsed = Self::parse()?;
        Ok(SHARED.get_or_init(|| parsed))
    }

    /// Force a fresh parse, bypassing the process-wide cache.
    pub fn parse_uncached() -> Result<Self, OntologyError> {
        Self::parse()
    }

    fn parse() -> Result<Self, OntologyError> {
        let store =
            Store::new().map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;

        load_turtle_into_graph(&store, ONTOLOGY_TTL, ONTOLOGY_GRAPH_IRI)?;
        load_turtle_into_graph(&store, SHAPES_TTL, SHAPES_GRAPH_IRI)?;
        // FT-069 / ADR-038: universal mechanical-provenance fragment lives
        // beside the per-type shapes in the same shapes named graph so
        // SHACL composition (sh:and) resolves without further wiring.
        load_turtle_into_graph(
            &store,
            MECHANICAL_PROVENANCE_SHAPES_TTL,
            SHAPES_GRAPH_IRI,
        )?;
        // FT-070 / ADR-038 / ADR-039: slice-1 motivational-predicate
        // declarations. Loaded into the same shapes named graph so the
        // subPropertyOf relationship and the rdfs:range constraints are
        // queryable alongside the universal mechanical fragment.
        load_turtle_into_graph(&store, MOTIVATIONAL_PREDICATES_TTL, SHAPES_GRAPH_IRI)?;
        // FT-071 / ADR-040: BoundaryArtifact class + four slice-1
        // subclasses + per-subtype shape extensions. Loaded into the
        // shapes named graph so per-type shapes (FT-072) can reference
        // `:BoundaryArtifact` class membership in their `sh:or` blocks
        // and SPARQL `rdfs:subClassOf` reasoning is queryable.
        load_turtle_into_graph(&store, BOUNDARY_ARTIFACT_SHAPES_TTL, SHAPES_GRAPH_IRI)?;
        // FT-072 / ADR-038: per-type shape catalog. Each file composes
        // the mechanical block (loaded above) via sh:and and the
        // BoundaryArtifact branch (loaded above) via the first sh:or
        // alternative. Load order is enumerated in `PER_TYPE_SHAPE_FILES`
        // and mirrored in `assets/shapes/manifest.ttl` for non-Rust
        // consumers.
        for (_filename, ttl) in PER_TYPE_SHAPE_FILES {
            load_turtle_into_graph(&store, ttl, SHAPES_GRAPH_IRI)?;
        }

        invariant_ontology_classes_present(&store)?;
        invariant_shapes_present(&store)?;
        invariant_mechanical_provenance_shapes_present(&store)?;
        invariant_motivational_predicates_present(&store)?;
        invariant_boundary_artifact_shapes_present(&store)?;
        invariant_per_type_shape_files_present(&store)?;

        let hash = sha256_hex_of_assets();
        let version = extract_version_info(&store)?.unwrap_or_else(|| ONTOLOGY_VERSION.to_string());
        if version != ONTOLOGY_VERSION {
            return Err(OntologyError::InvariantViolation(format!(
                "ontology owl:versionInfo {version} disagrees with compile-time constant {ONTOLOGY_VERSION}"
            )));
        }

        Ok(Self {
            store,
            hash,
            version,
        })
    }

    /// SHA-256 (hex) of the embedded ontology + shapes asset bytes.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// `owl:versionInfo` parsed out of the embedded ontology header.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Underlying parsed [`Store`] holding `(ontology_graph, shapes_graph)`.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Iterator over the quads in the SHACL shapes graph.
    pub fn shapes_graph(&self) -> impl Iterator<Item = Quad> + '_ {
        let g = NamedNode::new_unchecked(SHAPES_GRAPH_IRI);
        self.store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(g.as_ref())))
            .filter_map(Result::ok)
    }

    /// Iterator over the quads in the ontology graph (TBox + property declarations).
    pub fn ontology_graph(&self) -> impl Iterator<Item = Quad> + '_ {
        let g = NamedNode::new_unchecked(ONTOLOGY_GRAPH_IRI);
        self.store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(g.as_ref())))
            .filter_map(Result::ok)
    }

    /// True iff `iri` is declared as an `rdfs:Class` by the embedded ontology.
    pub fn declares_class(&self, iri: &str) -> bool {
        let q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <http://www.w3.org/2000/01/rdf-schema#Class> }} }}",
            g = ONTOLOGY_GRAPH_IRI
        );
        matches!(
            self.store.query(q.as_str()),
            Ok(QueryResults::Boolean(true))
        )
    }

    /// True iff `iri` is declared as an `rdf:Property` by the embedded ontology.
    pub fn declares_property(&self, iri: &str) -> bool {
        let q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> }} }}",
            g = ONTOLOGY_GRAPH_IRI
        );
        matches!(
            self.store.query(q.as_str()),
            Ok(QueryResults::Boolean(true))
        )
    }

    /// Convenience: NamedNodeRef for the ontology's IRI namespace.
    #[must_use]
    pub fn ns() -> NamedNodeRef<'static> {
        NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_caches() {
        let a = OntologyHandle::load().expect("first load succeeds");
        let b = OntologyHandle::load().expect("second load succeeds");
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a.version(), b.version());
    }

    #[test]
    fn version_matches_constant() {
        let h = OntologyHandle::load().expect("load");
        assert_eq!(h.version(), ONTOLOGY_VERSION);
    }

    #[test]
    fn hash_is_deterministic_hex_sha256() {
        let h = OntologyHandle::load().expect("load");
        assert_eq!(h.hash().len(), 64);
        assert!(h.hash().chars().all(|c| c.is_ascii_hexdigit()));
        let again = OntologyHandle::parse_uncached().expect("re-parse");
        assert_eq!(h.hash(), again.hash());
    }

    #[test]
    fn declares_named_classes() {
        let h = OntologyHandle::load().expect("load");
        for c in [
            "https://decision-cli.dev/ns#ValueStream",
            "https://decision-cli.dev/ns#ValueAction",
            "https://decision-cli.dev/ns#Goal",
            "https://decision-cli.dev/ns#Session",
            "https://decision-cli.dev/ns#Dispatch",
            "https://decision-cli.dev/ns#Event",
            "https://decision-cli.dev/ns#Role",
            "https://decision-cli.dev/ns#Authority",
            "https://decision-cli.dev/ns#ActionSession",
            "https://decision-cli.dev/ns#InterpretationSession",
            "https://decision-cli.dev/ns#DispatchGroup",
            "https://decision-cli.dev/ns#Feedback",
            "https://decision-cli.dev/ns#RoleBinding",
            "https://decision-cli.dev/ns#EscalationStep",
            "https://decision-cli.dev/ns#EscalationTrigger",
            "https://decision-cli.dev/ns#Bundle",
        ] {
            assert!(h.declares_class(c), "missing class declaration: {c}");
        }
    }

    #[test]
    fn shapes_graph_is_non_empty() {
        let h = OntologyHandle::load().expect("load");
        let count = h.shapes_graph().count();
        assert!(count > 0, "shapes graph should not be empty");
    }

    #[test]
    fn ontology_graph_is_non_empty() {
        let h = OntologyHandle::load().expect("load");
        let count = h.ontology_graph().count();
        assert!(count > 0, "ontology graph should not be empty");
    }
}
