//! TC-011 — SSE dispatch event delivered within one second.
//!
//! Validates: FT-003 (event emission and outbox), FT-004 (SSE delivery).
//! Spec: `.product/tests/TC-011-sse-dispatch-event-delivered-within-one-second.md`
//!
//! Threads a real HTTP client (reqwest) through the axum SSE router so
//! the measured latency includes chunked framing and TCP delivery — not
//! just an in-process broadcast. For N ≥ 10 successive dispatches we
//! assert `t_recv − t_emit < 1.000 s`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use oxi_events::sse;
use oxi_events::{EventEnvelope, GraphWriter, Mutation, OutboxPublisher, Subscription};
use oxigraph::model::{GraphName, NamedNode, Quad};
use oxigraph::store::Store;
use tokio::net::TcpListener;
use tokio::time::timeout;

const N_DISPATCHES: usize = 10;
const LATENCY_BUDGET: Duration = Duration::from_secs(1);

#[tokio::test]
async fn sse_dispatch_event_delivered_within_one_second() {
    // ── server-side: store + writer + outbox + SSE router ────────────
    let store = Arc::new(Store::new().expect("in-memory store"));
    let writer = GraphWriter::open(Arc::clone(&store)).expect("writer opens");

    let sub_id = NamedNode::new("urn:oxi-events:test:sub:sse-dispatch").expect("sub iri");
    writer
        .register_subscription(&Subscription::new(sub_id, "ASK { ?s ?p ?o }".to_string()))
        .expect("register subscription");

    // Tight poll cadence so the publisher's run loop reacts faster than
    // its own notify path for safety.
    let publisher = Arc::new(OutboxPublisher::new(
        Arc::clone(&store),
        1024,
        Duration::from_millis(10),
    ));
    writer
        .attach_outbox(Arc::clone(&publisher))
        .expect("attach outbox");
    let outbox_handle = Arc::clone(&publisher).spawn();

    let app = sse::router(Arc::clone(&publisher));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind localhost");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // ── client-side: open SSE stream and wait until the server has seen
    // the subscription on the broadcast bus. The broadcast channel only
    // delivers to live receivers, so we must not race the first commit.
    let url = format!("http://{addr}/events");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("client");
    let response = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("connect to /events");
    assert!(response.status().is_success(), "SSE handshake must succeed");
    let mut byte_stream = response.bytes_stream();
    let mut parser = SseParser::new();

    // Wait until the server-side handler has registered its receiver on
    // the broadcast bus before firing mutations.
    let publisher_wait = Arc::clone(&publisher);
    timeout(Duration::from_secs(2), async move {
        loop {
            if publisher_wait.receiver_count() > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("SSE handler must register on the broadcast bus");

    // ── fire N dispatches, measuring per-event latency ───────────────
    let mut latencies = Vec::with_capacity(N_DISPATCHES);
    let mut received_seqs = Vec::with_capacity(N_DISPATCHES);

    for i in 0..N_DISPATCHES {
        let s = NamedNode::new(format!("urn:oxi-events:test:disp:{i}")).expect("s iri");
        let p = NamedNode::new("urn:oxi-events:test:dispatched").expect("p iri");
        let o = NamedNode::new(format!("urn:oxi-events:test:artifact:{i}")).expect("o iri");
        let quad = Quad::new(s, p, o, GraphName::DefaultGraph);
        let t_emit = Instant::now();
        let commit = writer
            .commit(Mutation::insert([quad]).with_cause("sse-dispatch"))
            .expect("commit succeeds");
        assert_eq!(
            commit.events.len(),
            1,
            "single ASK subscription → single event per dispatch"
        );

        // Pump bytes through the SSE parser until a `dispatch` event
        // surfaces. The bounded loop is the assertion mechanism.
        let env = read_next_dispatch(&mut byte_stream, &mut parser, LATENCY_BUDGET).await;
        let t_recv = Instant::now();
        let latency = t_recv.duration_since(t_emit);
        latencies.push(latency);
        received_seqs.push(env.seq);
        assert!(
            latency < LATENCY_BUDGET,
            "TC-011 budget violation at dispatch {i}: {latency:?} >= {LATENCY_BUDGET:?}"
        );
    }

    // ── completeness + seq continuity ─────────────────────────────────
    assert_eq!(
        received_seqs.len(),
        N_DISPATCHES,
        "every emitted event must arrive over SSE (no loss)"
    );
    for window in received_seqs.windows(2) {
        assert!(
            window[1] > window[0],
            "SSE delivers events in strictly increasing seq order: {received_seqs:?}"
        );
    }

    eprintln!(
        "TC-011 latencies (ms): {:?}",
        latencies
            .iter()
            .map(std::time::Duration::as_millis)
            .collect::<Vec<_>>()
    );

    outbox_handle.shutdown().await;
}

async fn read_next_dispatch<S, B>(
    stream: &mut S,
    parser: &mut SseParser,
    budget: Duration,
) -> EventEnvelope
where
    S: futures_util::Stream<Item = reqwest::Result<B>> + Unpin,
    B: AsRef<[u8]>,
{
    let deadline = Instant::now() + budget;
    loop {
        if let Some(env) = parser.next_dispatch() {
            return env;
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO);
        let chunk = timeout(remaining, stream.next())
            .await
            .expect("SSE chunk must arrive within latency budget")
            .expect("SSE stream not closed")
            .expect("SSE chunk read OK");
        parser.feed(chunk.as_ref());
    }
}

/// Minimal incremental SSE parser. Handles `id:`, `event:`, `data:` and
/// the blank-line frame terminator; folds multi-line `data:` payloads.
struct SseParser {
    buf: String,
    queued: std::collections::VecDeque<EventEnvelope>,
    cur_event: Option<String>,
    cur_data: String,
    cur_id: Option<String>,
}

impl SseParser {
    fn new() -> Self {
        Self {
            buf: String::new(),
            queued: std::collections::VecDeque::new(),
            cur_event: None,
            cur_data: String::new(),
            cur_id: None,
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        // SSE is required to be UTF-8 per the spec.
        if let Ok(s) = std::str::from_utf8(bytes) {
            self.buf.push_str(s);
        }
        while let Some(idx) = self.buf.find('\n') {
            let mut line = self.buf[..idx].to_string();
            self.buf.drain(..=idx);
            if line.ends_with('\r') {
                line.pop();
            }
            self.process_line(&line);
        }
    }

    fn process_line(&mut self, line: &str) {
        if line.is_empty() {
            self.flush_event();
            return;
        }
        if line.starts_with(':') {
            // SSE comment / keepalive — ignore.
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.cur_event = Some(value.to_string()),
            "id" => self.cur_id = Some(value.to_string()),
            "data" => {
                if !self.cur_data.is_empty() {
                    self.cur_data.push('\n');
                }
                self.cur_data.push_str(value);
            }
            _ => {}
        }
    }

    fn flush_event(&mut self) {
        let event = self.cur_event.take();
        let data = std::mem::take(&mut self.cur_data);
        let _id = self.cur_id.take();
        if data.is_empty() {
            return;
        }
        if event.as_deref() == Some("dispatch") {
            match serde_json::from_str::<EventEnvelope>(&data) {
                Ok(env) => self.queued.push_back(env),
                Err(err) => panic!("malformed SSE dispatch payload: {err}; raw: {data}"),
            }
        }
    }

    fn next_dispatch(&mut self) -> Option<EventEnvelope> {
        self.queued.pop_front()
    }
}
