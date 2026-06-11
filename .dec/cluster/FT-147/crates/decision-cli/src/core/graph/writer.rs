use crate::core::graph::error::GraphError;
use crate::core::ontology::archetype::Archetype;
use crate::core::ontology::vocab::archetype as vocab;
use oxigraph::model::{NamedNode, Quad, Subject, GraphName};
use oxigraph::store::Store;
use std::collections::HashSet;

/// Writer for graph operations.
pub struct GraphWriter<'a> {
    store: &'a Store,
}

impl<'a> GraphWriter<'a> {
    /// Creates a new GraphWriter.
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// Writes an archetype to the graph.
    pub fn write_archetype(&self, archetype: &Archetype) -> Result<(), GraphError> {
        let quads = crate::core::ontology::archetype::emit_archetype(archetype);
        self.store.insert(quads.iter())?;
        Ok(())
    }
}