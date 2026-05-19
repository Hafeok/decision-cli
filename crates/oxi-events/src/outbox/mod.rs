//! Outbox publisher — graph-native, crash-safe at-least-once event delivery.
//!
//! Per **FT-003** the outbox flag (`oxi:published`) plus a background
//! publisher gives crash-safe delivery: events land in the events graph
//! with `published = false`; the publisher fans them out through the
//! configured transports; on confirmed send it flips `published = true`.
//! On startup (e.g. after a SIGKILL) the publisher's first scan picks up
//! anything that was in flight before the crash (TC-010).
//!
//! The publisher does **not** know about SSE — it owns a
//! [`tokio::sync::broadcast::Sender`] of [`EventEnvelope`]s and any
//! number of consumers (SSE router, an in-process worker, a test
//! harness) subscribe to receive a fan-out copy.

mod store_ops;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;
use tracing::{debug, trace, warn};

use crate::error::WriterError;

use store_ops::{mark_published, scan_unpublished};

/// Default channel capacity for the in-process broadcast bus.
pub const DEFAULT_OUTBOX_CAPACITY: usize = 1024;

/// Default poll interval for the background sweep loop. Short enough to
/// keep the slice-1 SSE delivery budget (TC-011: < 1 s) comfortable, and
/// long enough not to hammer SPARQL between commits.
pub const DEFAULT_OUTBOX_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Serialisable envelope handed to downstream consumers (broadcast bus,
/// SSE clients). The envelope intentionally carries only the framework
/// fields — payload shape is up to consumers (ADR-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// IRI of the persisted `oxi:Event` artifact.
    pub event: String,
    /// Monotonic sequence number minted at commit time.
    pub seq: u64,
    /// IRI of the triggering mutation (`prov:wasGeneratedBy`).
    pub mutation: String,
    /// IRI of the subscription that produced this event.
    pub subscription: String,
    /// RFC-3339 emission timestamp.
    #[serde(rename = "emittedAt")]
    pub emitted_at: String,
}

/// Crash-safe outbox publisher (FT-003).
///
/// The publisher is constructed in two stages so callers can `subscribe`
/// before the background loop starts (avoiding the broadcast-channel
/// race where messages emitted between channel creation and the first
/// receiver are dropped).
pub struct OutboxPublisher {
    store: Arc<Store>,
    sender: broadcast::Sender<EventEnvelope>,
    notify: Arc<Notify>,
    poll_interval: Duration,
}

impl OutboxPublisher {
    /// Build a publisher over `store` with the given broadcast capacity
    /// and poll interval.
    #[must_use]
    pub fn new(store: Arc<Store>, capacity: usize, poll_interval: Duration) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            store,
            sender,
            notify: Arc::new(Notify::new()),
            poll_interval,
        }
    }

    /// Build a publisher with [`DEFAULT_OUTBOX_CAPACITY`] and
    /// [`DEFAULT_OUTBOX_POLL_INTERVAL`].
    #[must_use]
    pub fn with_defaults(store: Arc<Store>) -> Self {
        Self::new(store, DEFAULT_OUTBOX_CAPACITY, DEFAULT_OUTBOX_POLL_INTERVAL)
    }

    /// Subscribe to the in-process broadcast bus.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.sender.subscribe()
    }

    /// Borrow the underlying broadcast sender (used by transports that
    /// want to call `.subscribe()` themselves, e.g. the SSE router).
    #[must_use]
    pub fn sender(&self) -> broadcast::Sender<EventEnvelope> {
        self.sender.clone()
    }

    /// Current number of live receivers on the broadcast bus.
    #[must_use]
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Poke the background loop to sweep immediately (e.g. right after a
    /// commit minted new events).
    pub fn notify(&self) {
        self.notify.notify_one();
    }

    /// Borrow the store this publisher operates against.
    #[must_use]
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// Run a single sweep: scan for unpublished events, fan them out to
    /// the broadcast bus, and flip the `oxi:published` flag for those
    /// that survived the broadcast send.
    ///
    /// Returns the number of events processed. Broadcast send with **no
    /// receivers** is NOT treated as an error (FT-003 §Error handling) —
    /// the in-process bus is best-effort fan-out.
    pub fn run_once(&self) -> Result<usize, WriterError> {
        let pending = scan_unpublished(&self.store)?;
        let mut delivered = 0_usize;
        for envelope in pending {
            if let Ok(receivers) = self.sender.send(envelope.clone()) {
                trace!(
                    event = %envelope.event,
                    seq = envelope.seq,
                    receivers,
                    "outbox: dispatched event to broadcast bus"
                );
            } else {
                trace!(
                    event = %envelope.event,
                    seq = envelope.seq,
                    "outbox: no receivers on broadcast bus (slice 1 best-effort)"
                );
            }
            mark_published(&self.store, &envelope.event)?;
            delivered += 1;
        }
        if delivered > 0 {
            debug!(delivered, "outbox sweep complete");
        }
        Ok(delivered)
    }

    /// Start a background sweep loop on the current tokio runtime. The
    /// loop runs until the returned [`OutboxHandle::shutdown`] is awaited
    /// (or the runtime exits).
    #[must_use]
    pub fn spawn(self: Arc<Self>) -> OutboxHandle {
        let shutdown_signal = Arc::new(Notify::new());
        let shutdown_for_task = shutdown_signal.clone();
        let publisher = self.clone();
        let handle = tokio::spawn(async move {
            publisher.run_loop(shutdown_for_task).await;
        });
        OutboxHandle {
            shutdown: shutdown_signal,
            join: Some(handle),
        }
    }

    async fn run_loop(self: Arc<Self>, shutdown: Arc<Notify>) {
        if let Err(err) = self.run_once() {
            warn!(?err, "outbox: initial sweep failed");
        }

        loop {
            tokio::select! {
                () = shutdown.notified() => {
                    debug!("outbox: shutdown signalled, exiting run loop");
                    return;
                }
                () = self.notify.notified() => {}
                () = tokio::time::sleep(self.poll_interval) => {}
            }
            if let Err(err) = self.run_once() {
                warn!(
                    ?err,
                    "outbox sweep failed; events stay unpublished for next tick"
                );
            }
        }
    }
}

/// Handle to a running [`OutboxPublisher::spawn`] background task.
///
/// Dropping the handle without calling [`Self::shutdown`] detaches the
/// loop; the loop will continue until the runtime stops.
pub struct OutboxHandle {
    shutdown: Arc<Notify>,
    join: Option<JoinHandle<()>>,
}

impl OutboxHandle {
    /// Signal the loop and await its exit.
    pub async fn shutdown(mut self) {
        self.shutdown.notify_waiters();
        if let Some(handle) = self.join.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for OutboxHandle {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}
