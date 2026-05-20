//! Definition byte loading + Turtle parsing helpers.

use std::fs;
use std::path::PathBuf;

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode};
use oxigraph::store::Store;

use crate::core::bundled;

use super::{DefinitionSource, InitError};

pub(super) fn read_definition_bytes(
    source: &DefinitionSource,
) -> Result<(Vec<u8>, String, Option<String>), InitError> {
    match source {
        DefinitionSource::Template(name) => {
            let t = bundled::lookup_stream_template(name).ok_or_else(|| {
                InitError::UnknownTemplate {
                    name: name.clone(),
                    available: bundled::known_template_names().join(", "),
                }
            })?;
            Ok((
                t.ttl.as_bytes().to_vec(),
                source.label(),
                Some(t.iri.to_string()),
            ))
        }
        DefinitionSource::File(path) => {
            let bytes = fs::read(path).map_err(|source| InitError::ReadFailed {
                path: path.clone(),
                source,
            })?;
            Ok((bytes, source.label(), None))
        }
    }
}

pub(super) fn parse_into_graph(
    store: &Store,
    bytes: &[u8],
    graph: &NamedNode,
    source_label: &str,
    base_iri: Option<&str>,
) -> Result<(), InitError> {
    let mut parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph.clone()));
    if let Some(b) = base_iri {
        parser = parser
            .with_base_iri(b)
            .map_err(|e| InitError::ParseFailed {
                source_label: source_label.to_string(),
                detail: format!("invalid base IRI {b}: {e}"),
            })?;
    }
    store
        .load_from_reader(parser, bytes)
        .map_err(|e| InitError::ParseFailed {
            source_label: source_label.to_string(),
            detail: e.to_string(),
        })?;
    Ok(())
}

pub(super) fn _retain_path_import() -> Option<PathBuf> {
    None
}
