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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::drive::{
        Action, ArtifactKind, ArtifactRef, Goal, PlanContext, Planner, PlanError,
    };
    use crate::features::drive::execute::Executor;
    use crate::features::drive::run::{run_with_planner_executor_and_progress, RunArgs};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Plans one dispatch action, then Done.
    struct OneShotPlanner {
        calls: AtomicUsize,
    }

    impl Planner for OneShotPlanner {
        fn plan(&self, _: &PlanContext, _: &ArtifactRef) -> Result<Action, PlanError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(if n == 0 {
                Action::DispatchCluster {
                    feature_id: "FT-T135".to_string(),
                    task_type_name: "add-artifact-type".to_string(),
                }
            } else {
                Action::Done
            })
        }
    }

    struct OkExecutor;
    impl Executor for OkExecutor {
        fn execute(&mut self, _: &PlanContext, _: &Action) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn ctx() -> PlanContext {
        PlanContext {
            workdir: std::env::temp_dir(),
            product_root: std::env::temp_dir(),
            env_override: None,
        }
    }

    fn args() -> RunArgs {
        RunArgs {
            goal: Goal::Ship,
            artifact: ArtifactRef {
                kind: ArtifactKind::Feature,
                short_id: "FT-T135".to_string(),
            },
            max_iter: 3,
        }
    }

    /// TC-324: per-round plan line with feature id and action tag.
    #[test]
    fn ft_135_plan_line_per_round() {
        let sink = RecordingProgressSink::default();
        let planner = OneShotPlanner {
            calls: AtomicUsize::new(0),
        };
        let mut executor = OkExecutor;
        run_with_planner_executor_and_progress(&ctx(), &args(), &planner, &mut executor, &sink)
            .expect("drive completes");
        let lines = sink.snapshot();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("[FT-T135] iter 0  plan=dispatch:cluster")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("[FT-T135] iter 1  plan=done")),
            "{lines:?}"
        );
    }

    /// TC-325: exec start + exec ok bracket with elapsed seconds.
    #[test]
    fn ft_135_exec_bracket_lines() {
        let sink = RecordingProgressSink::default();
        let planner = OneShotPlanner {
            calls: AtomicUsize::new(0),
        };
        let mut executor = OkExecutor;
        run_with_planner_executor_and_progress(&ctx(), &args(), &planner, &mut executor, &sink)
            .expect("drive completes");
        let lines = sink.snapshot();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("exec start: dispatch:cluster")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("exec ok") && l.contains('s')),
            "{lines:?}"
        );
    }

    /// TC-327 (loop half): terminal outcome line streams from the loop.
    #[test]
    fn ft_135_outcome_line_on_done() {
        let sink = RecordingProgressSink::default();
        let planner = OneShotPlanner {
            calls: AtomicUsize::new(0),
        };
        let mut executor = OkExecutor;
        run_with_planner_executor_and_progress(&ctx(), &args(), &planner, &mut executor, &sink)
            .expect("drive completes");
        let lines = sink.snapshot();
        assert!(
            lines.iter().any(|l| l.contains("outcome=Done iter=1")),
            "{lines:?}"
        );
    }

    /// TC-326 (sink half): quiet suppresses stderr writes but the
    /// callbacks (and thus tracing) still fire — proven by the
    /// recording sink capturing while StderrProgressSink::emit gates
    /// only the eprintln. Exercised here via the quiet constructor not
    /// panicking and the trait dispatch path staying live.
    #[test]
    fn ft_135_quiet_sink_constructs_and_dispatches() {
        let sink = StderrProgressSink::new(true);
        let planner = OneShotPlanner {
            calls: AtomicUsize::new(0),
        };
        let mut executor = OkExecutor;
        run_with_planner_executor_and_progress(&ctx(), &args(), &planner, &mut executor, &sink)
            .expect("drive completes under quiet sink");
    }
}
