//! BoundaryArtifact class + slice-1 subclasses + SHACL validators (FT-071 / ADR-040).
//!
//! The dual-provenance discipline (ADR-038) splits provenance into a
//! universal mechanical block (FT-069) plus at least one per-type
//! motivational predicate (FT-070). Some artifacts legitimately have NO
//! graph-internal motivational ancestor — their motivational origin is
//! external. `dec:BoundaryArtifact` is the orphan-motivational escape
//! hatch: instances are exempt from per-type `sh:or` motivational
//! requirements at the type-shape level (FT-072 wires the boundary
//! branch into each per-type `sh:or`) but MUST still carry:
//!
//!   * the universal mechanical block (FT-069), and
//!   * an explicit `dec:external_origin` literal documenting how the
//!     artifact entered the system.
//!
//! `dec:MigrationBackfill` is a subtype that additionally requires
//! `dec:isMigrationBackfill true` so synthetic backfill provenance is
//! queryable (ADR-042 / FT-074).
//!
//! Scope of this module:
//!
//!   * IRI constants and `NamedNodeRef` accessors for the class hierarchy.
//!   * [`validate_boundary_artifact`] — Rust-side validator mirroring
//!     `:BoundaryArtifactShape`'s `dec:external_origin` constraint.
//!   * [`validate_migration_backfill`] — Rust-side validator mirroring
//!     `:MigrationBackfillShape`'s `dec:isMigrationBackfill true`
//!     constraint.
//!
//! Per-type composition (which artifact type accepts BoundaryArtifact
//! class membership as a satisfying alternative for its motivational
//! `sh:or`) lives in FT-072's per-type shape files; this module ships
//! only the boundary primitives.

mod shacl;
mod types;

#[cfg(test)]
mod tests;

pub use shacl::{
    validate_boundary_artifact, validate_migration_backfill, BoundaryArtifactShaclError,
    BoundaryArtifactViolation,
};
pub use types::{
    boundary_artifact_class, bootstrap_artifact_class, external_origin_pred, initial_request_class,
    is_migration_backfill_pred, migration_backfill_class, sensing_action_output_class,
};
