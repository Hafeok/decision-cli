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

pub mod code_change_committed_dispatch;
pub mod feedback_resume;
pub mod graph_accepted_dispatch;
pub mod verifier_dispatch;
pub mod verify_graph_author_dispatch;

pub use code_change_committed_dispatch::{
    dispatch_for_code_change, AggregateOutcome as CodeChangeAggregateOutcome,
    CodeChangeCommittedDispatchConfig, CodeChangeDispatchError,
    HANDLER as CODE_CHANGE_COMMITTED_DISPATCH_HANDLER,
    SEED_TTL as CODE_CHANGE_COMMITTED_DISPATCH_SEED_TTL,
    SUBSCRIPTION_IRI as CODE_CHANGE_COMMITTED_DISPATCH_SUBSCRIPTION_IRI,
};
pub use feedback_resume::{
    handle_pending as handle_feedback_resume, FeedbackResumeError, HandledGroup,
    FEEDBACK_RESUME_HANDLER, FEEDBACK_RESUME_SEED_TTL, FEEDBACK_RESUME_SUBSCRIPTION_IRI,
};
pub use graph_accepted_dispatch::{
    dispatch_for_graph, DispatchOutcome as GraphAcceptedDispatchOutcome, DispatchedTuple,
    GraphAcceptedDispatchConfig, GraphAcceptedDispatchError,
    HANDLER as GRAPH_ACCEPTED_DISPATCH_HANDLER, SEED_TTL as GRAPH_ACCEPTED_DISPATCH_SEED_TTL,
    SUBSCRIPTION_IRI as GRAPH_ACCEPTED_DISPATCH_SUBSCRIPTION_IRI,
};
pub use verifier_dispatch::{
    already_dispatched, dispatch_pending_groups, emit_verifier_dispatch_event,
    VerifierDispatchError, VerifierDispatchEvent, VerifierDispatchSeed, VERIFIER_DISPATCH_HANDLER,
    VERIFIER_DISPATCH_SUBSCRIPTION_IRI,
};
pub use verify_graph_author_dispatch::{
    emit_dispatch_event as emit_verify_graph_author_dispatch_event, AutoDispatchConfig,
    AutoDispatchError, AutoDispatchSeed, VerifyGraphAuthorDispatchEvent,
    VERIFY_GRAPH_AUTHOR_DISPATCH_HANDLER, VERIFY_GRAPH_AUTHOR_DISPATCH_SEED_TTL,
    VERIFY_GRAPH_AUTHOR_DISPATCH_SUBSCRIPTION_IRI,
};
