//! One-shot subprocess invocation of the code-writer worker (ADR-008).
//!
//! Resolution is delegated to [`crate::worker::resolve`] (FT-016 /
//! TC-050) — this module owns only the spawn / stdin / stdout-parse
//! plumbing. Inline `which`, `CODE_WRITER_CMD` reads, and Python module
//! probes have been removed in favour of the shared chain.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::worker::{
    self, format_report_text, role_entry, Resolution, ResolveInputs, WorkerReport,
};

#[derive(Debug, Clone, Serialize)]
pub struct DispatchPayloadJson {
    pub dispatch_id: String,
    pub session_id: String,
    pub feature_id: String,
    pub bundle_markdown: String,
    pub bundle_hash: String,
    pub workspace_path: String,
    pub model_id: String,
    /// FT-066 / ADR-033: the resolved capability's endpoint, pinned by
    /// the dispatcher. The worker translates this plus `model_id` into
    /// the right `ANTHROPIC_*` env vars before spawning `claude -p`.
    pub endpoint: String,
    pub timeout_seconds: u32,
    /// FT-123: agentic loop turn cap. Python worker schema defaults to
    /// 8 when absent; substantive features need much more headroom.
    pub max_turns: u32,
    /// FT-030 / ADR-027: role authority declaration. `None` when the
    /// orchestration store predates FT-030 (legacy slice-1 stores).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<AuthorityJson>,
    /// FT-108: runtime defect feedback the implementer should address.
    /// Empty for fresh-feature dispatch; non-empty when a prior verify
    /// run produced `rejected` evidence and routed it to the implementer.
    #[serde(default)]
    pub defect_feedback: Vec<crate::core::feedback::DefectFeedbackRecord>,
    /// FT-122 / ADR-070: role-scoped tool surface resolved from the role
    /// catalog. Empty for legacy stores pre-dating FT-121; worker
    /// fail-closes per ADR-069.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

/// Serialisable view of a `dec:Authority` for the worker bundle (FT-030).
#[derive(Debug, Clone, Serialize)]
pub struct AuthorityJson {
    pub iri: String,
    pub may_decide: Vec<String>,
    pub must_escalate: Vec<String>,
    pub escalate_via: Vec<EscalationHintJson>,
    pub rationale: String,
}

/// One entry of `escalate_via` — mirrors `core::role_catalog::EscalationHint`.
#[derive(Debug, Clone, Serialize)]
pub struct EscalationHintJson {
    pub category: String,
    pub class: String,
    pub target_role: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerResponseJson {
    #[allow(dead_code)]
    pub dispatch_id: String,
    #[allow(dead_code)]
    pub session_id: String,
    pub status: String,
    pub code_change: Option<CodeChangeJson>,
    #[serde(default)]
    pub telemetry: TelemetryJson,
    #[serde(default)]
    pub error: Option<ErrorJson>,
    /// FT-146: token-breakdown sum across every LiteLLM call in the
    /// dispatch. `None` when the worker did not invoke an LLM (or has
    /// not yet been updated to surface usage). The harness records the
    /// four fields onto the cell's `dec:SessionRecord` so cost rollups
    /// work on cluster-dispatched work.
    #[serde(default)]
    pub usage: Option<WorkerResponseUsage>,
}

/// FT-146: in-band token-breakdown the harness lifts onto the cell's
/// `dec:SessionRecord`. Matches the Python `WorkerResponseUsage` shape.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkerResponseUsage {
    #[serde(default)]
    pub input_tokens_base: u64,
    #[serde(default)]
    pub input_tokens_cache_write: u64,
    #[serde(default)]
    pub input_tokens_cache_hit: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeChangeJson {
    pub iri: String,
    #[allow(dead_code)]
    pub feature_id: String,
    #[allow(dead_code)]
    pub session_id: String,
    #[serde(default)]
    pub files: Vec<FileWriteJson>,
    #[serde(default)]
    pub summary: String,
    /// FT-108: feedback IRIs (urn:dec:feedback:<uuid>) this code change
    /// claims to address. The accept path uses this list to transition
    /// each cited feedback from `produced` to `addressed`.
    #[serde(default)]
    pub addressed_feedback_iris: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileWriteJson {
    pub path: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TelemetryJson {
    #[serde(default)]
    pub turn_count: u64,
    #[serde(default)]
    pub latency_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorJson {
    pub category: String,
    pub message: String,
    #[serde(default)]
    pub detail: String,
}

/// Look up the implementer role and run the shared resolution chain.
/// Returns a fully-built `argv` on success, or a renderable report on
/// missing-worker so `dec implement` can abort pre-session (TC-049).
pub fn preflight_implementer(
    workdir: &Path,
    override_command: Option<&str>,
) -> Result<Vec<String>, WorkerPreflightFailure> {
    let entry = role_entry(crate::implement::IMPLEMENTER_ROLE)
        .expect("code-writer role is in the embedded manifest");
    let res = worker::resolve(
        entry,
        ResolveInputs {
            override_command,
            workdir: Some(workdir),
        },
    );
    match res {
        Resolution::Resolved { mut argv, .. } => {
            // The implementer worker exposes its single-shot mode under
            // the `run-once` subcommand (FT-013). The shared resolver
            // returns the bare invocation; we append the subcommand
            // here so the call site stays role-agnostic.
            argv.push("run-once".to_string());
            Ok(argv)
        }
        Resolution::Missing { .. } => {
            let report = worker::build_report(
                worker::ACTIVE_ROLES_ENGINEERING_DEVELOPMENT,
                Some(workdir),
                override_command,
                Some(crate::implement::IMPLEMENTER_ROLE),
            );
            Err(WorkerPreflightFailure {
                rendered: format_report_text(&report),
                _report: report,
            })
        }
    }
}

/// Carrier for a missing-worker diagnostic. `Display` is the
/// install-hint block; `dec implement` writes it to stderr.
#[derive(Debug)]
pub struct WorkerPreflightFailure {
    pub rendered: String,
    pub _report: WorkerReport,
}

impl std::fmt::Display for WorkerPreflightFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rendered)
    }
}

impl std::error::Error for WorkerPreflightFailure {}

/// Wrapper carrying the parsed worker response plus the raw stdout
/// stream the harness needs for FT-031 feedback record scanning (the
/// scanner is invoked by `super::run` so the `paused-for-feedback`
/// branch can fire before the action artifact is persisted — FT-032).
#[derive(Debug, Clone)]
pub struct WorkerRun {
    pub response: WorkerResponseJson,
    pub raw_stdout: String,
}

pub fn run_worker(argv: &[String], payload: &DispatchPayloadJson) -> Result<WorkerRun> {
    if let Some(mock_result) = try_invoke_mock(payload) {
        return mock_result;
    }
    let mut cmd = build_command_from_argv(argv)?;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("spawning code-writer worker subprocess")?;
    write_payload_to_stdin(&mut child, payload)?;
    let output = child
        .wait_with_output()
        .context("waiting for worker subprocess")?;
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(anyhow!(
            "code-writer worker exited with {} and no stdout. stderr: {stderr}",
            output.status
        ));
    }
    let raw_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let response = parse_worker_response(&output.stdout)?;
    Ok(WorkerRun {
        response,
        raw_stdout,
    })
}

fn build_command_from_argv(argv: &[String]) -> Result<Command> {
    let (head, tail) = argv
        .split_first()
        .ok_or_else(|| anyhow!("resolved worker argv was empty"))?;
    let mut c = Command::new(head);
    c.args(tail);
    Ok(c)
}

fn write_payload_to_stdin(
    child: &mut std::process::Child,
    payload: &DispatchPayloadJson,
) -> Result<()> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("worker stdin closed"))?;
    let body = serde_json::to_vec(payload).context("serialising DispatchPayload")?;
    stdin
        .write_all(&body)
        .context("writing DispatchPayload to worker stdin")?;
    Ok(())
}

fn parse_worker_response(stdout_bytes: &[u8]) -> Result<WorkerResponseJson> {
    let stdout = String::from_utf8_lossy(stdout_bytes).into_owned();
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| anyhow!("worker produced no parseable stdout"))?;
    let response: WorkerResponseJson =
        serde_json::from_str(line).with_context(|| format!("parsing worker response: {line}"))?;
    Ok(response)
}

/// Build the failure detail string surfaced when a worker reports
/// ``status != "ok"``.
pub fn format_worker_failure(error: Option<&ErrorJson>) -> String {
    error
        .map(|e| {
            if e.detail.is_empty() {
                format!("{}: {}", e.category, e.message)
            } else {
                format!(
                    "{}: {}\n--- worker detail ---\n{}",
                    e.category, e.message, e.detail
                )
            }
        })
        .unwrap_or_else(|| "(no error detail)".into())
}

// ---------------------------------------------------------------------
// FT-108 test seam: install_mock lets integration tests intercept
// `run_worker` without spawning a subprocess. Modelled after the
// verify-graph-generate worker's mock harness.
// ---------------------------------------------------------------------

use std::cell::RefCell;

type MockFn = Box<dyn Fn(&DispatchPayloadJson) -> Result<WorkerRun> + 'static>;

thread_local! {
    static MOCK_WORKER: RefCell<Option<MockFn>> = const { RefCell::new(None) };
}

/// Install a thread-local mock for the code-writer worker. While the
/// returned guard is alive, [`run_worker`] consults the mock instead of
/// spawning the subprocess. Drop the guard to restore real behaviour.
#[must_use]
pub fn install_mock(
    mock: impl Fn(&DispatchPayloadJson) -> Result<WorkerRun> + 'static,
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

fn try_invoke_mock(payload: &DispatchPayloadJson) -> Option<Result<WorkerRun>> {
    MOCK_WORKER.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|mock| (mock)(payload))
    })
}

/// FT-108 test helper: surface the `DispatchPayloadJson` shape so
/// integration tests can construct expected payloads. Re-exported via
/// the public crate root only inside `cfg(test)`.
#[cfg(test)]
pub(crate) fn payload_defect_feedback_len(payload: &DispatchPayloadJson) -> usize {
    payload.defect_feedback.len()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::tempdir;

    /// TC-270: Dispatch JSON written to worker stdin contains non-empty
    /// `allowed_tools` when the role is seeded. Validates the FT-122 wire
    /// format: the JSON serialization includes the field and its content
    /// matches the role catalog seed.
    #[test]
    fn tc_270_dispatch_json_has_allowed_tools() {
        // Build a payload with allowed_tools populated
        let tmpdir = tempdir().expect("tmpdir");
        let workspace = tmpdir.path().to_path_buf();

        let payload = DispatchPayloadJson {
            dispatch_id: "urn:dec:dispatch:TC-270".into(),
            session_id: "urn:dec:session:TC-270".into(),
            feature_id: "FT-122".into(),
            bundle_markdown: "# TC-270\nTest bundle\n".into(),
            bundle_hash: "a".repeat(64),
            workspace_path: workspace.to_string_lossy().into_owned(),
            model_id: "claude-sonnet-4-5".into(),
            endpoint: "anthropic".into(),
            timeout_seconds: 1800,
            max_turns: 8,
            authority: None,
            defect_feedback: Vec::new(),
            allowed_tools: vec![
                "read_file".into(),
                "write_file".into(),
                "run_build".into(),
                "run_lint".into(),
                "run_tests".into(),
            ],
        };

        // Serialize to JSON
        let json_str = serde_json::to_string(&payload).expect("serialize payload");

        // Parse back to check the field is present
        let parsed: Value = serde_json::from_str(&json_str).expect("parse JSON");

        // Assert allowed_tools field exists and is an array
        let tools_value = parsed
            .get("allowed_tools")
            .expect("allowed_tools field should exist in JSON");
        assert!(
            tools_value.is_array(),
            "allowed_tools should be a JSON array"
        );

        let tools_array = tools_value.as_array().expect("as array");
        assert_eq!(
            tools_array.len(),
            5,
            "allowed_tools array should have 5 elements"
        );

        // Check each expected tool is present (order-insensitive)
        let expected = vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"];
        for tool in &expected {
            let found = tools_array
                .iter()
                .any(|v| v.as_str() == Some(tool));
            assert!(
                found,
                "expected tool {tool} in JSON allowed_tools array"
            );
        }
    }
}
