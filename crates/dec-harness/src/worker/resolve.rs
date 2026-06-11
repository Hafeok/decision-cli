//! The single shared resolution chain (FT-016 / TC-050).
//!
//! Order of attempts, per the manifest entry for the requested role:
//!
//! 1. `--worker-command <cmd>` override (only set by `dec implement`).
//! 2. `$<ROLE>_CMD` env var (e.g. `CODE_WRITER_CMD`).
//! 3. `which <console_script>` on `$PATH`.
//! 4. Sibling-workspace probe: when run from inside a workspace tree,
//!    `workers/<role>/.venv/bin/<console_script>`.
//! 5. `python3 -c "import <python_module>"` probe.
//!
//! Steps 3–5 are **side-effect-free** (TC-047 #6): no descendant process
//! is invoked with stdin data; `python3 -c "import X"` is the heaviest
//! probe and it runs with stdin closed.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::manifest::WorkerEntry;

/// How the chain landed on a candidate invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    /// `--worker-command` (or programmatic) override.
    Override,
    /// `$<ROLE>_CMD` env var.
    Env,
    /// `which <console_script>` on `$PATH`.
    Path,
    /// Workspace-relative `workers/<role>/.venv/bin/<console_script>`.
    SiblingWorkspace,
    /// `python3 -m <python_module>` (preceded by a successful import probe).
    PythonModule,
}

impl ResolutionKind {
    /// Stable lower-kebab string used by the JSON shape (TC-048 #3).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::Env => "env",
            Self::Path => "path",
            Self::SiblingWorkspace => "sibling-workspace",
            Self::PythonModule => "python-module",
        }
    }
}

/// Result of running [`resolve`] against a single role.
#[derive(Debug, Clone)]
pub enum Resolution {
    /// Resolved successfully — `argv` is non-empty and ready to spawn.
    Resolved {
        /// Which step in the chain matched.
        kind: ResolutionKind,
        /// Argument vector ready for `Command::new(&argv[0]).args(&argv[1..])`.
        argv: Vec<String>,
        /// Probe-time stderr / stale-entry notes (never `dec` errors).
        diagnostics: Vec<String>,
    },
    /// Chain exhausted with no candidate found.
    Missing {
        /// Diagnostics gathered during failed probes (e.g. stale `which`
        /// hits, the captured stderr from a failing import probe).
        diagnostics: Vec<String>,
    },
}

impl Resolution {
    /// True iff the chain found a usable invocation.
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }
}

/// Inputs to [`resolve`]. Everything ambient is read inside `resolve` —
/// this struct only carries the explicit overrides each caller supplies.
#[derive(Debug, Clone, Default)]
pub struct ResolveInputs<'a> {
    /// Override the resolution result (only present on `dec implement`).
    pub override_command: Option<&'a str>,
    /// Working directory used for the sibling-workspace probe (and for
    /// CWD-relative shell commands like `code-writer run-once`).
    pub workdir: Option<&'a Path>,
}

/// Resolve a worker for the given manifest entry. See module docs for
/// the chain. **The only place in `crates/decision-cli/src/` that may
/// implement worker resolution** — see TC-050.
#[must_use]
pub fn resolve(entry: &WorkerEntry, inputs: ResolveInputs<'_>) -> Resolution {
    let mut diagnostics: Vec<String> = Vec::new();
    if let Some(r) = try_override(inputs.override_command) {
        return r;
    }
    if let Some(r) = try_env(entry) {
        return r;
    }
    if let Some(r) = try_path(entry, &mut diagnostics) {
        return r;
    }
    if let Some(r) = try_sibling_workspace(entry, inputs.workdir) {
        return r;
    }
    try_python_module(entry, diagnostics)
}

fn try_override(override_command: Option<&str>) -> Option<Resolution> {
    let cmd = override_command?.trim();
    if cmd.is_empty() {
        return None;
    }
    Some(Resolution::Resolved {
        kind: ResolutionKind::Override,
        argv: split_shell_cmd(cmd),
        diagnostics: Vec::new(),
    })
}

fn try_env(entry: &WorkerEntry) -> Option<Resolution> {
    let env_cmd = std::env::var(entry.env_var).ok()?;
    let trimmed = env_cmd.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(Resolution::Resolved {
        kind: ResolutionKind::Env,
        argv: split_shell_cmd(trimmed),
        diagnostics: Vec::new(),
    })
}

fn try_path(entry: &WorkerEntry, diagnostics: &mut Vec<String>) -> Option<Resolution> {
    match which_on_path(entry.console_script) {
        WhichResult::Found(path) => Some(Resolution::Resolved {
            kind: ResolutionKind::Path,
            argv: vec![path.to_string_lossy().into_owned()],
            diagnostics: std::mem::take(diagnostics),
        }),
        WhichResult::StaleEntry(path) => {
            diagnostics.push(format!("stale PATH entry: {} (skipped)", path.display()));
            None
        }
        WhichResult::NotFound => None,
    }
}

fn try_sibling_workspace(entry: &WorkerEntry, workdir: Option<&Path>) -> Option<Resolution> {
    let workdir = workdir?;
    let path = sibling_workspace_probe(workdir, entry.role, entry.console_script)?;
    Some(Resolution::Resolved {
        kind: ResolutionKind::SiblingWorkspace,
        argv: vec![path.to_string_lossy().into_owned()],
        diagnostics: Vec::new(),
    })
}

fn try_python_module(entry: &WorkerEntry, mut diagnostics: Vec<String>) -> Resolution {
    match python_import_probe(entry.python_module) {
        PythonProbe::Importable => Resolution::Resolved {
            kind: ResolutionKind::PythonModule,
            argv: vec![
                "python3".to_string(),
                "-m".to_string(),
                entry.python_module.to_string(),
            ],
            diagnostics,
        },
        PythonProbe::NotImportable(stderr) => {
            push_python_diagnostic(&mut diagnostics, entry.python_module, &stderr);
            Resolution::Missing { diagnostics }
        }
        PythonProbe::Python3Missing => {
            diagnostics.push("python3 not on PATH".to_string());
            Resolution::Missing { diagnostics }
        }
    }
}

fn push_python_diagnostic(diagnostics: &mut Vec<String>, module: &str, stderr: &str) {
    if stderr.is_empty() {
        diagnostics.push(format!(
            "python3 -c \"import {module}\" failed (no stderr captured)"
        ));
    } else {
        diagnostics.push(format!("python3 -c \"import {module}\": {stderr}"));
    }
}

/// Split a shell-style command line into argv. Conservative — no full
/// POSIX quoting — but matches what `sh -c` would do for the common
/// case of `"<bin> arg1 arg2"` strings operators set in `$CODE_WRITER_CMD`.
fn split_shell_cmd(cmd: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for ch in cmd.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(cmd.to_string());
    }
    out
}

enum WhichResult {
    Found(PathBuf),
    StaleEntry(PathBuf),
    NotFound,
}

fn which_on_path(bin: &str) -> WhichResult {
    let Some(path) = std::env::var_os("PATH") else {
        return WhichResult::NotFound;
    };
    let mut stale: Option<PathBuf> = None;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        match std::fs::metadata(&candidate) {
            Ok(md) if md.is_file() => return WhichResult::Found(candidate),
            Ok(_) => {}
            Err(_) => {
                // Some PATH dirs reference candidates by symlink; capture
                // the first not-found-but-named-it entry as a stale hit.
                if stale.is_none() && candidate.symlink_metadata().is_ok() {
                    stale = Some(candidate);
                }
            }
        }
    }
    if let Some(p) = stale {
        WhichResult::StaleEntry(p)
    } else {
        WhichResult::NotFound
    }
}

fn sibling_workspace_probe(workdir: &Path, role: &str, console_script: &str) -> Option<PathBuf> {
    let mut dir = workdir
        .canonicalize()
        .ok()
        .or_else(|| Some(workdir.to_path_buf()))?;
    for _ in 0..10 {
        if dir.join("Cargo.toml").is_file() {
            let candidate = dir
                .join("workers")
                .join(role)
                .join(".venv")
                .join("bin")
                .join(console_script);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        let parent = dir.parent()?.to_path_buf();
        if parent == dir {
            break;
        }
        dir = parent;
    }
    None
}

enum PythonProbe {
    Importable,
    NotImportable(String),
    Python3Missing,
}

fn python_import_probe(module: &str) -> PythonProbe {
    let mut cmd = Command::new("python3");
    cmd.arg("-c").arg(format!("import {module}"));
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());
    let out = match cmd.output() {
        Ok(o) => o,
        Err(_) => return PythonProbe::Python3Missing,
    };
    if out.status.success() {
        PythonProbe::Importable
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        PythonProbe::NotImportable(stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::manifest::role_entry;

    #[test]
    fn override_resolves_first() {
        let e = role_entry("code-writer").expect("entry");
        let r = resolve(
            e,
            ResolveInputs {
                override_command: Some("my-worker --flag"),
                workdir: None,
            },
        );
        match r {
            Resolution::Resolved { kind, argv, .. } => {
                assert_eq!(kind, ResolutionKind::Override);
                assert_eq!(argv[0], "my-worker");
                assert_eq!(argv[1], "--flag");
            }
            Resolution::Missing { .. } => panic!("override should resolve"),
        }
    }

    #[test]
    fn split_shell_cmd_handles_quotes() {
        let v = split_shell_cmd("a 'b c' d");
        assert_eq!(v, vec!["a", "b c", "d"]);
        let v2 = split_shell_cmd("/x/y \"a b\"");
        assert_eq!(v2, vec!["/x/y", "a b"]);
    }
}
