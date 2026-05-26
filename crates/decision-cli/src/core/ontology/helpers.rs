//! Generic helpers for the embedded ontology bundle: TTL loading, asset
//! hashing, and `owl:versionInfo` extraction. Invariant checks (FT-006
//! and friends) live in the sibling `invariants.rs` module.

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

use super::{
    OntologyError, BOUNDARY_ARTIFACT_SHAPES_TTL, MECHANICAL_PROVENANCE_SHAPES_TTL,
    MOTIVATIONAL_PREDICATES_TTL, ONTOLOGY_GRAPH_IRI, ONTOLOGY_TTL, PER_TYPE_SHAPE_FILES,
    SHAPES_MANIFEST_TTL, SHAPES_TTL,
};

pub(super) fn load_turtle_into_graph(
    store: &Store,
    ttl: &str,
    graph_iri: &str,
) -> Result<(), OntologyError> {
    let graph = NamedNode::new_unchecked(graph_iri);
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph));
    store
        .load_from_reader(parser, ttl.as_bytes())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    Ok(())
}

pub(super) fn sha256_hex_of_assets() -> String {
    let mut hasher = Sha256::new();
    hasher.update(ONTOLOGY_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(SHAPES_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(MECHANICAL_PROVENANCE_SHAPES_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(MOTIVATIONAL_PREDICATES_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(BOUNDARY_ARTIFACT_SHAPES_TTL.as_bytes());
    // FT-072: per-type shape files contribute to the embedded-asset
    // hash so any byte-level drift bumps the ontology version digest.
    for (_filename, ttl) in PER_TYPE_SHAPE_FILES {
        hasher.update(b"\x00");
        hasher.update(ttl.as_bytes());
    }
    hasher.update(b"\x00");
    hasher.update(SHAPES_MANIFEST_TTL.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub(super) fn extract_version_info(store: &Store) -> Result<Option<String>, OntologyError> {
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
