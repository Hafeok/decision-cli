//! Id-minting + collision detection for `dec verify graph new` (FT-041).
//!
//! Mirrors `verify_bench_new::mint` but for `VG-NNN[-suffix]` ids under
//! `.dec/verify/graph/`. The next mint scans the directory for the
//! highest existing `VG-NNN` prefix and returns the next integer.

use std::fs;
use std::io;
use std::path::Path;

/// Mint the next `VG-NNN` id (no suffix) free under `graph_dir`.
pub fn mint_next_id(graph_dir: &Path) -> io::Result<String> {
    let next = next_index(graph_dir)?;
    Ok(format!("VG-{next:03}"))
}

/// True iff a file `VG-<tail>*.ttl` (matching `id` or `id-*`) already
/// lives under `graph_dir`. Catches both `VG-007.ttl` and
/// `VG-007-named.ttl` shapes.
pub fn id_exists(graph_dir: &Path, id: &str) -> io::Result<bool> {
    if !graph_dir.exists() {
        return Ok(false);
    }
    let prefix_dot = format!("{id}.");
    let prefix_dash = format!("{id}-");
    for entry in fs::read_dir(graph_dir)? {
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

fn next_index(graph_dir: &Path) -> io::Result<u64> {
    if !graph_dir.exists() {
        return Ok(1);
    }
    let mut max_seen: u64 = 0;
    for entry in fs::read_dir(graph_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".ttl") else {
            continue;
        };
        if let Some(n) = parse_graph_index(stem) {
            if n > max_seen {
                max_seen = n;
            }
        }
    }
    Ok(max_seen + 1)
}

fn parse_graph_index(stem: &str) -> Option<u64> {
    let tail = stem.strip_prefix("VG-")?;
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
            p.push(format!("dec-vg-mint-{tag}-{pid}-{nonce}"));
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
        File::create(dir.join(name)).expect("touch");
    }

    #[test]
    fn next_id_in_empty_dir_is_vg_001() {
        let dir = TmpDir::new("empty");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "VG-001");
    }

    #[test]
    fn next_id_increments_past_highest() {
        let dir = TmpDir::new("incr");
        touch(dir.path(), "VG-001.ttl");
        touch(dir.path(), "VG-007-named.ttl");
        let id = mint_next_id(dir.path()).expect("mint");
        assert_eq!(id, "VG-008");
    }

    #[test]
    fn id_exists_matches_plain_and_suffixed() {
        let dir = TmpDir::new("exists");
        touch(dir.path(), "VG-007-staging.ttl");
        assert!(id_exists(dir.path(), "VG-007").expect("io"));
        assert!(!id_exists(dir.path(), "VG-008").expect("io"));
        touch(dir.path(), "VG-010.ttl");
        assert!(id_exists(dir.path(), "VG-010").expect("io"));
    }
}
