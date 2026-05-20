//! Orchestration store load/dump helpers shared across features (ADR-016).
//!
//! The slice-1 store layer materialises `.dec/store/orchestration.nq` as
//! an on-disk N-Quads dump. Every read-side command (`dec status`,
//! `dec _sparql`, `dec events`, `dec session`, `dec health`) needs the
//! same load dance; consolidating it here keeps the import surface
//! across features::* small.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

/// Path of the persisted orchestration dump under a working directory.
#[must_use]
pub fn orchestration_dump_path(workdir: &Path) -> PathBuf {
    workdir.join(".dec").join("store").join("orchestration.nq")
}

/// Load `<workdir>/.dec/store/orchestration.nq` into a fresh in-memory
/// `Store`. Returns an error when the dump is missing or unreadable.
pub fn open_orchestration_store(workdir: &Path) -> Result<Store> {
    let dump = orchestration_dump_path(workdir);
    if !dump.exists() {
        return Err(anyhow!(
            "no orchestration store at {} — run `dec init` first",
            dump.display()
        ));
    }
    load_store_from_dump(&dump)
}

/// Load the N-Quads file at `dump` into a fresh in-memory `Store`.
pub fn load_store_from_dump(dump: &Path) -> Result<Store> {
    let bytes = fs::read(dump).with_context(|| format!("reading {}", dump.display()))?;
    let store = Store::new().context("opening in-memory orchestration store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump.display()))?;
    Ok(store)
}

/// Dump `store` atomically to `dump` (write-temp + rename).
pub fn persist_store(store: &Store, dump: &Path) -> Result<()> {
    if let Some(parent) = dump.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = dump.with_extension("nq.tmp");
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping orchestration store")?;
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, dump)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), dump.display()))?;
    Ok(())
}
