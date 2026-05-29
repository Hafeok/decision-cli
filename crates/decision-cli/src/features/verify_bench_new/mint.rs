//! Id-minting for `dec verify bench new`, including collision detection (FT-038).
//!
//! Ids are monotonic per ADR-038 §Invariants: the next mint scans
//! `.dec/verify/bench/` for the highest existing `BNCH-NNN` prefix and
//! returns the next integer. Caller-supplied ids round-trip through
//! [`id_exists`] which returns true when a file with that id (regardless
//! of trailing suffix) already exists.

use std::fs;
use std::io;
use std::path::Path;

/// Scan `bench_dir` for existing `BNCH-NNN-*.ttl` files and mint the next
/// free `BNCH-NNN` id (without a suffix).
///
/// Returns `BNCH-001` when the directory does not yet exist or is empty.
/// Id width is held at 3 digits for the first 999 benches; rolls over to
/// 4+ digits naturally as the integer grows.
pub fn mint_next_id(bench_dir: &Path) -> io::Result<String> {
    let next = next_index(bench_dir)?;
    Ok(format!("BNCH-{next:03}"))
}

/// True iff a file `BNCH-<id-tail>*.ttl` (where the id matches `id` or
/// `id-*`) already lives under `bench_dir`. The id-only-prefix variant
/// catches both `BNCH-007.ttl` and `BNCH-007-named.ttl`.
pub fn id_exists(bench_dir: &Path, id: &str) -> io::Result<bool> {
    if !bench_dir.exists() {
        return Ok(false);
    }
    let prefix_dot = format!("{id}.");
    let prefix_dash = format!("{id}-");
    for entry in fs::read_dir(bench_dir)? {
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

/// Walk `bench_dir`, parse each `BNCH-NNN…` filename's numeric tail, and
/// return `max + 1` (or 1 when empty). Files that don't match the
/// `BNCH-<digits>(-…)?\.ttl` shape are ignored — the seed file
/// `BNCH-001-ephemeral-cli.ttl` does match and bumps the counter to 2.
fn next_index(bench_dir: &Path) -> io::Result<u64> {
    if !bench_dir.exists() {
        return Ok(1);
    }
    let mut max_seen: u64 = 0;
    for entry in fs::read_dir(bench_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if let Some(n) = parse_bench_index(stem) {
            if n > max_seen {
                max_seen = n;
            }
        }
    }
    Ok(max_seen + 1)
}

/// Parse the numeric tail of `BNCH-NNN[-suffix]` stems. Returns `None`
/// when the stem does not match the `BNCH-<digits>` shape.
fn parse_bench_index(stem: &str) -> Option<u64> {
    let tail = stem.strip_prefix("BNCH-")?;
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
    fn next_id_in_empty_dir_is_bnch_001() {
        let dir = TmpDir::new("empty");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "BNCH-001");
    }

    #[test]
    fn next_id_increments_past_highest() {
        let dir = TmpDir::new("incr");
        touch(dir.path(), "BNCH-001-ephemeral-cli.ttl");
        touch(dir.path(), "BNCH-002.ttl");
        touch(dir.path(), "BNCH-007-named.ttl");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "BNCH-008");
    }

    #[test]
    fn non_bench_files_are_ignored() {
        let dir = TmpDir::new("noise");
        touch(dir.path(), "BNCH-005.ttl");
        touch(dir.path(), "README.md");
        touch(dir.path(), "not-a-bench.ttl");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "BNCH-006");
    }

    #[test]
    fn id_exists_matches_plain_and_suffixed() {
        let dir = TmpDir::new("exists");
        touch(dir.path(), "BNCH-007-staging.ttl");
        assert!(id_exists(dir.path(), "BNCH-007").expect("io"));
        assert!(!id_exists(dir.path(), "BNCH-008").expect("io"));
        touch(dir.path(), "BNCH-010.ttl");
        assert!(id_exists(dir.path(), "BNCH-010").expect("io"));
    }

    #[test]
    fn id_exists_returns_false_when_dir_missing() {
        let dir = TmpDir::new("missing");
        let missing = dir.path().join("does-not-exist");
        assert!(!id_exists(&missing, "BNCH-001").expect("io"));
    }
}
