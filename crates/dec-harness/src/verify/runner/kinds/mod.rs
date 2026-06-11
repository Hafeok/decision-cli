//! Step-kind handlers + dispatch table for the FT-098 runner.
//!
//! Each handler implements [`StepKindHandler::run`] over a typed
//! `VerificationStep`. The dispatch table in [`kind_table`] maps from
//! `StepKind` to the appropriate handler implementation.

mod capture;
mod common;
mod file;
mod http;
mod shell;
mod sparql;
mod wait_for;

/// Re-export a tiny helper for the top-level runner. The fn lives in
/// `kinds::common` so handler modules can use the abbreviated import
/// path, but the runner module also needs it for envelope timestamps.
#[must_use]
pub(super) fn iso_now_pub() -> String {
    common::iso_now()
}

use dec_graph::ontology::verification_graph::{StepKind, VerificationStep};
use dec_graph::ontology::verification_result::StepOutcome;

use super::context::RunContext;

/// Per-step runtime trace produced by a handler. The runner folds these
/// into a `VerificationStepTrace` for the persisted result.
#[derive(Debug, Clone)]
pub struct StepRunTrace {
    /// Two-tier outcome.
    pub outcome: StepOutcome,
    /// UTC ISO 8601 start timestamp.
    pub started_at: String,
    /// UTC ISO 8601 end timestamp.
    pub ended_at: String,
    /// Cap-4-KiB stdout excerpt (empty when not applicable).
    pub stdout_excerpt: String,
    /// Cap-4-KiB stderr excerpt (empty when not applicable).
    pub stderr_excerpt: String,
    /// Exit code (`shell-command`, `http-request`).
    pub exit_code: Option<i64>,
    /// Operator-facing one-liner — empty when outcome == pass.
    pub error_message: String,
    /// If `Some(true)`, the step has `dec:stopOnFail` set — the runner
    /// halts the loop after recording this trace.
    pub stop_on_fail: bool,
}

impl StepRunTrace {
    /// Pass trace constructor.
    pub(super) fn pass(started_at: String, ended_at: String) -> Self {
        Self {
            outcome: StepOutcome::Pass,
            started_at,
            ended_at,
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            exit_code: None,
            error_message: String::new(),
            stop_on_fail: false,
        }
    }

    /// Fail trace constructor.
    pub(super) fn fail(started_at: String, ended_at: String, error_message: String) -> Self {
        Self {
            outcome: StepOutcome::Fail,
            started_at,
            ended_at,
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            exit_code: None,
            error_message,
            stop_on_fail: false,
        }
    }

    /// Unrunnable trace constructor.
    pub(super) fn unrunnable(started_at: String, ended_at: String, error_message: String) -> Self {
        Self {
            outcome: StepOutcome::Unrunnable,
            started_at,
            ended_at,
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            exit_code: None,
            error_message,
            stop_on_fail: false,
        }
    }
}

/// Uniform interface every step-kind handler implements.
pub trait StepKindHandler: Send + Sync {
    /// Execute the step against the run context.
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace;
}

/// Dispatch a step to its kind handler. Adding a kind = adding one match
/// arm here and one module under `kinds/`.
pub(super) fn dispatch(step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
    match step.kind {
        StepKind::ShellCommand => shell::ShellHandler.run(step, ctx),
        StepKind::SparqlAssertion => sparql::SparqlHandler.run(step, ctx),
        StepKind::FileAssertion => file::FileHandler.run(step, ctx),
        StepKind::HttpRequest => http::HttpHandler.run(step, ctx),
        StepKind::WaitFor => wait_for::WaitForHandler.run(step, ctx),
        StepKind::Capture => capture::CaptureHandler.run(step, ctx),
    }
}

/// Re-export the six seed kind handler structs for callers that want to
/// register their own dispatch table (slice-3+ extension point).
pub use capture::CaptureHandler;
pub use file::FileHandler;
pub use http::HttpHandler;
pub use shell::ShellHandler;
pub use sparql::SparqlHandler;
pub use wait_for::WaitForHandler;
