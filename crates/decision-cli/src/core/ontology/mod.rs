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

mod helpers;
pub mod coverage_waiver;
pub mod verdict;
pub mod verification_env;
pub mod verification_graph;

use std::sync::OnceLock;

use oxigraph::model::{GraphNameRef, NamedNode, NamedNodeRef, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use helpers::{
    extract_version_info, invariant_ontology_classes_present, invariant_shapes_present,
    load_turtle_into_graph, sha256_hex_of_assets,
};

/// The version baked into the embedded ontology header. Bumping this
/// constant requires regenerating `ontology.ttl`'s `owl:versionInfo`
/// and is intentionally tied to a binary release.
///
/// Recorded on every bootstrap session (ADR-007).
pub const ONTOLOGY_VERSION: &str = "0.3.0";

/// Named graph the embedded ontology is loaded into. Exposed so FT-008
/// can run shapes / queries against the same graph the handle parses.
pub const ONTOLOGY_GRAPH_IRI: &str = "https://decision-cli.dev/ns/ontology";

/// Named graph the embedded SHACL shapes are loaded into.
pub const SHAPES_GRAPH_IRI: &str = "https://decision-cli.dev/ns/shapes";

/// Raw Turtle bytes for the base ontology. Read-only.
pub const ONTOLOGY_TTL: &str = include_str!("assets/ontology.ttl");

/// Raw Turtle bytes for the SHACL shapes graph. Read-only.
pub const SHAPES_TTL: &str = include_str!("assets/shapes.ttl");

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

        invariant_ontology_classes_present(&store)?;
        invariant_shapes_present(&store)?;

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
