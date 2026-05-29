//! `dec drive ship --all` — multi-feature sweep (FT-111).
//!
//! Iterates every product feature, runs the existing FT-110 ship driver
//! per feature with a per-feature timeout, and reports a structured tally.
//! Implements PAT-003: per-item bounded execution, per-item failure
//! isolation, deterministic ordering, pure formatter.

pub mod format;
pub mod sweep;

#[cfg(test)]
mod tests;

pub use format::{render, Format};
pub use sweep::{
    apply_filter, resolve_features, run_sweep, Outcome, Row, SweepError, SweepInput, Tally,
};
