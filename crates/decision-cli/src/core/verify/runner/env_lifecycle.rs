//! Phase-2 env setup + Phase-4 teardown (FT-098 §Behaviour).
//!
//! Slice-3 scope: support `ephemeral-tempdir` (mktemp under `DEC_TMP`),
//! `repo-path` (resolve relative path), and `remote-http` (no working
//! dir). Per FT-098 §Out of scope, additional env types land in later
//! slices.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::ontology::verification_bench::VerificationBench;

/// Result of Phase-2 setup — the resolved working directory and a flag
/// telling Phase-4 whether to clean it up.
pub(crate) struct EnvHandle {
    /// Working directory `${dec_workdir}` binds to.
    pub dec_workdir: PathBuf,
    /// `true` for `envType = ephemeral-tempdir`; the runner removes the
    /// dir at teardown (unless `DEC_KEEP_TMP=1`).
    pub cleanup: bool,
}

/// Resolve the env's runtime working directory. No I/O for `remote-*`
/// envs; `mkdir_all` for `ephemeral-tempdir`; join for `repo-path`.
pub(crate) fn setup(workdir: &std::path::Path, env: &VerificationBench) -> EnvHandle {
    match env.bench_type.as_str() {
        "ephemeral-tempdir" => {
            let dec_workdir = mint_tempdir(env);
            let _ = std::fs::create_dir_all(&dec_workdir);
            EnvHandle {
                dec_workdir,
                cleanup: true,
            }
        }
        "repo-path" => {
            // FT-053: `dec:fixtureSource` carries the repo-relative path
            // a fixture tree was materialised at. When absent, fall back
            // to the working directory itself.
            let rel = env.fixture_source.clone().unwrap_or_else(|| ".".into());
            let dec_workdir = workdir.join(rel);
            EnvHandle {
                dec_workdir,
                cleanup: false,
            }
        }
        // `remote-http`, `remote-grpc`, etc. — HTTP-only kinds run with
        // the workdir as base. The handler picks up the endpoint
        // separately.
        _ => EnvHandle {
            dec_workdir: workdir.to_path_buf(),
            cleanup: false,
        },
    }
}

/// Phase-4 teardown — best-effort cleanup of the ephemeral tempdir.
pub(crate) fn teardown(handle: &EnvHandle) {
    if !handle.cleanup {
        return;
    }
    if std::env::var("DEC_KEEP_TMP").is_ok() {
        return;
    }
    let _ = std::fs::remove_dir_all(&handle.dec_workdir);
}

fn mint_tempdir(env: &VerificationBench) -> PathBuf {
    let base = std::env::var_os("DEC_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("dec-verify"));
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    base.join(format!("{id}-{pid}-{nonce}", id = env.id))
}
