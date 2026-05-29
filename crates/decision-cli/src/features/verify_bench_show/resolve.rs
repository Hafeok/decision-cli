//! On-disk bench lookup for `dec verify bench show` (FT-040).
//!
//! Maps an `BNCH-NNN[-suffix]` id to its `.dec/verify/bench/<id>.ttl` file.
//! Two id shapes resolve:
//!
//!   * Exact id — `BNCH-001-ephemeral-cli` → `BNCH-001-ephemeral-cli.ttl`.
//!   * Numeric-only id — `BNCH-001` resolves to any file in the dir
//!     whose stem matches either `BNCH-001` exactly or `BNCH-001-*`
//!     (i.e. an bench minted as `BNCH-001` with no suffix, OR the seed
//!     bench's full id `BNCH-001-ephemeral-cli`).
//!
//! Ambiguity (two files share the same numeric tail) surfaces as
//! `Internal` so it does not silently shadow the seed.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_bench::{from_turtle, VerificationBench};

const ARTIFACT_KIND: &str = "VerificationBench";

/// Locate the on-disk file for `id` and parse it into a
/// [`VerificationBench`]. Returns the resolved path alongside
/// the parsed bench so the caller can report it in responses.
pub(super) fn load_env(
    bench_dir: &Path,
    id: &str,
) -> Result<(PathBuf, VerificationBench), HandlerError> {
    let path = resolve_path(bench_dir, id)?;
    let bench = from_turtle(&path).map_err(|e| HandlerError::Internal {
        detail: format!("parsing bench file {p}: {e}", p = path.display()),
    })?;
    Ok((path, bench))
}

/// Resolve `id` to a file path under `bench_dir`. Returns
/// [`HandlerError::ArtifactNotFound`] when no file matches.
fn resolve_path(bench_dir: &Path, id: &str) -> Result<PathBuf, HandlerError> {
    let exact = bench_dir.join(format!("{id}.ttl"));
    if exact.is_file() {
        return Ok(exact);
    }
    if !bench_dir.exists() {
        return Err(not_found(id));
    }
    let mut matches = collect_suffix_matches(bench_dir, id)?;
    match matches.len() {
        0 => Err(not_found(id)),
        1 => Ok(matches.remove(0)),
        n => Err(HandlerError::Internal {
            detail: format!(
                "ambiguous bench id {id:?}: {n} files match (under {dir})",
                dir = bench_dir.display()
            ),
        }),
    }
}

/// Walk `bench_dir` and collect `.ttl` files whose stem matches `id`
/// exactly OR begins with `id + "-"` (suffixed siblings).
fn collect_suffix_matches(bench_dir: &Path, id: &str) -> Result<Vec<PathBuf>, HandlerError> {
    let prefix = format!("{id}-");
    let mut matches: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(bench_dir).map_err(|e| HandlerError::Internal {
        detail: format!("reading {dir}: {e}", dir = bench_dir.display()),
    })? {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("walking {dir}: {e}", dir = bench_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        // An exact stem match is already handled by the caller; consider
        // suffixed siblings only.
        if stem == id || stem.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    Ok(matches)
}

fn not_found(id: &str) -> HandlerError {
    HandlerError::ArtifactNotFound {
        kind: ARTIFACT_KIND.to_string(),
        id: id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

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
            p.push(format!("dec-resolve-{tag}-{pid}-{nonce}"));
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
    fn resolves_exact_match() {
        let dir = TmpDir::new("exact");
        let p = dir.path().join("BNCH-007.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "BNCH-007").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn resolves_suffixed_seed_via_full_id() {
        let dir = TmpDir::new("seed-full");
        let p = dir.path().join("BNCH-001-ephemeral-cli.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "BNCH-001-ephemeral-cli").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn resolves_suffixed_seed_via_short_id() {
        let dir = TmpDir::new("seed-short");
        let p = dir.path().join("BNCH-001-ephemeral-cli.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "BNCH-001").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn missing_id_returns_artifact_not_found() {
        let dir = TmpDir::new("missing");
        let err = resolve_path(dir.path(), "BNCH-999").expect_err("must fail");
        match err {
            HandlerError::ArtifactNotFound { kind, id } => {
                assert_eq!(kind, "VerificationBench");
                assert_eq!(id, "BNCH-999");
            }
            other => panic!("expected ArtifactNotFound, got {other:?}"),
        }
    }

    #[test]
    fn missing_directory_returns_artifact_not_found() {
        let dir = TmpDir::new("nodir");
        let inner = dir.path().join("does-not-exist");
        let err = resolve_path(&inner, "BNCH-001").expect_err("must fail");
        match err {
            HandlerError::ArtifactNotFound { id, .. } => assert_eq!(id, "BNCH-001"),
            other => panic!("expected ArtifactNotFound, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_short_id_returns_internal() {
        let dir = TmpDir::new("ambig");
        File::create(dir.path().join("BNCH-001-foo.ttl")).expect("touch");
        File::create(dir.path().join("BNCH-001-bar.ttl")).expect("touch");
        let err = resolve_path(dir.path(), "BNCH-001").expect_err("must fail");
        match err {
            HandlerError::Internal { detail } => assert!(detail.contains("ambiguous")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }
}
