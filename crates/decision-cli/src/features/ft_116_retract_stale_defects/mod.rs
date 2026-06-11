//! FT-116 vertical slice: `dec _retract-stale-defects` CLI wrapper.
//!
//! The retraction machinery was promoted to
//! [`dec_harness::verify::stale_defects`] under the ADR-016 promotion
//! rule (ADR-086 / FT-169) because the verification runner invokes it on
//! every VGR commit. This slice keeps the operator-facing CLI and
//! re-exports the promoted surface for existing import paths.

pub mod cli;

pub use dec_harness::verify::stale_defects::{
    retract_stale_defects, retract_stale_defects_in_transaction,
};
