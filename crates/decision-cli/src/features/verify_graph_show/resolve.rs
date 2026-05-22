//! On-disk graph lookup for `dec verify graph show` (FT-043).
//!
//! Maps a `VG-NNN[-suffix]` id to its `.dec/verify/graph/<id>.ttl` file.
//! Both `VG-001` (no suffix) and `VG-001-foo` shapes resolve. Ambiguity
//! (two files share the same numeric tail) surfaces as `Internal` so
//! the caller does not silently shadow the wrong artifact.
//!
//! Also exposes a best-effort helper that reads a referenced environment's
//! safety class from `.dec/verify/env/<id>.ttl`. The helper is best-effort
//! because the env may have been deleted while the graph remains on disk;
//! the text renderer falls back to omitting the safety annotation when
//! it is `None`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_env::from_turtle as from_env_turtle;
use crate::core::ontology::verification_graph::{from_turtle, VerificationGraph};

const ARTIFACT_KIND: &str = "VerificationGraph";

/// Locate the on-disk file for `id` and parse it into a
/// [`VerificationGraph`]. Returns the resolved path alongside the parsed
/// graph so callers can report it in responses.
pub(super) fn load_graph(
    graph_dir: &Path,
    id: &str,
) -> Result<(PathBuf, VerificationGraph), HandlerError> {
    let path = resolve_path(graph_dir, id)?;
    let graph = from_turtle(&path).map_err(|e| HandlerError::Internal {
        detail: format!("parsing graph file {p}: {e}", p = path.display()),
    })?;
    Ok((path, graph))
}

/// Resolve `id` to a file path under `graph_dir`. Returns
/// [`HandlerError::ArtifactNotFound`] when no file matches.
fn resolve_path(graph_dir: &Path, id: &str) -> Result<PathBuf, HandlerError> {
    let exact = graph_dir.join(format!("{id}.ttl"));
    if exact.is_file() {
        return Ok(exact);
    }
    if !graph_dir.exists() {
        return Err(not_found(id));
    }
    let matches = collect_id_matches(graph_dir, id)?;
    pick_unique_match(matches, graph_dir, id)
}

fn collect_id_matches(graph_dir: &Path, id: &str) -> Result<Vec<PathBuf>, HandlerError> {
    let prefix = format!("{id}-");
    let mut matches: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(graph_dir).map_err(|e| HandlerError::Internal {
        detail: format!("reading {dir}: {e}", dir = graph_dir.display()),
    })? {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("walking {dir}: {e}", dir = graph_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if stem == id || stem.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    Ok(matches)
}

fn pick_unique_match(
    mut matches: Vec<PathBuf>,
    graph_dir: &Path,
    id: &str,
) -> Result<PathBuf, HandlerError> {
    match matches.len() {
        0 => Err(not_found(id)),
        1 => Ok(matches.remove(0)),
        n => Err(HandlerError::Internal {
            detail: format!(
                "ambiguous graph id {id:?}: {n} files match (under {dir})",
                dir = graph_dir.display()
            ),
        }),
    }
}

fn not_found(id: &str) -> HandlerError {
    HandlerError::ArtifactNotFound {
        kind: ARTIFACT_KIND.to_string(),
        id: id.to_string(),
    }
}

/// Best-effort lookup of the environment's safety class. Returns `None`
/// when the env file is missing or unreadable; the text renderer treats
/// the absence as "no safety annotation".
pub(super) fn load_environment_safety(env_dir: &Path, env_id: &str) -> Option<String> {
    let exact = env_dir.join(format!("{env_id}.ttl"));
    let path = if exact.is_file() {
        exact
    } else {
        find_env_path(env_dir, env_id)?
    };
    let env = from_env_turtle(&path).ok()?;
    Some(env.safety_class.as_str().to_string())
}

fn find_env_path(env_dir: &Path, env_id: &str) -> Option<PathBuf> {
    if !env_dir.exists() {
        return None;
    }
    let prefix = format!("{env_id}-");
    let entries = fs::read_dir(env_dir).ok()?;
    let mut matches: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if stem == env_id || stem.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    if matches.len() == 1 {
        Some(matches.remove(0))
    } else {
        None
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
            p.push(format!("dec-vgshow-resolve-{tag}-{pid}-{nonce}"));
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
        let p = dir.path().join("VG-007.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "VG-007").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn resolves_suffixed_via_full_id() {
        let dir = TmpDir::new("suff-full");
        let p = dir.path().join("VG-001-foo.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "VG-001-foo").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn resolves_suffixed_via_short_id() {
        let dir = TmpDir::new("suff-short");
        let p = dir.path().join("VG-001-foo.ttl");
        File::create(&p).expect("touch");
        let got = resolve_path(dir.path(), "VG-001").expect("ok");
        assert_eq!(got, p);
    }

    #[test]
    fn missing_id_returns_artifact_not_found() {
        let dir = TmpDir::new("missing");
        let err = resolve_path(dir.path(), "VG-999").expect_err("must fail");
        match err {
            HandlerError::ArtifactNotFound { kind, id } => {
                assert_eq!(kind, "VerificationGraph");
                assert_eq!(id, "VG-999");
            }
            other => panic!("expected ArtifactNotFound, got {other:?}"),
        }
    }

    #[test]
    fn missing_directory_returns_artifact_not_found() {
        let dir = TmpDir::new("nodir");
        let inner = dir.path().join("does-not-exist");
        let err = resolve_path(&inner, "VG-001").expect_err("must fail");
        match err {
            HandlerError::ArtifactNotFound { id, .. } => assert_eq!(id, "VG-001"),
            other => panic!("expected ArtifactNotFound, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_short_id_returns_internal() {
        let dir = TmpDir::new("ambig");
        File::create(dir.path().join("VG-001-foo.ttl")).expect("touch");
        File::create(dir.path().join("VG-001-bar.ttl")).expect("touch");
        let err = resolve_path(dir.path(), "VG-001").expect_err("must fail");
        match err {
            HandlerError::Internal { detail } => assert!(detail.contains("ambiguous")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn load_environment_safety_returns_none_for_missing_env_dir() {
        let dir = TmpDir::new("env-none");
        let env_dir = dir.path().join("does-not-exist");
        assert_eq!(
            load_environment_safety(&env_dir, "ENV-001-ephemeral-cli"),
            None
        );
    }
}
