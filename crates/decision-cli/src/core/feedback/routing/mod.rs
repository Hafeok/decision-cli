//! Feedback routing subscription + handler (FT-029 / ADR-026).
//!
//! Two responsibilities live here:
//!
//! 1. The routing table itself (`table`) — Rust constants mirroring
//!    ADR-026 §Routing table. Phase C will lift these rows into
//!    graph-resident `dec:RoutingRule` triples; the table here is the
//!    pre-graph source of truth.
//!
//! 2. The delivery handler (`handler`) — the small piece of logic that
//!    turns each row of the seed subscription's SELECT into a
//!    `produced → routed` transition (or a `rejected` marker for
//!    unknown / disallowed override targets).
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-033 CLI surface, FT-031 worker SDK defaults, FT-032 dispatch
//! pause semantics) imports from this module — no consumer imports from
//! a sibling feature.

pub mod handler;
pub mod seed;
pub mod table;

pub use handler::{
    pending_feedback, route_one, route_pending_feedback, routing_table_len, FeedbackRoutingError,
    PendingFeedback, RoutingOutcome, FEEDBACK_ROUTING_HANDLER, FEEDBACK_ROUTING_SEED_TTL,
    FEEDBACK_ROUTING_SUBSCRIPTION_IRI, REJECTION_OVERRIDE_NOT_PERMITTED,
    REJECTION_UNKNOWN_TARGET_ROLE,
};
pub use seed::seed_quads;
pub use table::{
    default_target_role, override_permitted_for, role_may_address, rule_for, OverrideActor,
    RoutingRule, ROUTING_TABLE,
};
