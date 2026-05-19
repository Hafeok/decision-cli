//! Bundle assembly via `product context` and product-root discovery.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

/// Resolve a product-cli root by walking up from `workdir`. When no
/// `.product/` is found we default to `workdir/.product/`. Tests and
/// CI fixtures may also pass `--product-root` explicitly.
pub fn resolve_product_root(workdir: &Path, override_path: Option<&Path>) -> PathBuf {
    if let Some(p) = override_path {
        return p.to_path_buf();
    }
    let mut current = Some(workdir.to_path_buf());
    while let Some(p) = current {
        if p.join(".product").is_dir() {
            return p;
        }
        current = p.parent().map(Path::to_path_buf);
    }
    workdir.to_path_buf()
}

/// The fixed location for the product-cli CodeChange graph slice.
pub(super) fn product_codechange_path(product_root: &Path) -> PathBuf {
    product_root
        .join(".product")
        .join("graph")
        .join("code-changes.nq")
}

pub(super) fn assemble_bundle(
    product_root: &Path,
    feature_id: &str,
    depth: usize,
) -> Result<String> {
    let mut cmd = Command::new("product");
    cmd.arg("context")
        .arg(feature_id)
        .arg("--depth")
        .arg(depth.to_string())
        .arg("--root")
        .arg(product_root);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    match cmd.output() {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout).into_owned();
            if body.trim().is_empty() {
                Ok(synthetic_bundle(feature_id))
            } else {
                Ok(body)
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            tracing::warn!(
                target = "dec.implement",
                feature = feature_id,
                "product-cli context failed: {stderr}; using synthetic bundle"
            );
            Ok(synthetic_bundle(feature_id))
        }
        Err(_) => Ok(synthetic_bundle(feature_id)),
    }
}

fn synthetic_bundle(feature_id: &str) -> String {
    format!(
        "# {feature_id} — synthetic context bundle\n\n\
This is a slice-1 fallback bundle produced because `product context` is\n\
not available in the current environment. The harness still computes a\n\
stable SHA-256 over these bytes so the Session's `dec:contentHash` is\n\
deterministic.\n",
    )
}

pub(super) fn persist_store(store: &Store, dump_path: &Path) -> Result<()> {
    if let Some(parent) = dump_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = dump_path.with_extension("nq.tmp");
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping orchestration store")?;
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, dump_path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), dump_path.display()))?;
    Ok(())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}
