//! `RunContext` — the mutable state threaded through Phase 3's step loop.
//!
//! Holds:
//!   * the resolved working directory (`${dec_workdir}` source),
//!   * the capture binding table (`${name}` resolution),
//!   * the parent graph's step table (for `wait-for` lookup),
//!   * a reference to the parent graph's step IRIs (for stop-on-fail
//!     diagnostics).
//!
//! Internal to the runner; not exposed by `core::verify::runner`'s public
//! surface per FT-098 §Boundaries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dec_graph::ontology::verification_graph::VerificationStep;

/// Per-run mutable state passed by `&mut` reference to every step
/// handler. Lives for one run; never persisted beyond the result
/// artifact (FT-098 §Invariants).
pub struct RunContext {
    /// Working directory the runner operates against (test root).
    pub workdir: PathBuf,
    /// Per-step ephemeral workdir (`${dec_workdir}`) — either a freshly
    /// minted temp directory or a repo-relative resolved path.
    pub dec_workdir: PathBuf,
    /// Whether `dec_workdir` should be cleaned up at Phase-4 teardown.
    pub cleanup_workdir: bool,
    /// Capture bindings (`${name}` → value). Seeded from the request and
    /// extended by `capture` steps (FT-098 §Phase 3.2).
    pub bindings: HashMap<String, String>,
    /// Lookup table for the parent graph's steps, keyed by step IRI.
    /// Used by `wait-for` to resolve its wrapped sub-condition.
    pub step_lookup: HashMap<String, VerificationStep>,
    /// Step index → last-recorded outcome / stdout / exit code, so
    /// `capture` steps can read prior trace state without storing the
    /// full trace history in their own type.
    pub prior_outputs: Vec<PriorOutput>,
    /// Set to `Some(index)` when a stop-on-fail step has fired; every
    /// subsequent step is recorded as `unrunnable` with a "skipped"
    /// error message.
    pub stop_on_fail_index: Option<usize>,
}

/// Snapshot of a prior step's runtime values (stdout / exit code) that
/// `capture` steps may bind into the context.
#[derive(Debug, Clone, Default)]
pub struct PriorOutput {
    /// Captured stdout (cap 4 KiB) — empty when the step does not
    /// produce stdout (e.g. `sparql-assertion` summarises via
    /// `stdout_excerpt` itself).
    pub stdout: String,
    /// Captured stderr excerpt.
    pub stderr: String,
    /// Exit code if applicable (`shell-command`, `http-request`).
    pub exit_code: Option<i64>,
}

impl RunContext {
    /// Construct a new run context. `dec_workdir` is the directory step
    /// handlers chroot their relative paths to; `bindings` is the
    /// pre-seeded capture map from the request.
    pub(crate) fn new(
        workdir: PathBuf,
        dec_workdir: PathBuf,
        cleanup_workdir: bool,
        bindings: HashMap<String, String>,
        steps: &[VerificationStep],
    ) -> Self {
        let mut step_lookup: HashMap<String, VerificationStep> = HashMap::new();
        for s in steps {
            step_lookup.insert(s.id.as_str().to_string(), s.clone());
        }
        let mut ctx = Self {
            workdir,
            dec_workdir: dec_workdir.clone(),
            cleanup_workdir,
            bindings,
            step_lookup,
            prior_outputs: Vec::with_capacity(steps.len()),
            stop_on_fail_index: None,
        };
        ctx.bindings
            .entry("dec_workdir".to_string())
            .or_insert_with(|| dec_workdir.to_string_lossy().into_owned());
        ctx
    }

    /// Resolve `${name}` placeholders against `bindings`. Returns the
    /// first unbound name as the `Err` arm so the caller surfaces
    /// `outcome = unrunnable` per FT-098 §Phase 3.1.
    pub(crate) fn substitute(&self, raw: &str) -> Result<String, String> {
        let mut out = String::with_capacity(raw.len());
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                let Some(end_rel) = raw[i + 2..].find('}') else {
                    // Unterminated `${`; copy literally.
                    out.push('$');
                    out.push('{');
                    i += 2;
                    continue;
                };
                let end = i + 2 + end_rel;
                let name = &raw[i + 2..end];
                if let Some(value) = self.bindings.get(name) {
                    out.push_str(value);
                } else {
                    return Err(name.to_string());
                }
                i = end + 1;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        Ok(out)
    }

    /// Resolve `path` against `dec_workdir` if it is relative;
    /// absolute paths are returned untouched.
    pub(crate) fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.dec_workdir.join(p)
        }
    }

    /// Append a step's runtime output to the prior-output table so a
    /// subsequent `capture` step can read it.
    pub(crate) fn record_output(&mut self, output: PriorOutput) {
        self.prior_outputs.push(output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_replaces_known_bindings() {
        let mut bindings: HashMap<String, String> = HashMap::new();
        bindings.insert("name".into(), "world".into());
        let ctx = RunContext::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
            false,
            bindings,
            &[],
        );
        assert_eq!(ctx.substitute("hello ${name}!").unwrap(), "hello world!");
    }

    #[test]
    fn substitute_reports_unbound_names() {
        let ctx = RunContext::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
            false,
            HashMap::new(),
            &[],
        );
        let err = ctx.substitute("echo ${oops}").unwrap_err();
        assert_eq!(err, "oops");
    }

    #[test]
    fn dec_workdir_binding_seeded() {
        let ctx = RunContext::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/scratch"),
            false,
            HashMap::new(),
            &[],
        );
        assert_eq!(
            ctx.bindings.get("dec_workdir").map(String::as_str),
            Some("/scratch")
        );
    }
}
