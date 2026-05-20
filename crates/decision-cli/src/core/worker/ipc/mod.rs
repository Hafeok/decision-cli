//! Worker → harness inter-process communication (FT-031).
//!
//! Workers run as subprocesses (see `core::worker::resolve`); they emit
//! structured records on stdout. The harness scans those records back
//! into typed Rust values before threading them through `StreamWriter`.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (slice-1 implementer harness, FT-032 dispatch interaction, FT-033 CLI)
//! imports from this module — no consumer reaches into a sibling feature
//! to parse worker output.

pub mod feedback;

pub use feedback::{
    apply, parse_records, FeedbackApplyError, FeedbackEmission, ParseRecordError,
    FEEDBACK_RECORD_SENTINEL,
};
