//! Id-minting for `dec verify env new`, including collision detection (FT-038).
//!
//! Ids are monotonic per ADR-038 §Invariants: the next mint scans
//! `.dec/verify/env/` for the highest existing `ENV-NNN` prefix and
//! returns the next integer. Caller-supplied ids round-trip through
//! [`id_exists`] which returns true when a file with that id (regardless
//! of trailing suffix) already exists.

use std::fs;
use std::io;
use std::path::Path;

/// Scan `env_dir` for existing `ENV-NNN-*.ttl` files and mint the next
/// free `ENV-NNN` id (without a suffix).
///
/// Returns `ENV-001` when the directory does not yet exist or is empty.
/// Id width is held at 3 digits for the first 999 envs; rolls over to
/// 4+ digits naturally as the integer grows.
pub fn mint_next_id(env_dir: &Path) -> io::Result<String> {
    let next = next_index(env_dir)?;
    Ok(format!("ENV-{next:03}"))
}

/// True iff a file `ENV-<id-tail>*.ttl` (where the id matches `id` or
/// `id-*`) already lives under `env_dir`. The id-only-prefix variant
/// catches both `ENV-007.ttl` and `ENV-007-named.ttl`.
pub fn id_exists(env_dir: &Path, id: &str) -> io::Result<bool> {
    if !env_dir.exists() {
        return Ok(false);
    }
    let prefix_dot = format!("{id}.");
    let prefix_dash = format!("{id}-");
    for entry in fs::read_dir(env_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.ends_with(".ttl") {
            continue;
        }
        if name == format!("{id}.ttl")
            || name.starts_with(&prefix_dot)
            || name.starts_with(&prefix_dash)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Walk `env_dir`, parse each `ENV-NNN…` filename's numeric tail, and
/// return `max + 1` (or 1 when empty). Files that don't match the
/// `ENV-<digits>(-…)?\.ttl` shape are ignored — the seed file
/// `ENV-001-ephemeral-cli.ttl` does match and bumps the counter to 2.
fn next_index(env_dir: &Path) -> io::Result<u64> {
    if !env_dir.exists() {
        return Ok(1);
    }
    let mut max_seen: u64 = 0;
    for entry in fs::read_dir(env_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if let Some(n) = parse_env_index(stem) {
            if n > max_seen {
                max_seen = n;
            }
        }
    }
    Ok(max_seen + 1)
}

/// Parse the numeric tail of `ENV-NNN[-suffix]` stems. Returns `None`
/// when the stem does not match the `ENV-<digits>` shape.
fn parse_env_index(stem: &str) -> Option<u64> {
    let tail = stem.strip_prefix("ENV-")?;
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
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
            p.push(format!("dec-mint-{tag}-{pid}-{nonce}"));
            std::fs::create_dir_all(&p).expect("create tmp");
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
        File::create(dir.join(name)).expect("touch");
    }

    #[test]
    fn next_id_in_empty_dir_is_env_001() {
        let dir = TmpDir::new("empty");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "ENV-001");
    }

    #[test]
    fn next_id_increments_past_highest() {
        let dir = TmpDir::new("incr");
        touch(dir.path(), "ENV-001-ephemeral-cli.ttl");
        touch(dir.path(), "ENV-002.ttl");
        touch(dir.path(), "ENV-007-named.ttl");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "ENV-008");
    }

    #[test]
    fn non_env_files_are_ignored() {
        let dir = TmpDir::new("noise");
        touch(dir.path(), "ENV-005.ttl");
        touch(dir.path(), "README.md");
        touch(dir.path(), "not-an-env.ttl");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "ENV-006");
    }

    #[test]
    fn id_exists_matches_plain_and_suffixed() {
        let dir = TmpDir::new("exists");
        touch(dir.path(), "ENV-007-staging.ttl");
        assert!(id_exists(dir.path(), "ENV-007").expect("io"));
        assert!(!id_exists(dir.path(), "ENV-008").expect("io"));
        touch(dir.path(), "ENV-010.ttl");
        assert!(id_exists(dir.path(), "ENV-010").expect("io"));
    }

    #[test]
    fn id_exists_returns_false_when_dir_missing() {
        let dir = TmpDir::new("missing");
        let missing = dir.path().join("does-not-exist");
        assert!(!id_exists(&missing, "ENV-001").expect("io"));
    }
}
