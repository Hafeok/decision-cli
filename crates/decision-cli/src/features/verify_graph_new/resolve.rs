//! Reference resolution for `dec verify graph new` (FT-041).
//!
//! Two reference fields need pre-write resolution per FT-041 §Behaviour:
//!
//!   * `verifies` — `FT-NNN` or `TC-NNN`. Resolved by probing the
//!     surrounding `.product/features/` or `.product/tests/` directory
//!     for an artifact file whose stem starts with the supplied id
//!     (e.g. `FT-001-*.md`). Missing → [`HandlerError::DanglingRef`]
//!     with `kind: "verifies"`.
//!   * `environment` — `BNCH-NNN[-suffix]`. Resolved against
//!     `.dec/verify/env/` the same way `verify_bench_show` does.
//!     Missing → [`HandlerError::DanglingRef`] with `kind: "environment"`.
//!
//! Both yield a stable IRI the SHACL shape expects:
//!
//!   * `https://decision-cli.dev/ns/feature/FT-NNN`
//!   * `https://decision-cli.dev/ns/tc/TC-NNN`
//!   * `https://decision-cli.dev/ns/bench/BNCH-NNN[-suffix]`

use std::fs;
use std::path::Path;

use oxigraph::model::NamedNode;

use crate::core::handler::Error as HandlerError;
use crate::core::vocab::IRI_DEC_BENCH_PREFIX;

/// IRI prefix for feature artifacts the graph's `dec:verifies` references.
pub(super) const IRI_FEATURE_PREFIX: &str = "https://decision-cli.dev/ns/feature/";
/// IRI prefix for test-criterion artifacts the graph's `dec:verifies` references.
pub(super) const IRI_TC_PREFIX: &str = "https://decision-cli.dev/ns/tc/";

/// Resolve the caller-supplied `verifies` string into a feature- or TC-IRI.
///
/// The shape `FT-NNN` looks up under `<workdir>/.product/features/`; the
/// shape `TC-NNN` looks up under `<workdir>/.product/tests/`. A reference
/// that resolves to no file surfaces as
/// [`HandlerError::DanglingRef`] with `kind: "verifies"` (FT-041 §Error handling).
pub(super) fn resolve_verifies(workdir: &Path, reference: &str) -> Result<NamedNode, HandlerError> {
    if reference.trim().is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "verifies".to_string(),
            detail: "verifies reference must be non-empty".to_string(),
        });
    }
    let (kind_dir, iri_prefix) = if reference.starts_with("FT-") {
        (".product/features", IRI_FEATURE_PREFIX)
    } else if reference.starts_with("TC-") {
        (".product/tests", IRI_TC_PREFIX)
    } else {
        return Err(HandlerError::InvalidArgument {
            field: "verifies".to_string(),
            detail: format!("verifies reference must start with FT- or TC-; got {reference:?}"),
        });
    };
    let dir = workdir.join(kind_dir);
    if !artifact_exists(&dir, reference)? {
        return Err(HandlerError::DanglingRef {
            reference: reference.to_string(),
            kind: "verifies".to_string(),
        });
    }
    let iri = format!("{iri_prefix}{reference}");
    NamedNode::new(iri).map_err(|e| HandlerError::Internal {
        detail: format!("constructing verifies IRI: {e}"),
    })
}

/// Resolve the caller-supplied `environment` string against
/// `.dec/verify/env/`. Both `BNCH-001` (no suffix) and `BNCH-001-suffix`
/// shapes resolve.
pub(super) fn resolve_environment(
    workdir: &Path,
    reference: &str,
) -> Result<NamedNode, HandlerError> {
    if reference.trim().is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "environment".to_string(),
            detail: "environment reference must be non-empty".to_string(),
        });
    }
    if !reference.starts_with("BNCH-") && !reference.starts_with("ENV-") {
        return Err(HandlerError::InvalidArgument {
            field: "environment".to_string(),
            detail: format!(
                "environment reference must start with BNCH- (or legacy ENV-); got {reference:?}"
            ),
        });
    }
    let env_dir = workdir.join(".dec").join("verify").join("bench");
    let resolved_id =
        resolve_env_id(&env_dir, reference)?.ok_or_else(|| HandlerError::DanglingRef {
            reference: reference.to_string(),
            kind: "environment".to_string(),
        })?;
    let iri = format!("{IRI_DEC_BENCH_PREFIX}{resolved_id}");
    NamedNode::new(iri).map_err(|e| HandlerError::Internal {
        detail: format!("constructing environment IRI: {e}"),
    })
}

/// True iff `dir` contains either `<reference>.md` or `<reference>-*.md`.
fn artifact_exists(dir: &Path, reference: &str) -> Result<bool, HandlerError> {
    if !dir.exists() {
        return Ok(false);
    }
    let exact = dir.join(format!("{reference}.md"));
    if exact.is_file() {
        return Ok(true);
    }
    let prefix = format!("{reference}-");
    for entry in fs::read_dir(dir).map_err(|e| HandlerError::Internal {
        detail: format!("reading {d}: {e}", d = dir.display()),
    })? {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("walking {d}: {e}", d = dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        if stem == reference || stem.starts_with(&prefix) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Walk `env_dir` for a file matching `reference` (exact or `prefix-…`)
/// and return the matching stem (which becomes the IRI tail). Returns
/// `Ok(None)` when no match is found.
fn resolve_env_id(env_dir: &Path, reference: &str) -> Result<Option<String>, HandlerError> {
    if !env_dir.exists() {
        return Ok(None);
    }
    let exact = env_dir.join(format!("{reference}.ttl"));
    if exact.is_file() {
        return Ok(Some(reference.to_string()));
    }
    let matches = collect_env_stems(env_dir, reference)?;
    pick_unique_env_stem(matches, env_dir, reference)
}

fn collect_env_stems(env_dir: &Path, reference: &str) -> Result<Vec<String>, HandlerError> {
    let prefix = format!("{reference}-");
    let mut matches: Vec<String> = Vec::new();
    for entry in fs::read_dir(env_dir).map_err(|e| HandlerError::Internal {
        detail: format!("reading {d}: {e}", d = env_dir.display()),
    })? {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("walking {d}: {e}", d = env_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if stem == reference || stem.starts_with(&prefix) {
            matches.push(stem.to_string());
        }
    }
    Ok(matches)
}

fn pick_unique_env_stem(
    mut matches: Vec<String>,
    env_dir: &Path,
    reference: &str,
) -> Result<Option<String>, HandlerError> {
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches.remove(0))),
        n => Err(HandlerError::Internal {
            detail: format!(
                "ambiguous environment id {reference:?}: {n} files match under {d}",
                d = env_dir.display(),
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
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
            p.push(format!("dec-vgnew-resolve-{tag}-{pid}-{nonce}"));
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

    fn touch(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).expect("mkdir");
        let mut f = File::create(dir.join(name)).expect("touch");
        f.write_all(b"---\nid: X\n---\n").expect("write");
    }

    #[test]
    fn resolves_known_feature_to_iri() {
        let dir = TmpDir::new("ft-ok");
        let features = dir.path().join(".product/features");
        touch(&features, "FT-001-the-thing.md");
        let iri = resolve_verifies(dir.path(), "FT-001").expect("ok");
        assert_eq!(iri.as_str(), "https://decision-cli.dev/ns/feature/FT-001",);
    }

    #[test]
    fn resolves_known_tc_to_iri() {
        let dir = TmpDir::new("tc-ok");
        let tests = dir.path().join(".product/tests");
        touch(&tests, "TC-013-also-a-thing.md");
        let iri = resolve_verifies(dir.path(), "TC-013").expect("ok");
        assert_eq!(iri.as_str(), "https://decision-cli.dev/ns/tc/TC-013");
    }

    #[test]
    fn dangling_verifies_surfaces_dangling_ref() {
        let dir = TmpDir::new("ft-missing");
        let features = dir.path().join(".product/features");
        std::fs::create_dir_all(&features).expect("mkdir");
        let err = resolve_verifies(dir.path(), "FT-999").expect_err("must fail");
        match err {
            HandlerError::DanglingRef { reference, kind } => {
                assert_eq!(reference, "FT-999");
                assert_eq!(kind, "verifies");
            }
            other => panic!("expected DanglingRef, got {other:?}"),
        }
    }

    #[test]
    fn malformed_verifies_prefix_surfaces_invalid_argument() {
        let dir = TmpDir::new("ft-bad");
        let err = resolve_verifies(dir.path(), "garbage").expect_err("must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "verifies"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn resolves_env_short_to_seed_iri() {
        let dir = TmpDir::new("env-short");
        let envs = dir.path().join(".dec/verify/env");
        std::fs::create_dir_all(&envs).expect("mkdir");
        File::create(envs.join("BNCH-001-ephemeral-cli.ttl")).expect("touch");
        let iri = resolve_environment(dir.path(), "BNCH-001").expect("ok");
        assert_eq!(
            iri.as_str(),
            "https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli",
        );
    }

    #[test]
    fn dangling_env_surfaces_dangling_ref() {
        let dir = TmpDir::new("env-missing");
        let envs = dir.path().join(".dec/verify/env");
        std::fs::create_dir_all(&envs).expect("mkdir");
        let err = resolve_environment(dir.path(), "ENV-999").expect_err("must fail");
        match err {
            HandlerError::DanglingRef { reference, kind } => {
                assert_eq!(reference, "ENV-999");
                assert_eq!(kind, "environment");
            }
            other => panic!("expected DanglingRef, got {other:?}"),
        }
    }
}
