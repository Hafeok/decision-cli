//! Server-Sent Events delivery transport (FT-003 § Behaviour step 3,
//! consumed by FT-004).
//!
//! `oxi-events` ships the axum router that exposes `/events` as an SSE
//! stream backed by the [`OutboxPublisher`](crate::outbox::OutboxPublisher)
//! broadcast bus. Each [`EventEnvelope`](crate::outbox::EventEnvelope) is
//! framed as a single SSE record with:
//!
//! * `id:` — the monotonic sequence number,
//! * `event:` — `dispatch`, and
//! * `data:` — the JSON-serialised envelope.
//!
//! The router lives here (rather than in a separate crate) because it
//! depends only on framework vocabulary — no DDD types — and ADR-001
//! permits it.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tracing::warn;

use crate::outbox::{EventEnvelope, OutboxPublisher};

/// Build an axum router exposing `/events` as an SSE stream.
///
/// The returned router takes ownership of an `Arc<OutboxPublisher>`;
/// the publisher's broadcast bus is subscribed on each incoming GET so
/// every client gets an independent copy of the fan-out.
pub fn router(publisher: Arc<OutboxPublisher>) -> Router {
    let state = SseState { publisher };
    Router::new()
        .route("/events", get(stream_events))
        .with_state(state)
}

#[derive(Clone)]
struct SseState {
    publisher: Arc<OutboxPublisher>,
}

async fn stream_events(
    State(state): State<SseState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.publisher.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(env) => Some(Ok(envelope_to_sse(&env))),
        Err(err) => {
            warn!(?err, "SSE: dropped broadcast item (subscriber lagging)");
            None
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

fn envelope_to_sse(env: &EventEnvelope) -> Event {
    let data = serde_json::to_string(env).unwrap_or_else(|err| {
        warn!(?err, "SSE: envelope serialise failed; emitting empty data");
        "{}".to_string()
    });
    Event::default()
        .id(env.seq.to_string())
        .event("dispatch")
        .data(data)
}
