//! `POST /llm-call-telemetry` reconciliation endpoint on pipeline-cli (FT-096).
//!
//! The LiteLLM proxy ships a custom callback (`pipeline-cli-telemetry`,
//! see `workers/litellm-telemetry-callback/`) that POSTs every LLM call's
//! telemetry — tokens, latency, cost, model, provider, fallback chain,
//! retry count — to this endpoint. LiteLLM is authoritative for spend
//! per ADR-064; the orchestration graph reconciles by indexing each
//! record under its `ddd_session_id` so session records can resolve the
//! authoritative cost figure at query time.
//!
//! Slice-1 shape per FT-096:
//!
//! - Bearer-token auth against an in-memory operator-known secret. Keys
//!   are rotated by editing the operator's `workers.env`; no online
//!   rotation.
//! - Single-tenant. Per-tenant scoping lands with
//!   `feature:multi-tenant-litellm` (slice 3+).
//! - In-memory store. Persistence is a slice-2 progression (the
//!   spend-tracking DB called out in FT-096's "Out of scope").

pub mod handler;
pub mod payload;
pub mod store;

pub use handler::{
    router, AppState, TelemetryAccepted, TelemetryRejection, TelemetryService,
    TelemetryServiceError,
};
pub use payload::TelemetryPayload;
pub use store::{TelemetryStore, TelemetryStoreError};
