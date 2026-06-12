//! FT-135: live progress feedback for `dec drive` — per-round planner
//! decisions, executor brackets, and per-feature outcomes on stderr,
//! mirrored to `tracing` (target `dec::drive::progress`).
//!
//! Stderr is the human surface; tracing is the machine surface. Stdout
//! (the terminal history dump and the `--all` summary table) is
//! untouched, so scripts consuming `dec drive` output are unaffected.

use std::sync::Mutex;

use crate::core::drive::Action;

/// Per-iteration progress callbacks threaded through the driver loop,
/// mirroring the `Executor` test seam.
pub trait ProgressSink {
    /// A planner decision, before execution begins.
    fn on_plan(&self, feature: &str, iter: usize, action: &Action);
    /// A non-terminal action is about to execute.
    fn on_exec_start(&self, feature: &str, iter: usize, tag: &str);
    /// The execution returned; `err` is `None` on success.
    fn on_exec_end(&self, feature: &str, iter: usize, tag: &str, elapsed_s: f64, err: Option<&str>);
    /// The drive terminated for this feature (sweeps stream this line
    /// before the next feature begins).
    fn on_outcome(&self, feature: &str, outcome: &str);
}

/// Variant-specific `key=value` trailers for the plan line.
fn action_trailer(action: &Action) -> String {
    match action {
        Action::Stuck { reason } => format!("  reason={:?}", reason),
        _ => String::new(),
    }
}

/// Production sink: single-line, tab-separated, `[FT-XXX]`-prefixed
/// stderr narration. `quiet` suppresses the stderr writes while the
/// tracing events keep firing.
pub struct StderrProgressSink {
    quiet: bool,
}

impl StderrProgressSink {
    /// Build the production sink; `quiet` comes from `--quiet`/`-q`.
    #[must_use]
    pub fn new(quiet: bool) -> Self {
        Self { quiet }
    }

    fn emit(&self, line: &str) {
        tracing::info!(target: "dec::drive::progress", "{line}");
        if !self.quiet {
            eprintln!("{line}");
        }
    }
}

impl ProgressSink for StderrProgressSink {
    fn on_plan(&self, feature: &str, iter: usize, action: &Action) {
        self.emit(&format!(
            "[{feature}] iter {iter}  plan={}{}",
            action.tag(),
            action_trailer(action)
        ));
    }

    fn on_exec_start(&self, feature: &str, iter: usize, tag: &str) {
        self.emit(&format!("[{feature}] iter {iter}  exec start: {tag}"));
    }

    fn on_exec_end(
        &self,
        feature: &str,
        iter: usize,
        tag: &str,
        elapsed_s: f64,
        err: Option<&str>,
    ) {
        match err {
            None => self.emit(&format!(
                "[{feature}] iter {iter}  exec ok    {elapsed_s:.2}s"
            )),
            Some(detail) => {
                let clean = detail.replace(['\t', '\n'], " ");
                self.emit(&format!(
                    "[{feature}] iter {iter}  exec fail  {elapsed_s:.2}s  err={clean:?}  ({tag})"
                ));
            }
        }
    }

    fn on_outcome(&self, feature: &str, outcome: &str) {
        self.emit(&format!("[{feature}]         outcome={outcome}"));
    }
}

/// Test sink: records every formatted line for assertions.
#[derive(Default)]
pub struct RecordingProgressSink {
    /// Captured lines, in emission order.
    pub lines: Mutex<Vec<String>>,
}

impl RecordingProgressSink {
    fn record(&self, line: String) {
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(line);
        }
    }

    /// Snapshot of the captured lines.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.lines.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

impl ProgressSink for RecordingProgressSink {
    fn on_plan(&self, feature: &str, iter: usize, action: &Action) {
        self.record(format!(
            "[{feature}] iter {iter}  plan={}{}",
            action.tag(),
            action_trailer(action)
        ));
    }

    fn on_exec_start(&self, feature: &str, iter: usize, tag: &str) {
        self.record(format!("[{feature}] iter {iter}  exec start: {tag}"));
    }

    fn on_exec_end(
        &self,
        feature: &str,
        iter: usize,
        tag: &str,
        elapsed_s: f64,
        err: Option<&str>,
    ) {
        match err {
            None => self.record(format!(
                "[{feature}] iter {iter}  exec ok    {elapsed_s:.2}s"
            )),
            Some(detail) => self.record(format!(
                "[{feature}] iter {iter}  exec fail  {elapsed_s:.2}s  err={detail:?}  ({tag})"
            )),
        }
    }

    fn on_outcome(&self, feature: &str, outcome: &str) {
        self.record(format!("[{feature}]         outcome={outcome}"));
    }
}

/// No-op sink for callers that genuinely want silence (post-hoc verbs).
pub struct NullProgressSink;

impl ProgressSink for NullProgressSink {
    fn on_plan(&self, _: &str, _: usize, _: &Action) {}
    fn on_exec_start(&self, _: &str, _: usize, _: &str) {}
    fn on_exec_end(&self, _: &str, _: usize, _: &str, _: f64, _: Option<&str>) {}
    fn on_outcome(&self, _: &str, _: &str) {}
}
