//! On-disk persistence for `dec verify bench new` (FT-038).
//!
//! Writes the canonical Turtle file produced by
//! [`crate::core::ontology::verification_bench::to_canonical_turtle`] under
//! `.dec/verify/bench/<id>.ttl`. The file is written **after** the
//! `StreamWriter` chokepoint approves the mutation; SHACL failures
//! therefore never leave a partial file on disk (FT-038 §Invariants).

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_bench::{to_canonical_turtle, VerificationBench};

/// Write the bench's Turtle representation to `.dec/verify/bench/<id>.ttl`.
///
/// Creates the directory if missing. Returns the path written.
pub fn write_bench_file(
    bench_dir: &Path,
    id: &str,
    bench: &VerificationBench,
) -> Result<PathBuf, HandlerError> {
    fs::create_dir_all(bench_dir).map_err(|e| HandlerError::Internal {
        detail: format!("creating {dir}: {e}", dir = bench_dir.display()),
    })?;
    let path = bench_dir.join(format!("{id}.ttl"));
    let ttl = to_canonical_turtle(bench);
    fs::write(&path, ttl.as_bytes()).map_err(|e| HandlerError::Internal {
        detail: format!("writing {p}: {e}", p = path.display()),
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ontology::verification_bench::SafetyClass;
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
        let bench = VerificationBench {
            id: "BNCH-042".to_string(),
            bench_type: "ephemeral-tempdir".to_string(),
            setup: None,
            teardown: None,
            allowed_ops: vec!["shell".to_string()],
            safety_class: SafetyClass::Isolated,
            endpoint: None,
            fixture_source: None,
        };
        let p = write_bench_file(dir.path(), "BNCH-042", &bench).expect("write");
        let bytes = fs::read_to_string(&p).expect("read");
        assert!(bytes.contains("a dec:VerificationBench"));
        assert!(bytes.contains("BNCH-042"));
    }
}
