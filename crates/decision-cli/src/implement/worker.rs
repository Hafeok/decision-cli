//! Spawn the code-writer worker as a one-shot subprocess and parse its
//! stdout response (ADR-008).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

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

pub(super) fn run_worker(
    worker_command: Option<&str>,
    payload: &DispatchPayloadJson,
) -> Result<WorkerResponseJson> {
    let mut cmd = if let Some(custom) = worker_command {
        build_shell_command(custom)
    } else if let Ok(env_cmd) = std::env::var("CODE_WRITER_CMD") {
        build_shell_command(&env_cmd)
    } else if which("code-writer").is_some() {
        let mut c = Command::new("code-writer");
        c.arg("run-once");
        c
    } else {
        let mut c = Command::new("python3");
        c.arg("-m").arg("code_writer.main").arg("run-once");
        c
    };

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("spawning code-writer worker subprocess")?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("worker stdin closed"))?;
        let body = serde_json::to_vec(payload).context("serialising DispatchPayload")?;
        stdin
            .write_all(&body)
            .context("writing DispatchPayload to worker stdin")?;
    }
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
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| anyhow!("worker produced no parseable stdout"))?;
    let response: WorkerResponseJson = serde_json::from_str(line)
        .with_context(|| format!("parsing worker response: {line}"))?;
    Ok(response)
}

fn build_shell_command(custom: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("{custom} run-once"));
    c
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
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
