//! Embedded base ontology and SHACL shapes (FT-006 / ADR-007).
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

use std::sync::OnceLock;

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, GraphNameRef, NamedNode, NamedNodeRef, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The version baked into the embedded ontology header. Bumping this
/// constant requires regenerating `ontology.ttl`'s `owl:versionInfo`
/// and is intentionally tied to a binary release.
///
/// Recorded on every bootstrap session (ADR-007).
pub const ONTOLOGY_VERSION: &str = "0.1.0";

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
    /// Parsed triples for ontology + shapes, each in its own named graph.
    store: Store,
    /// The SHA-256 of the concatenated raw asset bytes (ontology + shapes).
    hash: String,
    /// `owl:versionInfo` parsed out of the ontology header.
    version: String,
}

/// Errors produced by [`OntologyHandle::load`].
///
/// In a correctly-built binary every variant is unreachable; they
/// exist so a corrupted asset surfaces with a clear diagnostic rather
/// than a panic.
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
    ///
    /// The cache key is implicit — every call within the same process
    /// returns a handle backed by the same parsed [`Store`].
    pub fn load() -> Result<&'static Self, OntologyError> {
        if let Some(handle) = SHARED.get() {
            return Ok(handle);
        }
        let parsed = Self::parse()?;
        // If two callers race, OnceLock keeps the first; the second
        // drops its parsed copy. Cheap relative to the cost of the
        // contention itself.
        Ok(SHARED.get_or_init(|| parsed))
    }

    /// Force a fresh parse, bypassing the process-wide cache.
    ///
    /// Intended for tests that want to verify the parse-and-hash flow
    /// without relying on previously-cached state.
    pub fn parse_uncached() -> Result<Self, OntologyError> {
        Self::parse()
    }

    fn parse() -> Result<Self, OntologyError> {
        let store =
            Store::new().map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;

        load_turtle_into_graph(&store, ONTOLOGY_TTL, ONTOLOGY_GRAPH_IRI)?;
        load_turtle_into_graph(&store, SHAPES_TTL, SHAPES_GRAPH_IRI)?;

        // FT-006 §Invariants: declare at minimum the named classes,
        // and the ValueStream / ValueAction shapes must be present.
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
    ///
    /// FT-008 reads from here when validating user documents against
    /// the shapes graph; FT-009 leaves it untouched.
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
        matches!(self.store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
    }

    /// True iff `iri` is declared as an `rdf:Property` by the embedded ontology.
    pub fn declares_property(&self, iri: &str) -> bool {
        let q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> }} }}",
            g = ONTOLOGY_GRAPH_IRI
        );
        matches!(self.store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
    }

    /// Convenience: NamedNodeRef for the ontology's IRI namespace.
    #[must_use]
    pub fn ns() -> NamedNodeRef<'static> {
        NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#")
    }
}

fn load_turtle_into_graph(
    store: &Store,
    ttl: &str,
    graph_iri: &str,
) -> Result<(), OntologyError> {
    use oxigraph::io::RdfParser;
    let graph = NamedNode::new_unchecked(graph_iri);
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph));
    store
        .load_from_reader(parser, ttl.as_bytes())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    Ok(())
}

fn sha256_hex_of_assets() -> String {
    let mut hasher = Sha256::new();
    hasher.update(ONTOLOGY_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(SHAPES_TTL.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn extract_version_info(store: &Store) -> Result<Option<String>, OntologyError> {
    let q = format!(
        "SELECT ?v WHERE {{ GRAPH <{g}> {{ <https://decision-cli.dev/ns> <http://www.w3.org/2002/07/owl#versionInfo> ?v }} }} LIMIT 1",
        g = ONTOLOGY_GRAPH_IRI
    );
    let results = store
        .query(q.as_str())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    let QueryResults::Solutions(mut sols) = results else {
        return Err(OntologyError::CompiledAssetMalformed(
            "owl:versionInfo query returned a non-solution result".to_string(),
        ));
    };
    if let Some(sol) = sols.next() {
        let sol = sol.map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
        if let Some(term) = sol.get("v") {
            if let oxigraph::model::Term::Literal(lit) = term {
                return Ok(Some(lit.value().to_string()));
            }
        }
    }
    Ok(None)
}

fn invariant_ontology_classes_present(store: &Store) -> Result<(), OntologyError> {
    // FT-006 §Invariants: at minimum declare ValueStream, ValueAction,
    // Goal, Session, Dispatch, Event.
    for class in [
        "ValueStream",
        "ValueAction",
        "Goal",
        "Session",
        "Dispatch",
        "Event",
    ] {
        let iri = format!("https://decision-cli.dev/ns#{class}");
        let q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <http://www.w3.org/2000/01/rdf-schema#Class> }} }}",
            g = ONTOLOGY_GRAPH_IRI
        );
        let result = store
            .query(q.as_str())
            .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
        if !matches!(result, QueryResults::Boolean(true)) {
            return Err(OntologyError::InvariantViolation(format!(
                "ontology must declare dec:{class} as rdfs:Class"
            )));
        }
    }
    Ok(())
}

fn invariant_shapes_present(store: &Store) -> Result<(), OntologyError> {
    // FT-006 §Invariants: ValueStream shape requires name/title/
    // terminalValueAction/authorizedGoals; ValueAction shape requires
    // name/description/exitCriterion/expectedOutputType/
    // compatibleGoals.
    let required = [
        (
            "https://decision-cli.dev/ns#ValueStream",
            &[
                "https://decision-cli.dev/ns#name",
                "https://decision-cli.dev/ns#title",
                "https://decision-cli.dev/ns#terminalValueAction",
                "https://decision-cli.dev/ns#authorizedGoals",
            ][..],
        ),
        (
            "https://decision-cli.dev/ns#ValueAction",
            &[
                "https://decision-cli.dev/ns#name",
                "https://decision-cli.dev/ns#description",
                "https://decision-cli.dev/ns#exitCriterion",
                "https://decision-cli.dev/ns#expectedOutputType",
                "https://decision-cli.dev/ns#compatibleGoals",
            ][..],
        ),
    ];
    for (target_class, props) in required {
        for prop in props {
            let q = format!(
                "ASK {{ GRAPH <{g}> {{ \
                    ?shape <http://www.w3.org/ns/shacl#targetClass> <{target_class}> ; \
                           <http://www.w3.org/ns/shacl#property> ?p . \
                    ?p <http://www.w3.org/ns/shacl#path> <{prop}> ; \
                       <http://www.w3.org/ns/shacl#minCount> ?_ . \
                 }} }}",
                g = SHAPES_GRAPH_IRI
            );
            let result = store
                .query(q.as_str())
                .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
            if !matches!(result, QueryResults::Boolean(true)) {
                return Err(OntologyError::InvariantViolation(format!(
                    "shapes graph is missing a sh:minCount constraint on {prop} for {target_class}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_caches() {
        let a = OntologyHandle::load().expect("first load succeeds");
        let b = OntologyHandle::load().expect("second load succeeds");
        // Cached: same hash, same version, same parsed store identity.
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
        // 64 hex chars for sha256.
        assert_eq!(h.hash().len(), 64);
        assert!(h.hash().chars().all(|c| c.is_ascii_hexdigit()));
        // Stable across loads.
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
