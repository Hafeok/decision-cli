//! In-memory store indexing telemetry records by `ddd_session_id` (FT-096).
//!
//! Slice-1 persistence: a process-local `HashMap<session_id, Vec<record>>`
//! protected by a `Mutex`. Persistent spend-tracking (DB-backed
//! `database_url` in LiteLLM's config) is the slice-2 progression per
//! FT-096's "Out of scope". The wire shape is stable so the slice-2
//! migration is a backend swap, not a payload change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;

use super::payload::TelemetryPayload;

/// Error variants surfaced by [`TelemetryStore`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelemetryStoreError {
    /// Lock poisoned by a panicking writer. Should not happen in slice
    /// 1 (the writer paths are infallible); surfaced so callers do not
    /// have to handle panics implicitly.
    #[error("telemetry store mutex poisoned")]
    Poisoned,
}

/// In-memory store keyed by `ddd_session_id`. Cloneable so axum state
/// can share one logical store across handler invocations.
#[derive(Debug, Default, Clone)]
pub struct TelemetryStore {
    inner: Arc<Mutex<HashMap<String, Vec<TelemetryPayload>>>>,
}

impl TelemetryStore {
    /// Construct a fresh empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record under its `ddd_session_id`. Order of arrival is
    /// preserved per session (slice 1 has no ordering guarantees across
    /// sessions because the store is process-local).
    pub fn record(&self, payload: TelemetryPayload) -> Result<(), TelemetryStoreError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| TelemetryStoreError::Poisoned)?;
        guard
            .entry(payload.ddd_session_id.clone())
            .or_default()
            .push(payload);
        Ok(())
    }

    /// Return a snapshot of all telemetry records for the given session
    /// id. Empty vector when no records exist (callers cannot
    /// distinguish "no calls yet" from "unknown session" at slice 1;
    /// session-existence checks are graph-side).
    pub fn for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<TelemetryPayload>, TelemetryStoreError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| TelemetryStoreError::Poisoned)?;
        Ok(guard.get(session_id).cloned().unwrap_or_default())
    }

    /// Total cost reconciled across all records for a session, in USD.
    /// LiteLLM's per-call cost figure is authoritative per ADR-064; this
    /// method sums those figures verbatim.
    pub fn total_cost_usd(&self, session_id: &str) -> Result<f64, TelemetryStoreError> {
        let records = self.for_session(session_id)?;
        Ok(records.iter().map(|r| r.cost_usd).sum())
    }

    /// Count of recorded calls across every session — debug surface for
    /// the `dec doctor`-style health check; not load-bearing.
    pub fn record_count(&self) -> Result<usize, TelemetryStoreError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| TelemetryStoreError::Poisoned)?;
        Ok(guard.values().map(Vec::len).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(session: &str, cost: f64) -> TelemetryPayload {
        TelemetryPayload {
            ddd_session_id: session.to_string(),
            model: "frontier-reasoning".to_string(),
            provider: "anthropic".to_string(),
            capability_tag: "frontier-reasoning".to_string(),
            input_tokens: 10,
            output_tokens: 20,
            cost_usd: cost,
            latency_ms: 250,
            retry_count: 0,
            fallback_chain: vec![],
        }
    }

    #[test]
    fn records_persist_under_session_id() {
        let store = TelemetryStore::new();
        store.record(fixture("sess-1", 0.01)).expect("record");
        store.record(fixture("sess-1", 0.02)).expect("record");
        store.record(fixture("sess-2", 0.03)).expect("record");

        let sess_1 = store.for_session("sess-1").expect("for_session");
        assert_eq!(sess_1.len(), 2);
        let sess_2 = store.for_session("sess-2").expect("for_session");
        assert_eq!(sess_2.len(), 1);
    }

    #[test]
    fn total_cost_sums_across_records() {
        let store = TelemetryStore::new();
        store.record(fixture("sess-1", 0.01)).expect("record");
        store.record(fixture("sess-1", 0.02)).expect("record");
        let total = store.total_cost_usd("sess-1").expect("total");
        assert!((total - 0.03).abs() < 1e-9);
    }

    #[test]
    fn unknown_session_returns_empty_vec() {
        let store = TelemetryStore::new();
        let records = store.for_session("missing").expect("for_session");
        assert!(records.is_empty());
        let total = store.total_cost_usd("missing").expect("total");
        assert!(total.abs() < 1e-9);
    }
}
