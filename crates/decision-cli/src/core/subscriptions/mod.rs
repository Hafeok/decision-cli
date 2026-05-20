//! Slice-level subscription substrate for decision-cli (FT-022 / ADR-017).
//!
//! Lives under `core/` per the slice-level SDP convention: subscriptions
//! are platform substrate consumed by every feature module that mints a
//! `dec:DispatchGroup`, not feature-volatile code.
//!
//! This module owns:
//!
//! * the embedded Turtle seed for the verifier-dispatch subscription
//!   (FT-022 §Behaviour step 1), and
//! * the in-process delivery handler that detects a paired
//!   `dec:DispatchGroup` in `awaiting-interpretation` with no
//!   interpretation session yet attached and emits the
//!   `dec:VerifierDispatchEvent` consumed by the verifier worker
//!   (FT-022 §Behaviour step 3, ADR-017).
//!
//! The seed is installed alongside the v0 bootstrap subscriptions in
//! `features/init` (FT-009 §Behaviour step 4 — bootstrap-subscription
//! pattern). The handler is invoked by the action-side feature module
//! (slice 2: `features/implement`) once the `DispatchGroup` reaches
//! `awaiting-interpretation`; the dispatch event lands in the
//! orchestration store via [`crate::StreamWriter`] so the `oxi-events`
//! outbox (FT-003) and SSE transport (FT-004) deliver it to the verifier
//! worker.

pub mod feedback_resume;
pub mod verifier_dispatch;

pub use feedback_resume::{
    handle_pending as handle_feedback_resume, FeedbackResumeError, HandledGroup,
    FEEDBACK_RESUME_HANDLER, FEEDBACK_RESUME_SEED_TTL, FEEDBACK_RESUME_SUBSCRIPTION_IRI,
};
pub use verifier_dispatch::{
    already_dispatched, dispatch_pending_groups, emit_verifier_dispatch_event,
    VerifierDispatchError, VerifierDispatchEvent, VerifierDispatchSeed,
    VERIFIER_DISPATCH_HANDLER, VERIFIER_DISPATCH_SUBSCRIPTION_IRI,
};
