//! `file-assertion` step handler (FT-098 §Phase 3.2).
//!
//! Asserts existence, content equality (`dec:expectContent`), or
//! SHA-256 hash (`dec:expectHash`) on a path resolved against
//! `dec_workdir`.

use sha2::{Digest, Sha256};

use crate::core::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::RunContext;
use super::common::iso_now;
use super::{StepKindHandler, StepRunTrace};

/// `file-assertion` handler.
pub struct FileHandler;

impl StepKindHandler for FileHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::FileAssertion {
            path,
            expect_hash,
            expect_content,
        } = &step.fields
        else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "file-assertion handler received non-file fields".into(),
            );
        };
        let path = match ctx.substitute(path) {
            Ok(v) => v,
            Err(missing) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("unbound capture: ${{{missing}}}"),
                );
            }
        };
        let resolved = ctx.resolve_path(&path);
        if !resolved.exists() {
            let ended = iso_now();
            // Existence-only assertion (no hash, no content) with a
            // missing file is a `fail`, not unrunnable. Hash/content
            // assertions on a missing file are also `fail`.
            return StepRunTrace::fail(
                started,
                ended,
                format!("file missing: {p}", p = resolved.display()),
            );
        }
        let bytes = match std::fs::read(&resolved) {
            Ok(b) => b,
            Err(e) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("read failed: {e}"),
                );
            }
        };
        if let Some(expected) = expect_hash {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let got = hex_encode(&hasher.finalize());
            if got != *expected {
                let ended = iso_now();
                return StepRunTrace::fail(
                    started,
                    ended,
                    format!("expected sha256={expected}, got {got}"),
                );
            }
        }
        if let Some(expected) = expect_content {
            let got = String::from_utf8_lossy(&bytes);
            if got != *expected {
                let ended = iso_now();
                return StepRunTrace::fail(
                    started,
                    ended,
                    format!(
                        "expected content {} bytes, got {} bytes (mismatch)",
                        expected.len(),
                        got.len()
                    ),
                );
            }
        }
        let ended = iso_now();
        StepRunTrace::pass(started, ended)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let hi = (b >> 4) & 0x0f;
        let lo = b & 0x0f;
        s.push(hex_nibble(hi));
        s.push(hex_nibble(lo));
    }
    s
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '?',
    }
}
