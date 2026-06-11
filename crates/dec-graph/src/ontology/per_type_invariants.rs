//! Store-backed FT-072 invariant checks for the per-type shape catalog.
//!
//! The catalog constants themselves ([`PER_TYPE_SHAPE_FILES`],
//! [`PER_TYPE_SHAPE_IRIS`]) are pure data and live in
//! [`dec_ontology::ontology::per_type_shapes`] (ADR-086); this module
//! keeps only the SPARQL `ASK` validation the ontology loader runs.

use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use dec_ontology::ontology::{OntologyError, PER_TYPE_SHAPE_IRIS, SHAPES_GRAPH_IRI};

/// FT-072 / ADR-038: every per-type shape file in
/// [`PER_TYPE_SHAPE_FILES`] must declare its catalog-published shape
/// IRI (see [`PER_TYPE_SHAPE_IRIS`]) as a `sh:NodeShape` in the shapes
/// graph. Failure means the shape file's body drifted from the catalog
/// (either the file is missing the shape or the catalog points at the
/// wrong IRI) — a build-time bug.
pub(super) fn invariant_per_type_shape_files_present(store: &Store) -> Result<(), OntologyError> {
    for (filename, shape_iri) in PER_TYPE_SHAPE_IRIS {
        ensure_per_type_node_shape(store, filename, shape_iri)?;
    }
    Ok(())
}

fn ensure_per_type_node_shape(
    store: &Store,
    filename: &str,
    shape_iri: &str,
) -> Result<(), OntologyError> {
    let q = format!(
        "ASK {{ GRAPH <{g}> {{ <{shape_iri}> a <http://www.w3.org/ns/shacl#NodeShape> }} }}",
        g = SHAPES_GRAPH_IRI
    );
    let result = store
        .query(q.as_str())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    if !matches!(result, QueryResults::Boolean(true)) {
        return Err(OntologyError::InvariantViolation(format!(
            "per-type shape file '{filename}' must declare <{shape_iri}> as sh:NodeShape"
        )));
    }
    Ok(())
}
