//! FT-116 — auto-close defects when a fresh approved VGR retracts the
//! failing evidence.
//!
//! When a new VGR lands with `outcome="pass"` for a TC, any open defect
//! emitted by a prior VGR of the *same graph* against the *same TC* is by
//! construction retracted — fresh evidence supersedes stale evidence. This
//! module automatically transitions those defects to `closed` at VGR-write
//! time, citing the new VGR as the retracting authority.

mod query;
mod transition;
mod pipeline;

pub mod cli;

#[cfg(test)]
mod tests;

pub use pipeline::{retract_stale_defects, retract_stale_defects_in_transaction};
