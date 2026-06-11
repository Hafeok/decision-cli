//! # dec-harness — the decision-cli orchestration machinery (ADR-086)
//!
//! The dispatch loop and dispatch sessions, drive planners (ship,
//! def-ready, readiness orchestrator), the worker contract and worker
//! resolution chain, subscriptions, the role catalog, capability
//! resolution and escalation, and the verification orchestration
//! (matcher, coverage, runner, chain integrity).
//!
//! The crate composes the stable layers beneath it — [`dec_graph`] for
//! store access and [`dec_ontology`] for the domain — and is itself
//! composed by the `decision-cli` binary, which keeps the clap trees,
//! the MCP shim, and the vertical feature slices (ADR-016). The harness
//! is invocable from any front end: nothing in here parses CLI args.

#![deny(clippy::unwrap_used)]

pub mod bootstrap;
pub mod cosign_trust;
pub mod dispatch;
pub mod dispatch_session;
pub mod drive;
pub mod feedback;
pub mod handler;
pub mod identity_verifier;
pub mod metrics;
pub mod oci_manifest;
pub mod role_catalog;
pub mod subscriptions;
pub mod task_type;
pub mod verify;
pub mod worker;
pub mod worker_curator;
pub mod worker_manifest;
