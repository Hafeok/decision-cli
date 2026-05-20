//! Embedded worker manifest — the static table of role → expected worker identity.
//!
//! Slice 1 ships exactly one entry (`code-writer`). The asset is
//! embedded via `include_str!`; the SHA-256 of the asset bytes is the
//! stable fingerprint TC-046/TC-048 assert on, and is recorded on the
//! bootstrap session's PROV-O record alongside the ontology hash.

use sha2::{Digest, Sha256};

/// Embedded TOML bytes — the canonical fingerprint of the manifest.
pub const MANIFEST_RAW: &str = include_str!("assets/manifest.toml");

/// One entry in the manifest. Hand-mirrored from `manifest.toml`.
#[derive(Debug, Clone, Copy)]
pub struct WorkerEntry {
    /// The role this worker satisfies (e.g. `code-writer`).
    pub role: &'static str,
    /// Binary name expected on `$PATH`.
    pub console_script: &'static str,
    /// Python module the `python3 -c "import …"` probe consults.
    pub python_module: &'static str,
    /// Informational install class — drives the suggestion text.
    pub install_kind: &'static str,
    /// Path or package spec for the install command.
    pub source_hint: &'static str,
    /// Operator-facing env var for the explicit-override fallback.
    pub env_var: &'static str,
}

/// Static manifest. Adding a role here means editing `manifest.toml`
/// (for the bytes / hash) and this constant (for the runtime view).
pub const MANIFEST: &[WorkerEntry] = &[WorkerEntry {
    role: "code-writer",
    console_script: "code-writer",
    python_module: "code_writer.main",
    install_kind: "uv-tool",
    source_hint: "./workers/code-writer",
    env_var: "CODE_WRITER_CMD",
}];

/// Slice-1 active-role set for the bundled `engineering-development`
/// stream. Future slices will derive this from the value stream's value
/// actions rather than hardcoding it.
pub const ACTIVE_ROLES_ENGINEERING_DEVELOPMENT: &[&str] = &["code-writer"];

/// Look up the manifest entry for a role, if any.
#[must_use]
pub fn role_entry(role: &str) -> Option<&'static WorkerEntry> {
    MANIFEST.iter().find(|e| e.role == role)
}

/// SHA-256 (hex) of the embedded `manifest.toml` bytes.
#[must_use]
pub fn manifest_sha256_hex() -> String {
    let mut h = Sha256::new();
    h.update(MANIFEST_RAW.as_bytes());
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_code_writer_entry() {
        let e = role_entry("code-writer").expect("code-writer in manifest");
        assert_eq!(e.console_script, "code-writer");
        assert_eq!(e.python_module, "code_writer.main");
        assert_eq!(e.env_var, "CODE_WRITER_CMD");
    }

    #[test]
    fn manifest_sha256_is_stable_hex() {
        let h = manifest_sha256_hex();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn manifest_toml_mentions_each_runtime_entry() {
        for e in MANIFEST {
            assert!(MANIFEST_RAW.contains(e.role));
            assert!(MANIFEST_RAW.contains(e.console_script));
        }
    }
}
