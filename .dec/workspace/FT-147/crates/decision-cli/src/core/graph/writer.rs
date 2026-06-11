//! Graph writer implementation.

use crate::core::graph::writer::GraphWriterError;
use dec_ontology::ontology::archetype::{Archetype, emit_archetype};
use oxigraph::model::{Graph, Quad, NamedNode};
use std::collections::HashSet;

/// Writer for graph operations.
pub struct GraphWriter {
    /// The underlying graph storage.
    graph: Graph,
}

impl GraphWriter {
    /// Creates a new graph writer.
    pub fn new(graph: Graph) -> Self {
        Self { graph }
    }

    /// Writes an archetype to the graph.
    pub fn write_archetype(&mut self, archetype: &Archetype) -> Result<(), GraphWriterError> {
        let quads = emit_archetype(archetype);
        self.graph.extend(quads);
        Ok(())
    }
}