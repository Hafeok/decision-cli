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
pub(super) struct DispatchPayloadJson {
    pub dispatch_id: String,
    pub session_id: String,
    pub feature_id: String,
    pub bundle_markdown: String,
    pub bundle_hash: String,
    pub workspace_path: String,
    pub model_id: String,
    pub timeout_seconds: u32,
    /// FT-030 / ADR-027: role authority declaration. `None` when the
    /// orchestration store predates FT-030 (legacy slice-1 stores).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<AuthorityJson>,
}

/// Serialisable view of a `dec:Authority` for the worker bundle (FT-030).
#[derive(Debug, Clone, Serialize)]
pub(super) struct AuthorityJson {
    pub iri: String,
    pub may_decide: Vec<String>,
    pub must_escalate: Vec<String>,
    pub escalate_via: Vec<EscalationHintJson>,
    pub rationale: String,
}

/// One entry of `escalate_via` — mirrors `core::role_catalog::EscalationHint`.
#[derive(Debug, Clone, Serialize)]
pub(super) struct EscalationHintJson {
    pub category: String,
    pub class: String,
    pub target_role: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorkerResponseJson {
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
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CodeChangeJson {
    pub iri: String,
    #[allow(dead_code)]
    pub feature_id: String,
    #[allow(dead_code)]
    pub session_id: String,
    #[serde(default)]
    pub files: Vec<FileWriteJson>,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FileWriteJson {
    pub path: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct TelemetryJson {
    #[serde(default)]
    pub turn_count: u64,
    #[serde(default)]
    pub latency_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ErrorJson {
    pub category: String,
    pub message: String,
    #[serde(default)]
    pub detail: String,
}

/// Look up the implementer role and run the shared resolution chain.
/// Returns a fully-built `argv` on success, or a renderable report on
/// missing-worker so `dec implement` can abort pre-session (TC-049).
pub(super) fn preflight_implementer(
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
pub(super) struct WorkerPreflightFailure {
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
pub(super) struct WorkerRun {
    pub response: WorkerResponseJson,
    pub raw_stdout: String,
}

pub(super) fn run_worker(argv: &[String], payload: &DispatchPayloadJson) -> Result<WorkerRun> {
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
pub(super) fn format_worker_failure(error: Option<&ErrorJson>) -> String {
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
