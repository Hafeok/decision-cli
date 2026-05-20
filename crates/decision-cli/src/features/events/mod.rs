//! `dec events` subcommands — event inspection over replay or live tail.
//!
//! Slice 1 surface for FT-012:
//!
//! * [`since`] reads the persisted orchestration store and runs
//!   [`oxi_events::replay`] (FT-005). The output is a one-line-per-event
//!   summary suitable for piping into `jq`-style tooling. Output is
//!   read-only and deterministic against an unchanged store.
//!
//! * [`tail`] connects to the FT-004 SSE endpoint of a running `dec`
//!   daemon (or any compatible publisher) and prints each delivered
//!   envelope. Slice 1 binds SSE to localhost only; the default URL is
//!   `http://127.0.0.1:7878/events` and is overridable via the
//!   `DEC_EVENTS_URL` environment variable. Connection failures surface
//!   with a clear hint rather than a backtrace.
//!
//! These commands are read-only and refuse cleanly when run outside a
//! `.dec/` working tree (ADR-012 / FT-012 invariants).

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxi_events::{replay, ReplayRequest, ReplayedEvent};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

/// Default URL the SSE tail attaches to when neither the CLI flag nor
/// `DEC_EVENTS_URL` is set. Matches the slice-1 localhost binding noted
/// in TC-011.
pub const DEFAULT_EVENTS_URL: &str = "http://127.0.0.1:7878/events";

/// Replay events with `oxi:seq >= since_seq` from the persisted store.
///
/// Returns the replayed events in seq-ascending order. The caller is
/// responsible for rendering — keeping the function pure makes it easy
/// to unit-test and reuse from non-CLI contexts.
pub fn since(workdir: &Path, since_seq: u64, limit: Option<usize>) -> Result<Vec<ReplayedEvent>> {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    if !dump_path.exists() {
        return Err(anyhow!(
            "no orchestration store at {} — run `dec init` first",
            dump_path.display()
        ));
    }
    let bytes =
        std::fs::read(&dump_path).with_context(|| format!("reading {}", dump_path.display()))?;
    let store = Store::new().context("opening in-memory orchestration store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;

    let mut req = ReplayRequest::since(since_seq);
    if let Some(l) = limit {
        req = req.with_limit(l);
    }
    let events = replay(&store, &req).context("replaying events from the orchestration store")?;
    Ok(events)
}

/// Render a single replayed event as one line of human-readable text.
///
/// Format: `seq=<n> at=<rfc3339> status=<ok|failed> sub=<sub> ev=<iri> mut=<mut>`
#[must_use]
pub fn format_event_line(e: &ReplayedEvent) -> String {
    format!(
        "seq={:>6} at={} status={} sub={} ev={} mut={}",
        e.seq, e.emitted_at, e.status, e.subscription, e.event, e.mutation
    )
}

/// Tail the SSE endpoint, printing each delivered SSE record line as it
/// arrives. Returns `Ok(())` only when the server closes the stream
/// cleanly; client interruption (`SIGINT`) is the expected exit path
/// and is converted to `Ok(())` by the caller.
///
/// This function runs a small tokio runtime internally so the rest of
/// `dec` can stay synchronous. The connection is plain HTTP — slice 1
/// binds the publisher to localhost; TLS is out of scope.
pub fn tail(url: &str) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime for SSE tail")?;
    rt.block_on(tail_async(url))
}

async fn tail_async(url: &str) -> Result<()> {
    use std::io::Write as _;

    let client = reqwest::Client::builder()
        .build()
        .context("building HTTP client")?;
    let mut resp = client
        .get(url)
        .header("accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| {
            anyhow!(
                "could not connect to {url}: {e}. Is the dec daemon running? \
             (Override the URL with --url or `DEC_EVENTS_URL`.)"
            )
        })?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "SSE endpoint {url} returned HTTP {}",
            resp.status()
        ));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    // SSE records are line-delimited and separated by a blank line. The
    // dec publisher emits each envelope as a single record per
    // `oxi-events::sse`; we relay verbatim so consumers can parse the
    // raw protocol if they prefer. `Response::chunk` reads one TCP
    // chunk at a time without requiring the `stream` cargo feature.
    while let Some(chunk) = resp.chunk().await.context("reading SSE chunk")? {
        handle.write_all(&chunk).context("writing SSE chunk")?;
        handle.flush().context("flushing SSE chunk")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_errors_when_workdir_uninitialized() {
        // Pick a deterministic non-existent path so the test never
        // accidentally hits a real store.
        let base = std::env::temp_dir().join(format!("dec-events-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let res = since(&base, 0, None);
        assert!(res.is_err(), "expected error for uninitialized workdir");
    }

    #[test]
    fn default_events_url_is_localhost() {
        assert!(
            DEFAULT_EVENTS_URL.starts_with("http://127.0.0.1"),
            "slice 1 binds SSE to localhost only"
        );
    }
}
