//! Subprocess invocation of the verify-graph-author worker (ADR-008 / ADR-030).
//!
//! Mirrors the `features/implement/worker.rs` plumbing: serialise the
//! `VerifyGraphAuthorInput` to stdin, spawn the Python worker, parse the
//! `GraphProposal` JSON line on stdout. Errors surface as
//! [`HandlerError::Internal`] with prefix `worker:` so test fixtures can
//! grep them out.
//!
//! Worker discovery is delegated to [`crate::core::worker::resolve`]
//! (FT-016 / FT-067 / TC-050) — this module owns only the spawn / stdin
//! / stdout-parse plumbing. No inline `python3 -m verify_graph_author`
//! literal survives here; that path is one branch of the shared chain.
//!
//! A thread-local override hook lets tests inject a deterministic
//! `GraphProposal` without ever spawning a subprocess (TC-080 explicitly
//! requires the subprocess NOT be spawned on the match path; TC-079 /
//! TC-081 use the hook to drive `New` proposals deterministically).

use std::cell::RefCell;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use crate::core::handler::Error as HandlerError;
use crate::core::worker::{resolve, role_entry, Resolution, ResolveInputs};

use super::bundle::VerifyGraphAuthorInputJson;
use super::proposal::GraphProposal;

/// Manifest role id for the verify-graph-author worker (matches the
/// `[[worker]]` entry in `core/worker/assets/manifest.toml`).
const VERIFY_GRAPH_AUTHOR_ROLE: &str = "verify-graph-author";

type MockFn = Box<dyn Fn(&VerifyGraphAuthorInputJson) -> Result<GraphProposal, HandlerError>>;

thread_local! {
    static MOCK_WORKER: RefCell<Option<MockFn>> = const { RefCell::new(None) };
}

/// Global counter of real subprocess invocations. Tests assert this
/// stays at zero on the `Match` path (TC-080 AC #2).
static SUBPROCESS_INVOCATIONS: AtomicU64 = AtomicU64::new(0);

/// Install a thread-local mock for the verify-graph-author worker.
/// While the returned guard is alive, [`invoke_worker`] consults the
/// mock instead of spawning a subprocess. Drop the guard to restore
/// the real subprocess behaviour.
#[must_use]
pub fn install_mock(
    mock: impl Fn(&VerifyGraphAuthorInputJson) -> Result<GraphProposal, HandlerError> + 'static,
) -> MockGuard {
    MOCK_WORKER.with(|cell| {
        *cell.borrow_mut() = Some(Box::new(mock));
    });
    MockGuard(())
}

/// RAII guard that clears the thread-local mock on drop.
pub struct MockGuard(());

impl Drop for MockGuard {
    fn drop(&mut self) {
        MOCK_WORKER.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

/// Number of real subprocess invocations since process start. Useful
/// for TC-080's "worker subprocess NOT spawned" assertion.
#[must_use]
pub fn subprocess_invocation_count() -> u64 {
    SUBPROCESS_INVOCATIONS.load(Ordering::SeqCst)
}

/// Reset the subprocess invocation counter (test helper).
pub fn reset_subprocess_invocation_count() {
    SUBPROCESS_INVOCATIONS.store(0, Ordering::SeqCst);
}

/// Invoke the verify-graph-author worker for one bundle.
///
/// Consults the thread-local mock first. Otherwise spawns
/// `python3 -m verify_graph_author --stdin` and pipes the bundle to
/// stdin; the worker writes a single JSON line of `GraphProposal` to
/// stdout, which we parse here.
pub fn invoke_worker(bundle: &VerifyGraphAuthorInputJson) -> Result<GraphProposal, HandlerError> {
    if let Some(mock_result) = try_invoke_mock(bundle) {
        return mock_result;
    }
    SUBPROCESS_INVOCATIONS.fetch_add(1, Ordering::SeqCst);
    invoke_real_subprocess(bundle)
}

fn try_invoke_mock(
    bundle: &VerifyGraphAuthorInputJson,
) -> Option<Result<GraphProposal, HandlerError>> {
    MOCK_WORKER.with(|cell| {
        let borrow = cell.borrow();
        borrow.as_ref().map(|f| f(bundle))
    })
}

fn invoke_real_subprocess(
    bundle: &VerifyGraphAuthorInputJson,
) -> Result<GraphProposal, HandlerError> {
    let mut child = spawn_worker_subprocess()?;
    write_bundle_to_stdin(&mut child, bundle)?;
    let output = child
        .wait_with_output()
        .map_err(|e| HandlerError::Internal {
            detail: format!("worker: waiting for subprocess: {e}"),
        })?;
    check_subprocess_exit(&output)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    parse_proposal_from_stdout(&stdout)
}

fn spawn_worker_subprocess() -> Result<std::process::Child, HandlerError> {
    let argv = resolve_worker_argv()?;
    let (head, tail) = argv
        .split_first()
        .ok_or_else(|| HandlerError::Internal {
            detail: "worker: resolved verify-graph-author argv was empty".to_string(),
        })?;
    let mut cmd = Command::new(head);
    cmd.args(tail)
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().map_err(|e| HandlerError::Internal {
        detail: format!(
            "worker: spawning verify-graph-author subprocess failed: {e} \
             (set up a mock via verify_graph_generate::worker::install_mock for tests)"
        ),
    })
}

/// Resolve the verify-graph-author worker through the shared chain
/// (FT-016). Returns the argv to spawn before `--stdin` is appended.
fn resolve_worker_argv() -> Result<Vec<String>, HandlerError> {
    let entry = role_entry(VERIFY_GRAPH_AUTHOR_ROLE).ok_or_else(|| HandlerError::Internal {
        detail: format!(
            "worker: role {VERIFY_GRAPH_AUTHOR_ROLE} missing from embedded manifest \
             (core/worker/assets/manifest.toml drift)"
        ),
    })?;
    let workdir = std::env::current_dir().ok();
    let inputs = ResolveInputs {
        override_command: None,
        workdir: workdir.as_deref(),
    };
    match resolve(entry, inputs) {
        Resolution::Resolved { argv, .. } => Ok(argv),
        Resolution::Missing { diagnostics } => {
            let detail = if diagnostics.is_empty() {
                format!(
                    "worker: {VERIFY_GRAPH_AUTHOR_ROLE} not resolvable on PATH or via \
                     ${env}=<command>; install with `uv tool install ./workers/verify-graph-author`",
                    env = entry.env_var
                )
            } else {
                format!(
                    "worker: {VERIFY_GRAPH_AUTHOR_ROLE} not resolvable ({})",
                    diagnostics.join("; ")
                )
            };
            Err(HandlerError::Internal { detail })
        }
    }
}

fn write_bundle_to_stdin(
    child: &mut std::process::Child,
    bundle: &VerifyGraphAuthorInputJson,
) -> Result<(), HandlerError> {
    let payload = serde_json::to_vec(bundle).map_err(|e| HandlerError::Internal {
        detail: format!("worker: serialising bundle: {e}"),
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| HandlerError::Internal {
        detail: "worker: stdin closed".to_string(),
    })?;
    stdin
        .write_all(&payload)
        .map_err(|e| HandlerError::Internal {
            detail: format!("worker: writing bundle: {e}"),
        })?;
    Ok(())
}

fn check_subprocess_exit(output: &std::process::Output) -> Result<(), HandlerError> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Err(HandlerError::Internal {
        detail: format!("worker: subprocess exited with {}: {stderr}", output.status),
    })
}

fn parse_proposal_from_stdout(stdout: &str) -> Result<GraphProposal, HandlerError> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| HandlerError::Internal {
            detail: "worker: subprocess produced no parseable stdout".to_string(),
        })?;
    let value: Value = serde_json::from_str(line).map_err(|e| HandlerError::Internal {
        detail: format!("worker: parsing stdout JSON: {e} (line: {line})"),
    })?;
    serde_json::from_value(value).map_err(|e| HandlerError::Internal {
        detail: format!("worker: GraphProposal shape mismatch: {e}"),
    })
}
