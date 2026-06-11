//! Typed write path for `dec:Archetype` (FT-147 §Behaviour 2–3).
//!
//! Routes through the existing SHACL-enforced chokepoint: quad-level
//! validation (incl. E102) runs inside [`StreamWriter::commit`] like
//! every other artifact type; the ADR-085 §6 status gate (E020) runs
//! here because it needs the stored prior status.

use anyhow::{anyhow, Result};
use oxi_events::Mutation;
use oxigraph::model::NamedNodeRef;

use dec_ontology::ontology::archetype::Archetype;

use super::promotion::{validate_status_transition_with_store, StatusWriteAuthority};
use crate::stream_writer::StreamWriter;

/// Validate and commit `archetype` into `graph` through the chokepoint.
///
/// `authority` decides whether status changes are permitted: every
/// caller except the `dec archetype promote`/`demote` path (FT-158)
/// passes [`StatusWriteAuthority::Standard`] and is refused with E020 on
/// any attempt to mint or change `dec:status` (ADR-085 §6).
pub fn write_archetype(
    writer: &StreamWriter,
    archetype: &Archetype,
    graph: NamedNodeRef<'_>,
    authority: StatusWriteAuthority,
) -> Result<()> {
    let quads = archetype.to_quads(graph);

    validate_status_transition_with_store(writer.inner().store().as_ref(), &quads, authority)
        .map_err(|err| anyhow!("SHACL violation: archetype mutation refused\n{err}"))?;

    writer
        .commit(Mutation::insert(quads))
        .map_err(|err| anyhow!("archetype commit failed: {err:#}"))?;
    Ok(())
}
