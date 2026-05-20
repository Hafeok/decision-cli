//! Action-interpretation agreement metric substrate (FT-024 / ADR-021).
//!
//! Computes the four agreement-related rates declared in ADR-021 against
//! a persisted orchestration store. Per the slice-level SDP convention
//! in `CLAUDE.md`, this module lives under `core/` because Phase C will
//! broaden the metric surface and other features (Phase B fitness
//! functions, slice-2 `dec metrics` CLI in FT-025) consume it through
//! this single public API — no sibling feature reaches into the
//! internals.
//!
//! The module is read-only. It executes SPARQL against the supplied
//! `oxigraph::store::Store`, counts in Rust (avoiding `xsd:double`
//! division surprises), and returns an [`AgreementReport`].

pub mod agreement;
pub mod queries;

#[cfg(test)]
mod tests;

pub use agreement::{agreement, format_report, AgreementReport, MetricsError};
