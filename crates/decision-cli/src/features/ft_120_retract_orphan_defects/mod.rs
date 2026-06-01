//! FT-120 — retract orphaned defects when source-VG topology no longer
//! covers the TC.
//!
//! FT-116 closes defects when fresh evidence retracts them inside the same
//! VG. FT-120 covers the dual case: the TC has left the VG entirely (runner
//! migration, TC unlink, VG deprecation), so no future VGR will ever
//! retract the defect. This module finds those orphaned defects and
//! transitions them to `superseded` via the legal `produced → superseded`
//! ADR-024 path, citing a `dec:supersededByTopologyChange` predicate.

mod query;
mod transition;
mod pipeline;

pub mod cli;

#[cfg(test)]
mod tests;

pub use pipeline::{
    retract_orphans_all, retract_orphans_for_feature, retract_orphans_for_graph, RetractScope,
    RetractSummary,
};
