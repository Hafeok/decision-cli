//! TaskType + Cell catalog substrate (FT-139 / ADR-080).
//!
//! Implements the typed-dispatch half of the DDD task/cell decomposition
//! pattern from `applications/sdlc.md`. The broad code-writer remains
//! the unknown-task fallback (FT-123); recognised TaskTypes route here
//! through a classifier branch and dispatch their declared cell
//! clusters with a per-task coherence audit.
//!
//! Module layout:
//! - `types` — `TaskTypeDecl`, `CellDecl`, `CoherenceAuditSpec`.
//! - `topo` — Kahn-style topological sort over `derived_from`.
//! - `registry` — static catalog of recognised TaskTypes (currently:
//!   `add-judge-worker`).
//!
//! Future slices add more TaskTypes (FT-140..FT-144 author them as
//! feature_specs; their implementation extends the registry here).

pub mod types;
pub mod topo;
pub mod registry;

#[cfg(test)]
pub mod tests;

pub use types::{CellDecl, CoherenceAuditSpec, TaskTypeDecl};
pub use topo::{topo_order, TopoError};
pub use registry::{lookup, registered_names};
