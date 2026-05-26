//! `wait-for` step handler (FT-098 §Phase 3.2).
//!
//! Polls a wrapped step (identified by `dec:condition` IRI; looked up
//! against the parent graph's step table) until it passes or
//! `dec:timeout` elapses. The wrapped step's last trace is folded into
//! the wait-for step's trace.

use std::thread;
use std::time::{Duration, Instant};

use crate::core::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::RunContext;
use super::common::iso_now;
use super::{dispatch, StepKindHandler, StepRunTrace};

const DEFAULT_POLL: Duration = Duration::from_secs(1);

/// `wait-for` handler.
pub struct WaitForHandler;

impl StepKindHandler for WaitForHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::WaitFor { condition, timeout } = &step.fields else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "wait-for handler received non-wait fields".into(),
            );
        };
        let timeout = parse_iso_duration(timeout).unwrap_or_else(|| Duration::from_secs(30));
        let wrapped = match ctx.step_lookup.get(condition.as_str()) {
            Some(w) => w.clone(),
            None => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!(
                        "wait-for condition <{c}> not found in parent graph's steps",
                        c = condition.as_str()
                    ),
                );
            }
        };
        let deadline = Instant::now() + timeout;
        loop {
            let inner = dispatch(&wrapped, ctx);
            if matches!(
                inner.outcome,
                crate::core::ontology::verification_result::StepOutcome::Pass
            ) {
                let ended = iso_now();
                return StepRunTrace::pass(started, ended);
            }
            if Instant::now() >= deadline {
                let ended = iso_now();
                return StepRunTrace::fail(
                    started,
                    ended,
                    format!(
                        "wait-for timed out after {secs}s; last error: {msg}",
                        secs = timeout.as_secs(),
                        msg = inner.error_message
                    ),
                );
            }
            thread::sleep(DEFAULT_POLL);
        }
    }
}

/// Parse a subset of ISO 8601 duration strings sufficient for FT-098:
/// `PTnS`, `PTnM`, `PTnH`. Returns `None` for inputs outside this
/// grammar; the caller falls back to a 30 s default.
fn parse_iso_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let rest = s.strip_prefix("PT")?;
    let last = rest.chars().last()?;
    let n_str = &rest[..rest.len() - last.len_utf8()];
    let n: u64 = n_str.parse().ok()?;
    let secs = match last {
        'S' => n,
        'M' => n.checked_mul(60)?,
        'H' => n.checked_mul(3600)?,
        _ => return None,
    };
    Some(Duration::from_secs(secs))
}
