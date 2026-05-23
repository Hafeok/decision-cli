//! DispatchGroup lifecycle substrate for action-interpretation pairing (ADR-017, FT-021).
//!
//! This module is the slice-level core extension that hosts the
//! `dec:DispatchGroup` Rust shape and its state machine. Per ADR-016,
//! `features/*` modules import these types via [`crate::core::dispatch`];
//! sibling features never reach into each other for the lifecycle
//! primitives that gate `dec implement` completion.

pub mod capability_resolver;
pub mod escalation;
pub mod group;
pub mod lifecycle;
pub mod pause;
pub mod quads;

pub use capability_resolver::{resolve_default_capability, ResolvedCapability, ResolverError};
pub use escalation::{
    build_enriched_bundle_quads, build_session_linkage_quads, collect_signals, dispatch_role,
    enrich_bundle_with_prior_attempt, evaluate_trigger, find_next_escalation_step,
    mint_session_iri, pick_reason, render_enriched_bundle_markdown, render_prior_attempt_block,
    AttemptTokens, AuditOutcome, ChainResult, DispatchAttempt, EscalationError, FeedbackArtifact,
    SessionId, SignalSet, WorkerResult, WorkerRunner,
};
pub use group::{DispatchGroup, GroupError};
pub use lifecycle::{DispatchEvent, DispatchStatus, LifecycleError};
pub use pause::{
    list_blocked_by, list_paused_groups_for_feedback, pause_on_feedback, resume_check, PauseError,
    ResumeError, ResumeOutcome,
};
pub use quads::{build_blocked_by_quad, build_group_creation_quads, build_status_transition_quads};
