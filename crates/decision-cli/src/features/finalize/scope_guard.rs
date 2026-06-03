//! Defect-scope guard's always-allowed predicate (FT-137 / ADR-078).
//!
//! Replaces the original `is_system_path` (FT-017 / FT-108 era) — that
//! predicate accepted only `.product/` and `.dec/`, which was too narrow
//! once cross-cutting features started needing to edit build manifests,
//! repo-level docs, or CI configs. Witnessed misfire: FT-136's iteration
//! 1 was blocked from committing `Cargo.toml` + the stub-crate deletions
//! because those paths sat outside both the prior `[FT-136]` commit set
//! and the narrow allowlist.
//!
//! The expanded predicate covers four additional default categories —
//! build manifests, root-level docs, CI/packaging configs, VCS metadata
//! — plus a project-configured `[scope-guard].always-allowed` array
//! read from `.dec/config.toml`. Defaults are hardcoded; extras are
//! additive (the config can grow the allowlist but cannot remove a
//! default).

use std::path::Path;

/// Path basenames that are always allowed regardless of feature scope
/// (ADR-078 §Build manifests). Matched against `Path::file_name` at any
/// depth — `Cargo.toml` at the workspace root and `Cargo.toml` under
/// `crates/foo/` both pass.
const ALLOWED_BASENAMES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pyproject.toml",
    "uv.lock",
    "pnpm-lock.yaml",
    "yarn.lock",
];

/// Path prefixes that are always allowed (ADR-078). `.product/` and
/// `.dec/` are the artifact graph and harness store — touched by
/// orchestration bookkeeping rather than feature-scoped worker output.
/// `.github/` and `.cargo/` are cross-cutting CI/packaging configs.
const ALLOWED_PREFIXES: &[&str] = &[".product/", ".dec/", ".github/", ".cargo/"];

/// Exact root-relative paths that are always allowed (ADR-078 §Repo-level
/// docs + §VCS metadata + §CI/packaging). Repo docs apply at root only;
/// a nested `README.md` inside a feature crate is feature-scoped and not
/// implicitly allowed.
const ALLOWED_ROOT_FILES: &[&str] = &[
    "CLAUDE.md",
    "README.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "CODE_OF_CONDUCT.md",
    "CHANGELOG.md",
    ".gitignore",
    ".gitattributes",
    "dist-workspace.toml",
    "rust-toolchain.toml",
    "rust-toolchain",
];

/// Returns `true` when `path` falls into any default always-allowed
/// category — build manifests, repo docs, CI/packaging configs, VCS
/// metadata, the artifact graph (`.product/`), or the harness store
/// (`.dec/`) — or matches a project-configured extra pattern from
/// `.dec/config.toml`'s `[scope-guard].always-allowed` array.
///
/// Extras support `**` glob suffixes (e.g. `scripts/checks/**` matches
/// any descendant path). Other patterns are matched as exact strings.
/// Unrecognised glob syntax falls through to literal-prefix matching.
pub(crate) fn is_always_allowed(path: &str, extras: &[String]) -> bool {
    if ALLOWED_PREFIXES.iter().any(|p| path.starts_with(p)) {
        return true;
    }
    if ALLOWED_ROOT_FILES.contains(&path) {
        return true;
    }
    if let Some(base) = Path::new(path).file_name().and_then(|s| s.to_str()) {
        if ALLOWED_BASENAMES.contains(&base) {
            return true;
        }
    }
    extras.iter().any(|pat| matches_extra_pattern(path, pat))
}

/// Match a path against a `[scope-guard].always-allowed` pattern.
///
/// - `prefix/**` matches any path whose first `prefix.len()` bytes equal
///   `prefix` and whose next byte is `/`. Covers nested descendants but
///   not the prefix itself with no child.
/// - Patterns without `*` are exact-match.
/// - Patterns containing `*` elsewhere fall back to literal-prefix match
///   up to the first `*` — covers `foo*` style without pulling in a full
///   glob crate.
fn matches_extra_pattern(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path.starts_with(prefix)
            && path.len() > prefix.len()
            && path.as_bytes()[prefix.len()] == b'/'
    } else if let Some(idx) = pattern.find('*') {
        path.starts_with(&pattern[..idx])
    } else {
        path == pattern
    }
}

/// Load the `[scope-guard].always-allowed` array from
/// `<workdir>/.dec/config.toml`. Returns an empty vector when the file
/// is missing, when the section is absent, when parsing fails, or when
/// the array contains non-string entries. The guard's protective intent
/// is preserved by failing closed (no extras) rather than panicking on
/// malformed config — a drive must not halt on a config typo.
pub fn load_scope_guard_extras(workdir: &Path) -> Vec<String> {
    let path = workdir.join(".dec").join("config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(value) = text.parse::<toml::Value>() else {
        return Vec::new();
    };
    value
        .get("scope-guard")
        .and_then(|sg| sg.get("always-allowed"))
        .and_then(|aa| aa.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
