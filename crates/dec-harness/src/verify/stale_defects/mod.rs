//! Auto-close stale defects when a fresh approved VGR retracts the
//! failing evidence (FT-116).
//!
//! Promoted from `features::ft_116_retract_stale_defects` under the
//! ADR-016 promotion rule (ADR-086 / FT-169): the verification runner's
//! trace writer invokes the in-transaction variant on every VGR commit,
//! so the machinery belongs below the feature slice. The `dec
//! _retract-stale-defects` CLI wrapper stays in the slice.

mod pipeline;
mod query;
mod transition;

#[cfg(test)]
mod tests;

pub use pipeline::{retract_stale_defects, retract_stale_defects_in_transaction};
