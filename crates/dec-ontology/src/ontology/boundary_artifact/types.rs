//! NamedNodeRef accessors for the FT-071 BoundaryArtifact vocabulary.
//!
//! IRI constants live in `core::ontology` (re-exported from the parent
//! `core::ontology::mod`) so callers can `use decision_cli::core::ontology::{
//! BOUNDARY_ARTIFACT_CLASS, EXTERNAL_ORIGIN_PROP}`. The thin wrappers
//! here construct stable `NamedNodeRef` views over those constants for
//! oxigraph-quad construction.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

use crate::ontology::{
    BOOTSTRAP_ARTIFACT, BOUNDARY_ARTIFACT_CLASS, EXTERNAL_ORIGIN_PROP, INITIAL_REQUEST,
    IS_MIGRATION_BACKFILL_PROP, MIGRATION_BACKFILL, SENSING_ACTION_OUTPUT,
};

#[must_use]
pub fn boundary_artifact_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(BOUNDARY_ARTIFACT_CLASS)
}

#[must_use]
pub fn sensing_action_output_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(SENSING_ACTION_OUTPUT)
}

#[must_use]
pub fn initial_request_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(INITIAL_REQUEST)
}

#[must_use]
pub fn bootstrap_artifact_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(BOOTSTRAP_ARTIFACT)
}

#[must_use]
pub fn migration_backfill_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(MIGRATION_BACKFILL)
}

#[must_use]
pub fn external_origin_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(EXTERNAL_ORIGIN_PROP)
}

#[must_use]
pub fn is_migration_backfill_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IS_MIGRATION_BACKFILL_PROP)
}
