//! Worker preflight — resolution chain, embedded manifest, report shape (FT-016).
//!
//! This module is the **single source of truth** for how `dec` finds a
//! worker for a given role. It is consumed by:
//!
//! - `dec init` — runs an advisory preflight after bootstrap.
//! - `dec doctor` — authoritative on-demand audit.
//! - `dec implement` — must call [`resolve`] before opening a session so
//!   missing workers are surfaced *before* any graph mutation (TC-049).
//!
//! Invariant TC-050 enforces structurally that no second copy of the
//! resolution chain lives anywhere else in `crates/decision-cli/src/`.

pub mod ipc;
pub mod manifest;
pub mod report;
pub mod resolve;

pub use ipc::feedback::{
    apply as apply_feedback_emission, parse_records as parse_feedback_records, FeedbackApplyError,
    FeedbackEmission, ParseRecordError, FEEDBACK_RECORD_SENTINEL,
};
pub use manifest::{
    manifest_sha256_hex, role_entry, ACTIVE_ROLES_ENGINEERING_DEVELOPMENT, MANIFEST, MANIFEST_RAW,
};
pub use report::{
    build_report, format_report_json, format_report_text, RoleStatus, WorkerReport, WorkerRow,
};
pub use resolve::{resolve, ResolveInputs, Resolution, ResolutionKind};
