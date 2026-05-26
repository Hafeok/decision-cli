//! Env-file parsing for `dec workers run` (FT-095 / ADR-063).
//!
//! Reads `KEY=VALUE` lines from a local config file (default
//! `~/.dec/workers.env`, overridable by `--env-file <path>`) and
//! confirms the four secrets the worker needs are present.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// The four env vars every worker container must receive (per FT-095
/// §Scope). Order is the order surfaced in error messages.
pub const REQUIRED_ENV_VARS: [&str; 4] = [
    "PIPELINE_ENDPOINT",
    "PIPELINE_TOKEN",
    "LITELLM_BASE_URL",
    "LITELLM_API_KEY",
];

/// Default env-file path under `$HOME`. The legacy `~/.pipeline-cli/`
/// directory is also probed as a fallback for operators following the
/// `feature_spec`'s literal reference.
const PRIMARY_REL: &str = ".dec/workers.env";
const LEGACY_REL: &str = ".pipeline-cli/workers.env";

/// Outcome of a successful env-file load. The map preserves whatever
/// extras the operator chose to include; only the four required keys
/// are validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnvFile {
    /// Absolute path that was read.
    pub path: PathBuf,
    /// Keys present in the file (for telemetry / display). Values are
    /// intentionally not retained — the docker invocation passes the
    /// file path through `--env-file`, so values never round-trip
    /// through Rust memory or logs.
    pub keys: Vec<String>,
}

/// Errors the env-file loader can produce.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvFileError {
    /// No env file was found at the override path or the defaults.
    #[error(
        "env file not found at {searched}. Create `~/.dec/workers.env` with the four \
         required keys ({required}) or pass --env-file <path>."
    )]
    NotFound {
        /// Comma-joined list of paths that were probed.
        searched: String,
        /// Comma-joined required keys for the operator hint.
        required: String,
    },
    /// The override path was supplied but the file does not exist.
    #[error("env file `{path}` does not exist")]
    OverrideMissing {
        /// Override path the operator provided.
        path: String,
    },
    /// The env file existed but could not be read.
    #[error("reading env file `{path}`: {message}")]
    Read {
        /// Path that failed to read.
        path: String,
        /// OS-level error message.
        message: String,
    },
    /// One or more required keys are missing from the env file. The
    /// emitted message names every missing key so the operator only
    /// re-runs once.
    #[error("env file `{path}` is missing required keys: {missing}")]
    MissingKeys {
        /// Path that was read.
        path: String,
        /// Comma-joined list of missing keys.
        missing: String,
    },
    /// A line in the env file is malformed (no `=` separator after a
    /// non-blank, non-comment prefix).
    #[error("env file `{path}` line {line}: expected KEY=VALUE, got `{content}`")]
    MalformedLine {
        /// Path the parse failed on.
        path: String,
        /// 1-based line number.
        line: usize,
        /// Verbatim content of the offending line.
        content: String,
    },
}

/// Load an env file, optionally overridden by `--env-file`, and
/// validate that the four required keys are present.
pub fn load_env_file(override_path: Option<&Path>) -> Result<ResolvedEnvFile, EnvFileError> {
    let path = resolve_path(override_path)?;
    let raw = fs::read_to_string(&path).map_err(|e| EnvFileError::Read {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    let keys = parse_keys(&path, &raw)?;
    let missing = missing_required(&keys);
    if !missing.is_empty() {
        return Err(EnvFileError::MissingKeys {
            path: path.display().to_string(),
            missing: missing.join(", "),
        });
    }
    Ok(ResolvedEnvFile { path, keys })
}

fn resolve_path(override_path: Option<&Path>) -> Result<PathBuf, EnvFileError> {
    if let Some(p) = override_path {
        if !p.exists() {
            return Err(EnvFileError::OverrideMissing {
                path: p.display().to_string(),
            });
        }
        return Ok(p.to_path_buf());
    }
    let mut searched: Vec<PathBuf> = Vec::new();
    if let Some(home) = home_dir() {
        for rel in [PRIMARY_REL, LEGACY_REL] {
            let candidate = home.join(rel);
            if candidate.exists() {
                return Ok(candidate);
            }
            searched.push(candidate);
        }
    } else {
        searched.push(PathBuf::from(PRIMARY_REL));
    }
    let searched_join = searched
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(EnvFileError::NotFound {
        searched: searched_join,
        required: REQUIRED_ENV_VARS.join(", "),
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn parse_keys(path: &Path, raw: &str) -> Result<Vec<String>, EnvFileError> {
    let mut keys = Vec::new();
    for (idx, raw_line) in raw.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip an optional leading `export `.
        let body = line.strip_prefix("export ").unwrap_or(line).trim();
        let eq = body.find('=').ok_or_else(|| EnvFileError::MalformedLine {
            path: path.display().to_string(),
            line: idx + 1,
            content: raw_line.to_string(),
        })?;
        let key = body[..eq].trim();
        if key.is_empty() {
            return Err(EnvFileError::MalformedLine {
                path: path.display().to_string(),
                line: idx + 1,
                content: raw_line.to_string(),
            });
        }
        keys.push(key.to_string());
    }
    Ok(keys)
}

fn missing_required(present: &[String]) -> Vec<&'static str> {
    REQUIRED_ENV_VARS
        .iter()
        .copied()
        .filter(|req| !present.iter().any(|k| k == req))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        p.push(format!("{label}-{pid}-{nonce}"));
        fs::create_dir_all(&p).expect("create tempdir");
        p
    }

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, content).expect("write env file");
        p
    }

    #[test]
    fn accepts_well_formed_env_file_with_all_required_keys() {
        let tmp = tempdir("env-good");
        let path = write(
            &tmp,
            "good.env",
            "PIPELINE_ENDPOINT=https://pipeline.example/sse\n\
             PIPELINE_TOKEN=tok\n\
             LITELLM_BASE_URL=http://localhost:4000\n\
             LITELLM_API_KEY=sk-litellm\n",
        );
        let resolved = load_env_file(Some(&path)).expect("load ok");
        assert_eq!(resolved.path, path);
        for req in REQUIRED_ENV_VARS {
            assert!(resolved.keys.iter().any(|k| k == req), "missing {req}");
        }
    }

    #[test]
    fn rejects_when_any_required_key_is_missing() {
        let tmp = tempdir("env-missing");
        let path = write(
            &tmp,
            "missing.env",
            "PIPELINE_ENDPOINT=https://x/\n\
             PIPELINE_TOKEN=tok\n\
             LITELLM_BASE_URL=http://localhost:4000\n",
        );
        let err = load_env_file(Some(&path)).expect_err("missing key must fail");
        match err {
            EnvFileError::MissingKeys { missing, .. } => {
                assert!(missing.contains("LITELLM_API_KEY"), "{missing}");
            }
            other => panic!("expected MissingKeys, got {other:?}"),
        }
    }

    #[test]
    fn override_missing_path_errors_distinctly() {
        let err = load_env_file(Some(Path::new("/nonexistent/workers.env")))
            .expect_err("override path must exist");
        assert!(matches!(err, EnvFileError::OverrideMissing { .. }));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let tmp = tempdir("env-comments");
        let path = write(
            &tmp,
            "ok.env",
            "# a comment\n\
             \n\
             export PIPELINE_ENDPOINT=https://e/\n\
             PIPELINE_TOKEN=t\n\
             LITELLM_BASE_URL=u\n\
             LITELLM_API_KEY=k\n",
        );
        load_env_file(Some(&path)).expect("ok");
    }

    #[test]
    fn malformed_line_rejected() {
        let tmp = tempdir("env-malformed");
        let path = write(
            &tmp,
            "bad.env",
            "PIPELINE_ENDPOINT=ok\nNOEQUAL\nPIPELINE_TOKEN=t\n",
        );
        let err = load_env_file(Some(&path)).expect_err("malformed line must fail");
        assert!(matches!(err, EnvFileError::MalformedLine { line: 2, .. }));
    }
}
