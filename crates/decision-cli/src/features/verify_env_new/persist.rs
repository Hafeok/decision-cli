//! On-disk persistence for `dec verify env new` (FT-038).
//!
//! Writes the canonical Turtle file produced by
//! [`crate::core::ontology::verification_env::to_canonical_turtle`] under
//! `.dec/verify/env/<id>.ttl`. The file is written **after** the
//! `StreamWriter` chokepoint approves the mutation; SHACL failures
//! therefore never leave a partial file on disk (FT-038 §Invariants).

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_env::{to_canonical_turtle, VerificationEnvironment};

/// Write the env's Turtle representation to `.dec/verify/env/<id>.ttl`.
///
/// Creates the directory if missing. Returns the path written.
pub fn write_env_file(
    env_dir: &Path,
    id: &str,
    env: &VerificationEnvironment,
) -> Result<PathBuf, HandlerError> {
    fs::create_dir_all(env_dir).map_err(|e| HandlerError::Internal {
        detail: format!("creating {dir}: {e}", dir = env_dir.display()),
    })?;
    let path = env_dir.join(format!("{id}.ttl"));
    let ttl = to_canonical_turtle(env);
    fs::write(&path, ttl.as_bytes()).map_err(|e| HandlerError::Internal {
        detail: format!("writing {p}: {e}", p = path.display()),
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ontology::verification_env::SafetyClass;
    use std::path::PathBuf;

    struct TmpDir {
        path: PathBuf,
    }
    impl TmpDir {
        fn new(tag: &str) -> Self {
            let mut p = std::env::temp_dir();
            let pid = std::process::id();
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            p.push(format!("dec-persist-{tag}-{pid}-{nonce}"));
            std::fs::create_dir_all(&p).expect("tmp");
            Self { path: p }
        }
        fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn writes_canonical_turtle_to_disk() {
        let dir = TmpDir::new("write");
        let env = VerificationEnvironment {
            id: "ENV-042".to_string(),
            env_type: "ephemeral-tempdir".to_string(),
            setup: None,
            teardown: None,
            allowed_ops: vec!["shell".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: None,
        };
        let p = write_env_file(dir.path(), "ENV-042", &env).expect("write");
        let bytes = fs::read_to_string(&p).expect("read");
        assert!(bytes.contains("a dec:VerificationEnvironment"));
        assert!(bytes.contains("ENV-042"));
    }
}
