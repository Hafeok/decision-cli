//! Waiver persistence — file + StreamWriter chokepoint (FT-047 §Behaviour).
//!
//! When the gate accepts a waiver intent, it:
//!
//!   1. allocates the next `CW-NNN` id (filesystem scan under
//!      `.dec/verify/waivers/`),
//!   2. builds the [`CoverageWaiver`] artifact,
//!   3. routes the SHACL-validated quads through [`StreamWriter`],
//!   4. writes the canonical Turtle to disk,
//!   5. returns the waiver IRI (and on-disk path) so the dispatching
//!      verb can record a `prov:used` link on its session.
//!
//! The single chokepoint contract is preserved — there is no direct
//! file-only path that bypasses the store.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use thiserror::Error;

use crate::core::ontology::coverage_waiver::CoverageWaiver;
use crate::core::vocab::{waivers_named_graph, IRI_DEC_WAIVER_PREFIX};
use crate::core::StreamWriter;

use super::waiver_intent::WaiverIntent;

/// Failures surfaced while persisting a waiver. Distinct from the
/// upstream `ChainIntegrityError` so callers can choose which exit code
/// to map onto (FT-047 §Error handling — schema/io violations → exit 1).
#[derive(Debug, Error)]
pub enum WaiverPersistError {
    /// SHACL refused, or the StreamWriter rejected the mutation.
    #[error("schema violation: {0}")]
    SchemaViolation(String),
    /// I/O failure while writing the on-disk Turtle.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Turtle serialisation failed.
    #[error("turtle emit error: {0}")]
    Emit(String),
}

/// Trait so tests can substitute id minting (e.g. always return `CW-001`).
/// Production uses [`scan_next_waiver_id`].
pub trait NextWaiverIdResolver {
    /// Return the next available waiver id (`CW-NNN`).
    fn next_id(&self, waivers_dir: &Path) -> Result<String, WaiverPersistError>;
}

/// Default resolver: scans `waivers_dir` for existing `CW-*` files and
/// returns the next zero-padded id (3 digits).
pub struct FilesystemIdResolver;

impl NextWaiverIdResolver for FilesystemIdResolver {
    fn next_id(&self, waivers_dir: &Path) -> Result<String, WaiverPersistError> {
        scan_next_waiver_id(waivers_dir)
    }
}

/// Scan `waivers_dir` and produce the next `CW-NNN` id (3-digit zero pad).
pub fn scan_next_waiver_id(waivers_dir: &Path) -> Result<String, WaiverPersistError> {
    if !waivers_dir.exists() {
        return Ok("CW-001".to_string());
    }
    let mut max_id: u64 = 0;
    for entry in fs::read_dir(waivers_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        let Some(stem) = s.strip_suffix(".ttl") else {
            continue;
        };
        let Some(num) = stem.strip_prefix("CW-") else {
            continue;
        };
        if let Ok(n) = num.parse::<u64>() {
            if n > max_id {
                max_id = n;
            }
        }
    }
    Ok(format!("CW-{:03}", max_id + 1))
}

/// Result of [`persist_waiver`] — the IRI minted for the waiver and the
/// on-disk path the canonical Turtle was written to.
#[derive(Debug, Clone)]
pub struct PersistedWaiver {
    /// Canonical IRI of the persisted waiver (`…/ns/waiver/CW-NNN`).
    pub iri: NamedNode,
    /// On-disk Turtle path (`.dec/verify/waivers/CW-NNN.ttl`).
    pub path: PathBuf,
    /// Snapshot of uncovered TC IRIs recorded on the waiver.
    pub uncovered_at_waive: Vec<NamedNode>,
}

/// Persist a waiver per FT-047 §Behaviour step 5.
///
/// `workdir` is the root of the orchestrated working tree (the same one
/// that owns `.dec/`); `feature_iri` is the resolved IRI of the gated
/// feature; `uncovered_tcs` is the snapshot returned by the coverage
/// primitive at gate-firing time; `attributed_to` is the PROV-O actor.
pub fn persist_waiver(
    workdir: &Path,
    writer: &StreamWriter,
    feature_iri: NamedNode,
    intent: &WaiverIntent,
    uncovered_tcs: Vec<NamedNode>,
    attributed_to: NamedNode,
) -> Result<PersistedWaiver, WaiverPersistError> {
    let waivers_dir = workdir.join(".dec").join("verify").join("waivers");
    fs::create_dir_all(&waivers_dir)?;
    let id = FilesystemIdResolver.next_id(&waivers_dir)?;
    let waiver = CoverageWaiver {
        id: id.clone(),
        waiver_for: feature_iri,
        reason: intent.reason.clone(),
        attributed_to,
        created: Utc::now(),
        uncovered_at_waive: uncovered_tcs.clone(),
    };

    let quads = waiver.to_quads(waivers_named_graph());
    let mutation =
        Mutation::insert(quads.iter().cloned()).with_cause(format!("dec implement: chain-integrity waiver {id}"));
    writer
        .commit(mutation)
        .map_err(|e| WaiverPersistError::SchemaViolation(format!("{e:#}")))?;

    let ttl = waiver
        .to_canonical_turtle()
        .map_err(|e| WaiverPersistError::Emit(e.to_string()))?;
    let path = waivers_dir.join(waiver.filename());
    let tmp = path.with_extension("ttl.tmp");
    fs::write(&tmp, ttl.as_bytes())?;
    fs::rename(&tmp, &path)?;

    let iri = NamedNode::new_unchecked(format!("{IRI_DEC_WAIVER_PREFIX}{id}"));
    Ok(PersistedWaiver {
        iri,
        path,
        uncovered_at_waive: uncovered_tcs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fresh_dir(tag: &str) -> PathBuf {
        let mut p = env::temp_dir();
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        p.push(format!(
            "dec-waiver-writer-{tag}-{}-{}-{}",
            std::process::id(),
            n,
            c
        ));
        fs::create_dir_all(&p).expect("mkdir");
        p
    }

    #[test]
    fn next_id_starts_at_001_when_dir_missing() {
        let base = fresh_dir("missing");
        let dir = base.join("waivers-not-yet");
        assert_eq!(scan_next_waiver_id(&dir).expect("ok"), "CW-001");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn next_id_increments_past_existing_files() {
        let dir = fresh_dir("incr");
        fs::write(dir.join("CW-001.ttl"), "").expect("write");
        fs::write(dir.join("CW-007.ttl"), "").expect("write");
        // Ignore non-matching files.
        fs::write(dir.join("not-a-waiver.txt"), "").expect("write");
        assert_eq!(scan_next_waiver_id(&dir).expect("ok"), "CW-008");
        let _ = fs::remove_dir_all(&dir);
    }
}
