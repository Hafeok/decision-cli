//! Dispatcher escalation loop per FT-062 / ADR-034.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, downstream
//! consumers (dispatcher entry point, telemetry) import from this
//! module — no consumer imports from sibling features.
//!
//! Scope of FT-062 is the typed pure helpers (`collect_signals`,
//! `find_next_escalation_step`, `enrich_bundle_with_prior_attempt`)
//! plus the dispatcher `dispatch_role` entry point. Workers are
//! abstracted via the [`WorkerRunner`] trait so tests can inject
//! canned responses; production worker invocation lives in FT-060.

pub mod bundle_enrich;
pub mod loop_driver;
pub mod quads;
pub mod signals;
pub mod triggers;
pub mod types;

pub use bundle_enrich::{
    enrich_bundle_with_prior_attempt, render_enriched_bundle_markdown,
    render_prior_attempt_block, PROV_WAS_DERIVED_FROM, IRI_DEC_SUPERSEDES_BUNDLE,
};
pub use loop_driver::{dispatch_role, mint_session_iri, ChainResult, WorkerRunner};
pub use quads::{build_enriched_bundle_quads, build_session_linkage_quads};
pub use signals::collect_signals;
pub use triggers::{evaluate_trigger, find_next_escalation_step, pick_reason};
pub use types::{
    AttemptTokens, AuditOutcome, DispatchAttempt, EscalationError, FeedbackArtifact, SessionId,
    SignalSet, WorkerResult,
};
