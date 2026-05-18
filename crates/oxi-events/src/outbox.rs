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

use std::sync::Arc;
use std::time::Duration;

use oxigraph::model::{Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;
use tracing::{debug, trace, warn};

use crate::error::WriterError;
use crate::vocab::{
    events_graph, IRI_OXI_EMITTED_AT, IRI_OXI_EVENT, IRI_OXI_MATCHED_SUBSCRIPTION,
    IRI_OXI_PUBLISHED, IRI_OXI_SEQ, IRI_PROV_WAS_GENERATED_BY,
};

const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

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
            // broadcast::Sender::send returns Err only when there are no
            // receivers. Slice 1 treats this as "delivered to nobody" —
            // we still mark the event published so the outbox does not
            // re-emit it forever. SSE clients arriving later can replay
            // via FT-005.
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
        // Initial sweep handles the crash-recovery path (TC-010): any
        // event marked `published = false` from a prior process gets
        // picked up before the loop settles into its tick cadence.
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
        // Best-effort: nudge the loop so it can notice shutdown if a
        // caller dropped the handle without awaiting.
        self.shutdown.notify_waiters();
    }
}

fn scan_unpublished(store: &Store) -> Result<Vec<EventEnvelope>, WriterError> {
    let q = format!(
        "SELECT ?e ?seq ?mut ?sub ?ts FROM <{events}> WHERE {{ \
            ?e a <{event}> ; \
               <{published}> false ; \
               <{seq}> ?seq ; \
               <{wgb}> ?mut ; \
               <{matched}> ?sub ; \
               <{emitted}> ?ts \
         }} ORDER BY ?seq",
        events = crate::vocab::IRI_OXI_GRAPH_EVENTS,
        event = IRI_OXI_EVENT,
        published = IRI_OXI_PUBLISHED,
        seq = IRI_OXI_SEQ,
        wgb = IRI_PROV_WAS_GENERATED_BY,
        matched = IRI_OXI_MATCHED_SUBSCRIPTION,
        emitted = IRI_OXI_EMITTED_AT,
    );
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        let Some(Term::NamedNode(event)) = sol.get("e").cloned() else {
            continue;
        };
        let Some(Term::Literal(seq_lit)) = sol.get("seq").cloned() else {
            continue;
        };
        let Some(Term::NamedNode(mutation)) = sol.get("mut").cloned() else {
            continue;
        };
        let Some(Term::NamedNode(subscription)) = sol.get("sub").cloned() else {
            continue;
        };
        let Some(Term::Literal(ts_lit)) = sol.get("ts").cloned() else {
            continue;
        };
        let seq: u64 = seq_lit
            .value()
            .parse()
            .map_err(|e| WriterError::Internal(format!("outbox: malformed seq literal: {e}")))?;
        out.push(EventEnvelope {
            event: event.as_str().to_string(),
            seq,
            mutation: mutation.as_str().to_string(),
            subscription: subscription.as_str().to_string(),
            emitted_at: ts_lit.value().to_string(),
        });
    }
    Ok(out)
}

fn mark_published(store: &Store, event_iri: &str) -> Result<(), WriterError> {
    let event = NamedNode::new(event_iri)
        .map_err(|e| WriterError::Internal(format!("outbox: invalid event IRI: {e}")))?;
    let g = events_graph();
    let pred = NamedNodeRef::new_unchecked(IRI_OXI_PUBLISHED);
    let xsd_bool = NamedNodeRef::new_unchecked(XSD_BOOLEAN);
    let old_quad = Quad::new(
        event.clone(),
        pred.into_owned(),
        Literal::new_typed_literal("false", xsd_bool.into_owned()),
        g.into_owned(),
    );
    let new_quad = Quad::new(
        event,
        pred.into_owned(),
        Literal::new_typed_literal("true", xsd_bool.into_owned()),
        g.into_owned(),
    );
    store.transaction(|mut tx| {
        tx.remove(old_quad.as_ref())?;
        tx.insert(new_quad.as_ref())?;
        Ok::<_, WriterError>(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocab::{events_graph, IRI_OXI_GRAPH_EVENTS};
    use oxigraph::model::{GraphName, NamedNode, Quad};

    fn insert_fake_event(store: &Store, iri: &str, seq: u64) {
        let g: GraphName = events_graph().into_owned().into();
        let event = NamedNode::new(iri).expect("iri");
        let mutation = NamedNode::new("urn:test:mutation:1").expect("mut iri");
        let subscription = NamedNode::new("urn:test:sub:1").expect("sub iri");
        let xsd_int = NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");
        let xsd_bool = NamedNodeRef::new_unchecked(XSD_BOOLEAN);
        let xsd_dt = NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime");
        let triples = vec![
            Quad::new(
                event.clone(),
                NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
                    .into_owned(),
                NamedNodeRef::new_unchecked(IRI_OXI_EVENT).into_owned(),
                g.clone(),
            ),
            Quad::new(
                event.clone(),
                NamedNodeRef::new_unchecked(IRI_OXI_SEQ).into_owned(),
                Literal::new_typed_literal(seq.to_string(), xsd_int.into_owned()),
                g.clone(),
            ),
            Quad::new(
                event.clone(),
                NamedNodeRef::new_unchecked(IRI_PROV_WAS_GENERATED_BY).into_owned(),
                mutation,
                g.clone(),
            ),
            Quad::new(
                event.clone(),
                NamedNodeRef::new_unchecked(IRI_OXI_MATCHED_SUBSCRIPTION).into_owned(),
                subscription,
                g.clone(),
            ),
            Quad::new(
                event.clone(),
                NamedNodeRef::new_unchecked(IRI_OXI_EMITTED_AT).into_owned(),
                Literal::new_typed_literal("2026-05-18T00:00:00Z", xsd_dt.into_owned()),
                g.clone(),
            ),
            Quad::new(
                event,
                NamedNodeRef::new_unchecked(IRI_OXI_PUBLISHED).into_owned(),
                Literal::new_typed_literal("false", xsd_bool.into_owned()),
                g,
            ),
        ];
        store
            .transaction(|mut tx| {
                for q in &triples {
                    tx.insert(q.as_ref())?;
                }
                Ok::<_, WriterError>(())
            })
            .expect("seed event");
        // Sanity touch on the events graph
        let _ = IRI_OXI_GRAPH_EVENTS;
    }

    #[test]
    fn run_once_marks_events_published() {
        let store = Arc::new(Store::new().expect("store"));
        insert_fake_event(&store, "urn:test:event:1", 2);
        let publisher = OutboxPublisher::with_defaults(Arc::clone(&store));
        let mut rx = publisher.subscribe();
        let n = publisher.run_once().expect("run_once");
        assert_eq!(n, 1, "exactly one event should be processed");
        let env = rx.try_recv().expect("envelope delivered");
        assert_eq!(env.seq, 2);
        // Re-running with no new unpublished events is a no-op.
        let again = publisher.run_once().expect("idempotent");
        assert_eq!(again, 0);
    }

    #[test]
    fn run_once_succeeds_with_no_receivers() {
        let store = Arc::new(Store::new().expect("store"));
        insert_fake_event(&store, "urn:test:event:2", 2);
        let publisher = OutboxPublisher::with_defaults(Arc::clone(&store));
        // No subscribers — must still publish without erroring.
        let n = publisher.run_once().expect("run_once");
        assert_eq!(n, 1);
    }
}
