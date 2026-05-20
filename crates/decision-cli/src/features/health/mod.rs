//! `dec health` — liveness check (FT-012).
//!
//! Reports on three structural readiness gates:
//!
//! 1. **Ontology parses.** The embedded base ontology + SHACL shapes
//!    (FT-006) must load without error. A failure here is a build-time
//!    bug.
//! 2. **Store opens.** When the working dir is initialised, the
//!    persisted N-Quads dump must parse into an Oxigraph store. When
//!    the working dir is **not** initialised, this check is skipped
//!    and the report says so — `dec health` is one of two commands that
//!    MUST NOT require a `.dec/` (FT-012 invariants, ADR-012).
//! 3. **Writer operational.** When the store opens and exposes a
//!    `ValueStream`, binding [`crate::StreamWriter`] to that stream must
//!    succeed. This proves the FT-010 chokepoint can accept mutations.
//!
//! Each gate produces one line of output. Exit code 0 means all
//! applicable gates passed; any failure produces exit code 1.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use oxigraph::io::RdfFormat;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::ontology::OntologyHandle;
use crate::core::scope::ActiveScope;
use crate::core::StreamWriter;

/// Outcome of one of the three gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    /// Gate passed; the detail is for display.
    Ok(String),
    /// Gate not applicable in the current environment; the detail is
    /// for display (e.g. "no .dec/ — run `dec init` to initialise").
    Skipped(String),
    /// Gate failed; the detail is the error message.
    Failed(String),
}

impl GateOutcome {
    /// Whether this gate counts as a failure for the overall exit code.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Render as `OK / SKIP / FAIL` plus the carried detail.
    #[must_use]
    pub fn render(&self, label: &str) -> String {
        let (tag, detail) = match self {
            Self::Ok(d) => ("OK", d),
            Self::Skipped(d) => ("SKIP", d),
            Self::Failed(d) => ("FAIL", d),
        };
        format!("[{tag}] {label}: {detail}")
    }
}

/// Full health report.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Result of loading the embedded ontology (FT-006).
    pub ontology: GateOutcome,
    /// Result of opening the persisted orchestration store (FT-009).
    pub store: GateOutcome,
    /// Result of binding `StreamWriter` to the active `ValueStream` (FT-010).
    pub writer: GateOutcome,
}

impl HealthReport {
    /// True iff every applicable gate passed.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        !self.ontology.is_failure() && !self.store.is_failure() && !self.writer.is_failure()
    }

    /// Render as one line per gate (stable order: ontology, store, writer).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.ontology.render("ontology"));
        out.push('\n');
        out.push_str(&self.store.render("store"));
        out.push('\n');
        out.push_str(&self.writer.render("writer"));
        out.push('\n');
        out
    }
}

/// Run the three liveness gates against the given working dir.
///
/// The function intentionally does not return `Result`: every gate's
/// failure is reported on the report rather than short-circuiting, so
/// operators see the full picture in one invocation.
#[must_use]
pub fn check(workdir: &Path) -> HealthReport {
    HealthReport {
        ontology: check_ontology(),
        store: check_store(workdir),
        writer: check_writer(workdir),
    }
}

fn check_ontology() -> GateOutcome {
    match OntologyHandle::load() {
        Ok(handle) => GateOutcome::Ok(format!(
            "embedded ontology v{} parsed (sha256:{}…)",
            handle.version(),
            short_hash(handle.hash())
        )),
        Err(err) => GateOutcome::Failed(format!("ontology load failed: {err}")),
    }
}

fn check_store(workdir: &Path) -> GateOutcome {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    if !dump_path.exists() {
        return GateOutcome::Skipped(format!(
            "no orchestration store at {} (this is fine — `dec init` creates it)",
            dump_path.display()
        ));
    }
    let bytes = match std::fs::read(&dump_path) {
        Ok(b) => b,
        Err(e) => return GateOutcome::Failed(format!("read {}: {e}", dump_path.display())),
    };
    let store = match Store::new() {
        Ok(s) => s,
        Err(e) => return GateOutcome::Failed(format!("open in-memory store: {e}")),
    };
    if let Err(e) = store.load_from_reader(RdfFormat::NQuads, bytes.as_slice()) {
        return GateOutcome::Failed(format!("load {}: {e}", dump_path.display()));
    }
    GateOutcome::Ok(format!(
        "store at {} opened ({} bytes)",
        dump_path.display(),
        bytes.len()
    ))
}

fn check_writer(workdir: &Path) -> GateOutcome {
    let dec_dir = workdir.join(".dec");
    if !dec_dir.exists() {
        return GateOutcome::Skipped(
            "no .dec/ yet — `dec init` will initialise the writer".to_string(),
        );
    }
    let scope = match ActiveScope::load(workdir) {
        Ok(s) => s,
        Err(err) => return GateOutcome::Failed(format!("load active scope: {err}")),
    };
    let store = match load_orchestration_store(&dec_dir) {
        Ok(s) => s,
        Err(g) => return g,
    };
    let stream = match NamedNode::new(&scope.stream_iri) {
        Ok(n) => n,
        Err(e) => {
            return GateOutcome::Failed(format!("active stream IRI {}: {e}", scope.stream_iri))
        }
    };
    match StreamWriter::open(Arc::new(store), stream) {
        Ok(_) => GateOutcome::Ok(format!("StreamWriter bound to <{}>", scope.stream_iri)),
        Err(e) => GateOutcome::Failed(format!("StreamWriter bind: {e}")),
    }
}

/// Read the on-disk N-Quads dump under `<dec_dir>/store/orchestration.nq`
/// into a fresh in-memory store, returning a `GateOutcome::Failed` if any
/// step (read, open, load) reports an error.
fn load_orchestration_store(dec_dir: &Path) -> Result<Store, GateOutcome> {
    let dump_path = dec_dir.join("store").join("orchestration.nq");
    let bytes = std::fs::read(&dump_path)
        .map_err(|e| GateOutcome::Failed(format!("read {}: {e}", dump_path.display())))?;
    let store = Store::new().map_err(|e| GateOutcome::Failed(format!("open store: {e}")))?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .map_err(|e| GateOutcome::Failed(format!("load store: {e}")))?;
    Ok(store)
}

fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

/// Convenience wrapper used by `main.rs`: render the report and return
/// `Result` so the caller can map to an exit code.
pub fn run(workdir: &Path) -> Result<HealthReport> {
    let report = check(workdir);
    if report.is_healthy() {
        Ok(report)
    } else {
        // Surface the rendered report on the error so `main.rs` can
        // print it on the failure path without losing the structured
        // result.
        Err(anyhow!("{}", report.render()))
    }
}
